# Wii std synchronization and entropy status

Updated: 2026-10-14. Source and link results only. No Wii execution result.

## Synchronization design

The pinned Rust library selects its existing threaded futex implementations
for `Mutex`, `Condvar`, `RwLock`, and `Once` on Wii. `OnceLock` uses that once
implementation. The overlay changes platform selection and provides the wait
transport. It does not copy or rewrite the four synchronization algorithms.
The separate GameCube target does not select these adapters.

`ogc-std-runtime` supplies `__wii_std_futex_wait` and `__wii_std_futex_wake`.
They call the production LWP scheduler. The scheduler compares the atomic word
and registers the blocked thread while interrupts are disabled. On Wii's single
CPU, a store followed by a wake cannot occur between those two operations.
Waits use a distinct scheduler wait kind. Tuxedo pending tokens and external
wakes cannot consume or satisfy a futex wait.

The word must remain live at the same address until the wait ends. The scheduler
stores its address in the existing thread record. It allocates no mutex handle,
condition handle, queue handle, or waiter object. Constant construction and lazy
initialization therefore do not depend on the compatibility pools of 64 locks
and 64 conditions. Thread creation can still fail for lack of memory.

Wake-one selects the highest effective LWP priority, then the oldest waiter
within that priority. Wake-all selects every blocked waiter at the address.
The returned count is the number actually woken. A wake without a waiter is not
retained. These futex mutexes do not provide LWP mutex priority inheritance.

The pinned Rust algorithms retain these rules:

- Mutex acquisition uses acquire ordering; unlock uses release ordering.
- A condition wait reads its notification sequence before mutex release. The
  wait compares that sequence before blocking, then reacquires the mutex after
  wake or timeout. Spurious wakeups are permitted. This backend permits use of
  a condition with different mutexes; it has no fixed OS mutex association.
- Reader/writer locks permit multiple readers or one writer. Waiting writers
  normally have priority over new readers. Downgrade can wake readers together.
  Safe Rust guards enforce the public ownership and unlock rules.
- Once completion publishes with release ordering; observers use acquire
  ordering. The upstream poison and force rules are unchanged. This target uses
  panic abort, so recovery from an unwinding initializer is not a Wii capability.

The implementation uses only 32-bit atomics. Hardware ordering and contention
acceptance remain open. Source inspection and target compilation do not prove
ordering on a real Wii.

## Timeouts and errors

Duration conversion rounds up at 60,750,000 ticks per second and uses `u128`
arithmetic to cover `Duration::MAX`. Scheduler deadlines remain strictly inside
the signed half range. A deadline equal to the infinite-wait sentinel is delayed
by one tick. Long durations use bounded waits and deduct actual elapsed ticks.
Zero duration reports timeout if the word still matches; a changed word returns
without blocking. The wait transport does not acquire a lock for its caller.

Provider wait results are zero for a real wake or value mismatch, one for a
timeout, and a negative Newlib code for failure. Null or misaligned words and
invalid nanoseconds fail with `EINVAL`. Waiting in an exception fails with
`EACCES`; waiting with interrupts disabled fails with `EBUSY`. Cancellation maps
to `EIO`. The std adapter panics on a transport error because its infallible
synchronization interface cannot return that error. It never treats an error
as an acquired lock or a completed wait.

## Entropy decision

No entropy source is accepted. Nonempty `fill_bytes` requests panic with an
explicit unsupported message. Empty requests need no entropy and do not modify
memory. Default `HashMap` seeding calls this same function and also fails. There
is no address-based, clock-based, console-ID, or fixed-seed fallback.

The following references were inspected on 2026-10-14:

