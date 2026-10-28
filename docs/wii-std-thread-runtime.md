# Wii std runtime entry, threads, and parking

Updated: 2026-10-14. Source and build work is complete for A03, T01–T06,
S05, and the loader argument adapter in P01. The 54 checks in the std thread
probe passed on RVL-101 and in Dolphin. Other service and timing gates remain
open. The user also reports an old thread DOL pass on RVL-CPU-20 (likely RVL-001).
See [the checklist](wii-std-checklist.md) and
[the verification record](wii-std-thread-evidence.txt).

## Runtime entry and normal return

`ogc-sys::_start_rust` shelters the loader strings and argument vector before
BSS is cleared. It publishes that immutable vector before the std startup hook.
An absent loader header gives an empty vector. A recognized but malformed header
causes checked std preparation to fail. The byte copy does not require UTF-8.

The application provider initializes the real system services, checks the Wii
revision and calling context, validates entry metadata, and reserves the System
allocator heap. It publishes `Prepared` only after preparation succeeds. The
compiler-generated main then calls the Wii PAL, which checks the initial LWP
thread and exact argument vector before it publishes `Running`. Repeated entry,
a different thread, or different arguments return an error without changing state.
`args_os` makes an owned copy of the sheltered bytes for each iterator.

After normal Rust main, std performs its stream cleanup and calls the Wii PAL
cleanup. The provider changes `Running` to `Cleaning`, runs final initial-thread
TLS cleanup, and records the actual result. A C entry that uses std reaches the
same cleanup from `_start_rust` after main returns. Completed cleanup can be
checked again; cleanup in progress cannot report success. A failure remains
failed and keeps its error. Only completed cleanup can commit the main return
status and enter `Returned`.

Startup and final cleanup failures remain in diagnostic state. `_start_rust`
does not enter main after failed preparation or call the exit callback after
failed final cleanup. It holds device state. Normal return uses the existing
exit callback. This does not implement `process::exit`, concurrent process exit,
a parent status channel, or complete stream and panic diagnostics. Those
platform contracts remain open.

## Worker ownership and stacks

The std `Thread` adapter transfers one boxed `ThreadInit` to the real LWP
creation path. On creation failure, ownership stays with the caller, the handle
output stays unchanged, and the adapter drops the closure once. On success, the
worker initializes std's current-thread state before it releases `ThreadInit`
and runs the closure. Scoped closures retain the upstream lifetime contract.

A std stack has at least 64 KiB of usable space. The std default is 128 KiB.
Checked arithmetic adds guard and initial frame space, aligns the allocation,
and rejects sizes that exceed the pointer, allocator, or LWP size limits. Std
stacks use the production workspace allocator. A failed stack allocation releases
the reserved native slot. The provider reports `EINVAL` for an invalid size and
`ENOMEM` for a real resource failure. The existing software stack guard remains;
this work does not add an MMU guard page or claim overflow recovery.

LWP records whether the native handle belongs to std or the compatibility API
before it publishes the worker. Compatibility join and detach reject std-owned
handles. Std join consumes its native handle. `JoinHandle` drop detaches it.
Neither path can claim ownership twice. A release store publishes worker writes
and final cleanup. Join and detached reaping check completion with acquire
ordering. Join releases the owned stack before returning the real cleanup error.
The detached reaper runs on another stack and counts completion after stack
release. TLS failure counters remain visible for detached workers.

Rust retains the pinned upstream `ThreadId` allocator and owned `Thread` handle.
The native LWP ID is a separate scheduler handle and can be reused. The public
Rust handle keeps its parker alive after the worker exits. Scheduler names hold
up to 63 UTF-8 bytes and truncate only at a character boundary. A supported,
initialized Wii reports one Broadway hardware thread; unsupported or
uninitialized hardware returns an error.

## TLS and final thread cleanup

