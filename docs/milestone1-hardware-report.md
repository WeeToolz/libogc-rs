# Milestone 1 hardware report template

Use this after building and running `target/xtask/probes/lwp_probe.dol` on a real Wii. Attach the complete probe log rather than only the summary line.

## Build environment

- Host OS:
- Rust nightly date/version:
- `cargo xtask milestone1-verify` result:
- `cargo fmt --all --check` result:
- Commit/archive SHA-256:

## Wii environment

- Wii model/revision:
- System Menu region/version:
- IOS/cIOS environment:
- Loader and launch method:
- SD/USB logging device:
- Number of repeated probe runs:

## Required artifacts

- Visible-console transcript or photograph:
- `/tmp/ogc-system-probe.log`:
- Mounted probe log, when available:
- Final scheduler counters:
- First failing record, if any:

## Expected probe sections

- `LWP SWITCH/JOIN STRESS`
- `LWP SYNCHRONIZATION`
- `LWP TIMED WAIT`
- `LWP QUEUE/DEFERRED`
- `TUXEDO OVER LWP` (thread/queue and tick-task delivery)
- `IPC INTERRUPT WAKE`
- `LWP DEBUG HOOKS`

## Acceptance observations

Record the measured values rather than only pass/fail.

- Worker A iterations:
- Worker B iterations:
- Context-switch delta:
- Join return values:
- Timed-wait return code:
- Timed-wait ticks:
- Semaphore wake result:
- Deferred callback count:
- Synchronous IPC wake delta:
- Low/high-priority serialized IPC callers both completed:
- Async IPC callback thread handle:
- Async IPC callback/close result:
- Stack-guard failures:
- Dropped debug events:
- Dropped deferred callbacks:

## Failure notes

Record the last visible boot marker before the last probe line:

- `BOOT 1/3` means VI initialized;
- `BOOT 2/3` means the probe entered IOS pre-main;
- `BOOT 3/3` means IOS pre-main returned.

For a hang, record the last visible probe line and the last eight drained scheduler events. For a crash, include exception type, SRR0/SRR1, LR, stack pointer, and the active probe section when available. A `FATAL: probe panic` screen confirms the Rust panic handler ran after console initialization. When a debugger or emulator inspector is available, also record `OGC_PROBE_BOOT_STAGE`, `lwp_probe_debug_hook_count`, and `lwp_probe_debug_last_event`.
