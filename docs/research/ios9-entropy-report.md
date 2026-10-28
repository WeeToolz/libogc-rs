# IOS9 v1034 random-generator investigation

Date: 2026-10-15. This report records static inspection. No console test was
performed. No firmware, OTP, or EEPROM operation was executed.

## Decision

The image contains an IOS-specific random generator that uses an OTP key,
a startup counter, a 32-bit output counter, and hardware AES-128-CBC. A stock
ES IPC handler calls it for signing. No raw random-byte IPC command was
established by this search. Keep the Rust std entropy rejection.

The OTP connection answers the seed-location question. It does not establish
how Nintendo generated that seed, its entropy, or its secrecy on a particular
console. Missing fresh noise on each request does not by itself prove that a
secret-seeded generator is insecure. The error paths and state limits below
still prevent acceptance of this implementation without further work.

## Target identity and method

| Item | Observed value |
| --- | --- |
| Local NAND title | `00000001-00000009` |
| TMD version | `1034` |
| Content ID and index | `00000006`, index `1` |
| Content size | `1,562,220` bytes |
| Content SHA-1 | `2843f38939d2a98cb084383095302fe73e684441` |
| Content SHA-256 | `bdfa0fdb9538bc3a8d6ceafc8d9215ba2bb814decfe3c8660190398da732fd25` |
| TMD SHA-256 | `49a1477825324a7d079eea1c79877554a9586136efb2775f95aa4dbbc1d80d81` |
| ELF payload offset | `0x334` within the content |
| ELF payload size | `1,561,400` bytes |
| ELF SHA-256 | `12cb0290972c3fbe0d03e27ba0f76fa868eee971fafc1e818bf896d7fa8ded91` |

The content SHA-1 matches the local TMD record. The extracted ELF equals the
complete content payload at offset `0x334`. This is a content-integrity check;
the TMD signature chain was not independently verified in this pass.

The `reverse-engineer-anything` skill guided the work. The tools were REA
6.5.0, Ghidra 12.1.2, and Capstone 5.0.9. REA's earlier exact-address reference
result is preserved in [ios9-seed-xrefs.json](ios9-seed-xrefs.json), Evidence
`ev_7bc97e131d084843e10bd951cf734beacf4c7f2fd473309d1eb6bd9d560daaf0`.

The follow-up used a saved Ghidra project and direct Ghidra CLI exports.
[ios9-v1034-functions.txt](ios9-v1034-functions.txt) preserves selected
pseudocode, incoming references, function ranges, and assembly. These direct
exports are not REA Evidence records. [ios9-v1034-paths.txt](ios9-v1034-paths.txt)
preserves source instruction bytes, file offsets, literal values, the syscall
stub inventory, and branch-search candidates. The earlier counter trace is in
[ios9-entropy-static.txt](ios9-entropy-static.txt).

Default Ghidra analysis missed the AES resource manager and ES signing function.
Their Thumb entries and bounded instruction ranges were restored in the
analysis database at `0x13643e1c..0x13644099` and
`0x20105354..0x20105471`, inclusive. The ELF stayed unchanged. Ghidra still
fails to decode some ARM UDF syscall stubs and does not recover the ES switch
table automatically. The raw instructions and table words supply those edges.
Decompiler warnings remain in the exports; pseudocode types are not original
types.

## OTP to crypto handle 5

The recovered path is:

1. Crypto-object initialization at `0x13649360` calls `0xffff17f0`, with a
   16-byte destination on its stack. The call is at `0x136493d4`.
2. The routine at `0xffff17f0` calls the OTP-content predicate at `0xffff14a4`.
   On the nonzero branch, it calls `0xffff1440(0x1a, destination, 0x10)`.
3. The reader at `0xffff1440` uses OTP command/data registers
   `0x0d8001ec` and `0x0d8001f0`. Word index `0x1a` means byte offset
   `0x68`. The 16-byte read covers offsets `0x68..0x77`.
4. Initialization allocates storage for crypto handle `5`, then calls the
   object-data writer at `0x136496dc`, with that handle, the same stack
   destination, and length `16`. The call is at `0x136494e8`.
5. The AES helper retrieves object bytes through `0x13649818`. Generator
   initialization passes handle `5` to this helper at `0x136435b4`.