The std OS-key adapter uses the existing generational LWP key registry. User TLS
destructors run with the value removed before each call. Values created again
during destruction receive up to four passes. The std runtime cleanup callback
is stored separately from that key table, so it runs after the user destructor
passes. This keeps std's current-thread metadata live during user destruction.

A callback cannot replace a different registered callback. Final cleanup rejects
reentry and preserves its completed result. Automatic worker exit cannot run an
explicitly completed cleanup twice. Remaining destructible values produce a real
cleanup error. Each failed thread is counted once. Join returns `EIO` after a
TLS cleanup failure; it does not convert the error to a successful Rust result.

Explicit final cleanup is unsafe: all user TLS borrows must have ended, and user
code must not access that thread's TLS afterward. The compiler runtime and native
exit path provide this final boundary. A host test cannot prove this hardware
execution order. The probe checks it through real std TLS and driver keys.

## Sleep and parking

Sleep and timed futex waits share `TickBudget`. It rounds fractional ticks up,
keeps long durations in `u128`, and supplies bounded hardware deadlines. Only
actual elapsed time reduces the sleep budget. A normal early wake does not end
sleep early. Cancellation returns an error. Exception context and interrupt
masking are rejected before a std operation blocks.

Wii selects the pinned upstream futex parker. Its 32-bit atomic state retains
one token, coalesces repeated unpark calls, handles timeout races, and provides
release/acquire publication. It uses the existing LWP address-wait transport;
there is no second scheduler or per-parker handle pool. This source selection
does not complete the separate `Instant` and `SystemTime` contracts.

## Probe acceptance

`cargo xtask build-wii-std-probe` builds both std DOLs. The new
`target/xtask/probes/wii_std_thread_probe.dol` uses a normal Rust `main`.
It checks real entry metadata and arguments, worker values and IDs, scoped
borrows, stack bounds, failed spawn capture cleanup, join after TLS destruction,
64 detached stack releases, retained handle lifetime, parker token behavior,
and timeout/unpark races.

The raw ABI checks cover invalid calls, compatibility ownership rejection,
multiple destructor passes, one final callback, and a deliberate destructor
limit failure. That failure uses a static value, so the test does not leak an
allocated TLS object. The probe requires one actual failure count and a join
error. It then checks that the native slot was released.

Main returns status 42. The final callback checks the committed return state,
initial-thread and late-created TLS destruction, and all reported failures. It
writes `WII_STD_THREAD_PROBE_RESULT = 0` only when all checks pass; a negative
value means failure. It holds the result on screen without resetting the Wii.
On 2026-10-14, it completed in Dolphin and on a black Wii Family Edition,
model RVL-101 (USA), motherboard RVK-CPU-01, with 54 passes and no failures.
See [the evidence record](wii-std-thread-evidence.txt).

## Run the thread probe in Dolphin on macOS

Double-click [the Dolphin launcher](../scripts/run-wii-std-thread-dolphin.command)
in Finder, or run it from a terminal:

```sh
./scripts/run-wii-std-thread-dolphin.command
```

The script builds the std probes and opens `wii_std_thread_probe.dol`. It uses
`/Applications/Dolphin.app/Contents/MacOS/Dolphin` by default. Use `--dolphin PATH`
or `DOLPHIN_BIN` to select another executable. Use `--no-build` to run the
existing DOL. Use `--help` for the options.

Each run creates a separate profile under `target/dolphin-std-thread/`. That
directory holds the build log, Dolphin console output, Dolphin's own log under
`user/Logs/`, and `run-info.txt` with the executable version, source commit,
working tree changes, and DOL hash. The script uses the shared framebuffer
readback settings and runs at normal emulation speed with one CPU thread.

Read the probe's final PASS/FAIL summary on screen, then close the Dolphin
window. The launcher's exit code reports a build, launch, emulator, or log
capture failure. It does not read the probe result or certify a pass. A hang or
missing final summary leaves the result incomplete. Dolphin execution does not
close real-Wii acceptance checks.