- [IOS syscall research](https://wiibrew.org/wiki/IOS/Syscalls) lists
  `IOSC_GenerateRand` at syscall 0x73. This is an IOS ARM-side call. The PPC
  process uses IPC requests to IOS resource managers; the syscall number is not
  itself a PPC entry point or an IPC command.
- Wii IOS documents `/dev/aes` as the PPC-facing AES device. Its documented
  commands are copy (IOCTLV 0), encrypt (2), and decrypt (3). The production
  `ogc-aes-sys` wrapper also uses only encrypt and decrypt. Its C reference at
  `C-sources/libogc/libogc/aes.c` opens `/dev/aes` and submits only commands 2
  and 3. A search of the checked-in `C-sources` tree found no call to
  `IOSC_GenerateRand` or other Wii random-service wrapper. It does not expose
  `IOSC_GenerateRand`.
- Wii U documentation describes `/dev/crypto` IOCTL 0x15 as
  `IOSC_GenerateRand`. This is an IOSU interface, not the Wii IOS `/dev/aes`
  interface. It cannot be used as evidence for RVL hardware support.
- The inspected Wii `/dev/es` and `/dev/net/ssl` command lists do not provide a
  public random-byte command. The inspected runtime crates also contain no
  verified Wii PPC-to-IOS random-data transport. This does not prove that no
  private or undocumented route exists; it means there is no safe supported
  route to implement yet.
- [Dolphin SSL source](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/Network/SSL.cpp)
  uses emulator-side entropy for TLS. That code does not establish a hardware
  entropy source for a Wii PowerPC application.

### IOS9 ARM image inspection

Read-only inspection of the installed Dolphin IOS9 v1034 content produced an ELF32,
big-endian ARM image with entry point `0xffff0000`. Its final zero-file-size
`PT_LOAD` has an offset equal to the end of the file and a nonzero memory size.
This is a BSS segment. Ghidra's warning about this segment does not show image
corruption.

The initialized kernel segment at `0xffff8870` contains a table of function
pointers. In this IOS9 image, table entry `0x71` points to Thumb code at
`0x13643704`; entry `0x73` points to a different function at `0x1364376c`.
This matches the IOS9 syscall numbering note that IOSC entries are two numbers
lower than the later list. The `0x71` function accepts a buffer and length and
calls the generator at `0x136435dc`. Therefore, use entry `0x71` for
`IOSC_GenerateRand` in this IOS9 image. IOS9 entry `0x73` maps to the later
list's `IOSC_GeneratePublicKeySign` entry `0x75`; do not assume a later syscall
index applies unchanged to IOS9.

REA's Ghidra 12.1.2 function analysis confirms the generator's full body at
`0x136435dc` (Evidence `ev_09b05821c5efcc6b620a1be57e05afff081693f5a9e4774eb298328de026e517`;
the pseudocode-only view is `ev_f7ac586f7d31b31dc6c3808ba837841300da44cc6194ffe89fa1936e0950ae34`).
The body increments a 32-bit counter, writes it into the last four bytes of a
zeroed 16-byte block, passes the block through an internal crypto function, and
copies the returned block bytes to the caller. The final block is truncated to
the requested length. The function also creates and releases a local IOS crypto
object, and returns errors from object setup or block processing.

On first use, initialization clears its state buffers, copies four bytes from
global address `0x136448a0` into the state, and calls the same crypto function
with built-in key handle `5`. Those four bytes are zero in the inspected ELF.
The follow-up trace below identifies the setup path that writes that global.
[WiiBrew's IOS syscall notes](https://wiibrew.org/wiki/IOS/Syscalls)
identify built-in handle `5` as the PRNG seed handle, but that documentation
and this call site do not prove the seed's hardware source or its behavior
across IOS restarts. The code supports a counter-driven,
AES-based generator interpretation. The later static trace below establishes
the AES mode and OTP connection. Seed quality and restart safety remain open.

The focused Ghidra function dossier reports four direct callers, including the
syscall wrapper at `0x13643704` and an internal signing function at
`0x1364132c`. Its disassembly requests 30 bytes from the generator for signing
work. In the syscall table, entry `0x73` points to `0x1364376c`; that routine's
local call path reaches the signing function and then the generator. This is a
stock IOS cryptographic use of generated bytes, not a raw-random IPC result.

The PPC-facing `/dev/es` command list identifies IOCTLV `0x30` as `ES_Sign`,
which returns a signature and certificate. The later static trace below
recovers the exact IPC-to-signing edge.
No raw random-byte IPC operation is established. The `/dev/aes` interface
remains limited to copy, encrypt, and decrypt.

This disassembly does not identify a fresh entropy input or establish safe
reseeding across IOS restarts. The four-byte input is traced below. The exact
`ES_Sign` IPC-to-signing call edge is recorded in the later trace below.
No raw PPC random-byte transport is established. This finding does not
enable std entropy.

### IOS9 seed input trace, 2026-10-15

The `reverse-engineer-anything` skill guided this follow-up. REA used the
installed Ghidra 12.1.2 through its CLI. The MCP session did not have a Ghidra
installation path. No persistent REA configuration change was needed.

The inspected ELF has SHA-256
`12cb0290972c3fbe0d03e27ba0f76fa868eee971fafc1e818bf896d7fa8ded91`.
REA reports two references to `0x136448a0`: a setup write at `0x136421f0`
and the generator setup read at `0x13643588`. The complete result is in
[ios9-seed-xrefs.json](research/ios9-seed-xrefs.json), Evidence
`ev_7bc97e131d084843e10bd951cf734beacf4c7f2fd473309d1eb6bd9d560daaf0`.
These are Ghidra's recovered exact-address references. They do not exclude
unresolved computed accesses.

A separate Capstone 5.0.9 disassembly of the unchanged ELF establishes the
following local paths. Selected instruction bytes, virtual addresses, file
offsets, and data literals are in
[ios9-entropy-static.txt](research/ios9-entropy-static.txt). These observations
come from the shell decoder, not from a REA function dossier.

| Code address | Observed operation |
| --- | --- |
| `0x136421dc` | Calls `0xffff1650`, then `0xffff1684`, and stores the latter result at `0x136448a0`. It clears the output counter and initialization flag. |
| `0xffff1650` | Uses the predicate at `0xffff14a4`. One branch increments the RAM word at `0xffff9540`. The other calls `0xffff7980`. |
| `0xffff1684` | Uses the same predicate. It returns either the RAM word at `0xffff9540` or the result of `0xffff7954`. |
| `0xffff7954` | Reads two 16-bit storage words, indices `0x7c` and `0x7d`, then combines them as `word_7c \| (word_7d << 16)`. |
| `0xffff7980` | Reads the same two words, increments the combined value, and writes the low and high words back. It checks read and write results. |
| `0xffff7444` | Sends an 11-bit `0x600 \| index` command and receives 16 bits. The lower routines use GPIO output `0x0d8000e0`, input `0x0d8000e8`, and input mask `0x2000`. |
| `0xffff14a4` | Calls `0xffff1440` with starting word index `5` and length `16`, then tests whether any returned word is nonzero. It caches this predicate. |
| `0xffff1440` | Reads consecutive OTP words through command register `0x0d8001ec` and data register `0x0d8001f0`. Thus the predicate reads OTP byte offsets `0x14..0x23`. |

The serial command, word size, registers, and GPIO input mask match the
open-source mini [SEEPROM reader](https://github.com/fail0verflow/mini/blob/master/seeprom.c),
[GPIO definitions](https://github.com/fail0verflow/mini/blob/master/gpio.h), and
[Hollywood register definitions](https://github.com/fail0verflow/mini/blob/master/hollywood.h).
This supports identification of the storage path as serial EEPROM. The two
word indices correspond to byte offsets `0xf8` and `0xfa`.

The four-byte generator input is therefore a counter value, with RAM and
SEEPROM branches selected by an OTP-content test. It is not evidence of a
fresh noise sample. The OTP word index `5` used by the reader is a storage
index; do not equate it with crypto object handle `5` merely because both
numbers are equal. The later trace below establishes the separate data flow
from OTP into crypto handle `5`. A secret-seeded deterministic generator can
be secure, but the counter trace alone
does not establish seed secrecy, output security, counter uniqueness after
restart, or complete error handling. In particular, setup does not check the
result of its first call to `0xffff1650` before it calls `0xffff1684`.

The first seed-input pass did not execute these routines, access EEPROM or OTP
on a console, or modify firmware. The follow-up also uses static analysis.

### IOS9 v1034 completed static trace, 2026-10-15

The follow-up establishes these results for the inspected image:

- The content hash matches its local TMD record, which identifies IOS9 v1034.
  The extracted ELF matches the content payload byte for byte.
- On the nonzero OTP-predicate branch, initialization reads OTP bytes
  `0x68..0x77` and copies them into crypto handle `5`. The alternate branch
  copies a value from the initialized image.
- The generator derives a session key and produces output through hardware
  AES-128-CBC. It uses a startup counter and a 32-bit output counter. No
  runtime reseed was found in the recovered paths.
- ES IOCTLV `0x30` reaches the signing syscall, which calls the generator for
  30 bytes. This command returns a signature and certificate.
- Setup ignores the startup-counter increment result. The generator ignores
  initial key-derivation and temporary-object deletion results. Its output
  loop has no counter-wrap rejection or periodic rekey.

The complete findings, addresses, search limits, and remaining work are in
[ios9-entropy-report.md](research/ios9-entropy-report.md). The report links the
saved Ghidra exports and original instruction bytes. These are static results,
not hardware acceptance evidence. No raw PPC random-byte command was
established. Keep the std entropy rejection.

Before replacing the rejection, establish the source, a non-destructive
transport, supported IOS/hardware revisions, generator quality, complete output
length, and error behavior. The internal IOSC random service is a promising
source only if a supported PPC-facing transport can be identified and tested.
Do not send undocumented IOCTLs to `/dev/aes` or use Wii U `/dev/crypto` on Wii.
Test the real source and its failure paths on Wii before enabling std random
data. No IOS patch, firmware write, or privilege change was used for this work.

## Probes and remaining work

`cargo xtask build-lwp-probe` links the production wait/provider tests into
`target/xtask/probes/lwp_probe.dol`. They check validation, value mismatch,
timeouts, address isolation, priority wake-one, wake-all counts, and a real
decrementer callback that wakes a waiter and rejects an IRQ-context wait.

`cargo xtask build-wii-std-probe` prepares the pinned std source, builds std, and
links `target/xtask/probes/wii_std_probe.dol`. Its additional tests use actual
`std::sync` types with production LWP workers: protected counters under
contention, condition notification and timeout reacquisition, 96 simultaneous
mutexes, concurrent readers, exclusive writers, queued-writer policy, and
contended `Once`/`OnceLock` publication. The probe uses LWP creation because the
std thread creation adapter is still open. It does not test `std::thread::spawn`.

Both DOLs link. Neither probe has run on Wii. Panic diagnostics, thread creation,
parking, clocks, streams, files, networking, and other PAL selectors still need
the work in the checklist. A linked synchronization probe is not a complete Wii
std backend or permission to enable std in an application port.