The mini project's packed [OTP structure](https://github.com/fail0verflow/mini/blob/master/crypto.h)
also identifies the 16 bytes at offset `0x68` as `rng_key`. This agrees with the
binary data flow; it is not the sole reason for the identification.

On the zero-predicate branch, `0xffff17f0` copies a 16-byte value from the
initialized image at `0xffff8f3a`. This is a built-in fallback branch. It is
not evidence of secure per-console seeding. The predicate examines OTP bytes
`0x14..0x23`; it does not test whether the RNG key itself is nonzero.
No console key values were read for this investigation.

## Generator construction and state

The AES helper at `0x13642eec` obtains a 16-byte object key, builds input/key
and output/IV vectors, and submits IOCTLV `2` through internal AES descriptor
`0x10000`. The AES resource manager at `0x13643e1c` accepts commands `0`, `2`,
and `3`. It loads the key and IV FIFOs, starts the hardware operation, checks
the hardware error bit, and returns completion through the IOS resource reply.
For encryption, it copies the last ciphertext block into the caller's IV.

For one encrypted block, the control word is `0xd0000000`: execute, interrupt,
and AES enable are set; decrypt and continue-IV are clear. The register
addresses and flags match the primary hardware research in
[WiiBrew's AES engine description](https://www.wiibrew.org/wiki/Hardware/AES_Engine).
That engine uses AES-128-CBC. The reached generator path uses this hardware
operation; it does not call an OpenSSL RAND implementation.

The following reconstruction is inferred from the instructions and documented
hardware semantics. It has not been compared with captured console output.
Define `I` as byte `0x01` followed by 15 zero bytes. Define `P(x)` as 12 zero
bytes followed by the big-endian 32-bit value `x`.

```text
At crypto-module setup:
    increment startup counter
    startup_value = read startup counter
    output_counter = 0
    initialized = false

On first generator use:
    session_key = AES128(OTP_rng_key, P(startup_value) XOR I)
    initialized = true

For each GenerateRand request:
    import session_key into a temporary AES object
    previous = I
    for each required 16-byte block:
        output_counter = output_counter + 1 modulo 2^32
        block = AES128(session_key, P(output_counter) XOR previous)
        previous = block
        copy the required bytes to the caller
    delete temporary object
```

The final block is truncated to the requested length. Unused bytes are not
saved for the next request. Each request resets the IV to `I`; the output
counter continues across requests. The block counter wraps after `2^32` block
operations. This corresponds to 64 GiB of generated blocks, with less data
returned if requests discard partial-block tails. There is no wrap rejection
or periodic rekey in the recovered output loop.

The recovered references to `0x13644840` show the session-key clear, derivation,
and import. References to `0x1364489c` show output-counter reset, increment,
and use. No fresh hardware-noise input or runtime reseed was found in these
paths. Unresolved computed accesses remain outside that negative result.

## Failure and restart limits

| Boundary | Observed behavior and implication |
| --- | --- |
| Startup counter increment | Setup at `0x136421e8` ignores the result. It then reads the counter. A failed persistent increment can leave the previous startup value in use. |
| Startup counter read | The reader can return an error code in its result register. Setup stores the result as a counter value without a separate error check. |
| EEPROM write | Two words are written separately. There is no readback in the increment routine. A write-ready poll at `0xffff7404` is bounded and can return `-9`. Power-loss atomicity and recovery are not established. |
| Initial key derivation | The AES result at `0x136435b4` is not checked by its caller. The caller sets the initialized flag at `0x136436dc`. The destination was cleared before derivation; a failure before output is written can therefore leave a known zero session key. This is a static failure-path inference, not an observed console result. |
| Temporary object setup | Creation and key import errors are returned by the generator. |
| Output AES operation | Errors stop the loop and are returned. Earlier output blocks can already have been copied; success must not be assumed after partial output. |
| Temporary object deletion | The return from `0x1364078c` is ignored; the generator returns the earlier operation status. |
| Counter wrap | The 32-bit counter wraps without a rekey or error in the recovered loop. With the same session key and the same counter at a request boundary, the reset IV permits repeated output. |

If the same startup value and OTP key recur, the reconstructed initialization
derives the same session key. Resetting the output counter then permits the
same output sequence for the same request pattern. Normal startup-counter
uniqueness, IOS reload behavior, and failure behavior have not been tested on
hardware. These are acceptance questions, not verified failures on the user's
Wiis.

## Stock IPC and other consumers

The ES signing edge is now recovered from the image:

```text
ES resource loop: IPC request type 7 at 0x20101090
  -> IOCTLV dispatcher 0x201000cc
  -> command table 0x20108744, entry 0x30 at 0x20108804
  -> case 0x201009d6, call at 0x20100a02
  -> signing function 0x20105354
  -> Thumb/ARM veneer 0x201086e0 / 0x201086e4
  -> syscall stub 0x201083d0: UDF word 0xe6000e70
  -> IOS9 syscall 0x73, table target 0x1364376c
  -> signing wrapper 0x13641400 -> signing function 0x1364132c
  -> generator call at 0x136413b8, length 30 bytes
```

The command checks one input vector and two output vectors. The output sizes
are `0x3c` bytes for the signature and `0x180` for the certificate. The handler
hashes the input, obtains signing material, and returns signing results. It
does not return the generator's raw 30-byte buffer. This establishes a stock
PPC-facing consumer, not a raw random-byte transport.

The conventional random syscall stub is ARM word `0xe6000e30` followed by
`BX LR`; this encodes IOS9 syscall `0x71`. A search of all initialized
`PT_LOAD` regions found eight such stubs:

| Module region | Random stub | Recovered use |
| --- | --- | --- |
| `0x13710000` | `0x1373228c` | IP code requests two bytes through a veneer. |
| `0x13cc0000` | `0x13d064d4` | SSL code has two direct consumers through a veneer. One fills a caller buffer in four-byte steps. |
| `0x13d60000` | `0x13d6704c` | NCD helper requests four bytes and calls another routine if the request fails. |
| `0x13da0000` | `0x13dce8b0` | KD code requests 32 bytes and converts their low nibbles into a 32-character hexadecimal value. |
| `0x13e20000` | `0x13e2b834` | WD helper requests four bytes and calls another routine if the request fails. |
| `0x20000000` | `0x20006bdc` | No incoming reference recovered. |
| `0x20100000` | `0x201083c0` | No direct random-stub caller recovered in ES. The signing syscall above consumes random data inside the crypto module. |
| `0x20200000` | `0x20203244` | No incoming reference recovered. |

The branch search covered executable `PT_LOAD` regions. The aligned pointer
search covered all initialized `PT_LOAD` regions. Ghidra references and the
selected callers were also inspected. This is not a proof that no other route
exists: indirect calls, computed addresses, nonstandard syscall sequences, and
full transitive reachability of all network commands remain incomplete.

The AES device accepts supplied key bytes; it does not accept an IOS crypto
object handle in place of those bytes. Its command `2` therefore does not
give PPC access to OTP key handle `5` or to `GenerateRand`.

## Question status and next work

| Question | Status |
| --- | --- |
| Exact image and content integrity? | Answered for the local IOS9 v1034 content. Signature-chain verification was not performed. |
| Generator syscall and function? | Answered: `0x71` -> `0x13643704` -> `0x136435dc`. |
| Seed source for handle 5? | Answered for the nonzero OTP-predicate branch: OTP bytes `0x68..0x77`. The alternate branch uses an image value. |
| Generator construction? | Static reconstruction answered: AES-128-CBC key derivation and counter/feedback output. Console-output comparison remains open. |
| Runtime reseeding? | None found in recovered paths. Complete computed-access coverage and runtime behavior remain open. |
| Stock IPC consumer? | Answered: ES IOCTLV `0x30` signing. Network consumers also exist. |
| Stock raw random-byte IPC? | Unresolved. No usable command was established. |
| Suitable for Rust std? | Not accepted. Transport, error handling, state safety, seed quality, and real-hardware verification remain open. |

The next transport investigation should use the IOS version selected by the
application. Local Dolphin storage also has IOS58 v6176, but that image was not
analyzed in this pass. Do not transfer IOS9 addresses, syscall indices, or
security conclusions to it without inspection. No Wii result has been recorded
for either the RVL-CPU-20 or RVK-CPU-01 entropy path.
