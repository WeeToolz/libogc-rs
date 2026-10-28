# Workspace architecture

Milestone 0 established implementation ownership and dependency direction. Milestone 1 applies that structure to the core scheduler.

## Layers

The authoritative package metadata is `config/crate-roles.csv`.

1. **ABI** — C-compatible types, constants, and layout support.
2. **Core** — PowerPC primitives, interrupt/IPC/IOS foundations, scheduler, timers, and shared contracts.
3. **Hardware** — EXI, SI, DSP, ARAM, audio, video, GX, and GU implementations.
4. **Services** — storage, USB, networking, Bluetooth, input, filesystems, and system services.
5. **Media** — decoders, players, and mixers.
6. **Compatibility** — historical libogc ABI facades and aggregate wrappers.
7. **Probe/tool** — target diagnostics and host repository tooling.

A package may depend on the same layer or a lower layer. Upward dependencies fail `cargo xtask check-dependencies` unless a reviewed temporary exception exists. Milestone 1 removes the prior SI-to-system exception; `config/dependency-exceptions.csv` currently contains only its header.

## Dependency direction

```text
ABI
  ↓
core runtime and synchronization
  ↓
hardware buses and IOS transports
  ↓
device and service implementations
  ↓
media and compatibility facades
  ↓
examples and probes
```

Rules enforced by tooling:

- every package has one recorded layer, role, ownership group, and host policy;
- core packages may not acquire dependencies on hardware, services, media, or compatibility packages;
- hardware packages may not acquire dependencies on services, media, or compatibility packages;
- compatibility packages may depend on implementations; implementations may not depend on compatibility packages;
- new exceptions require reason, owner, and removal milestone;
- stale exceptions fail the dependency check.

## Scheduler ownership

```text
ogc-core
  architectural timebase and low-memory primitives
        ↓
ogc-runtime
  interfaces and detailed internal errors
        ↓
ogc-lwp-driver
  only scheduler, ready selection, contexts, waits, timers, deferred worker
        ↓
ogc-lwp-sys / ogc-lwp / tuxedo-sys
  ABI and safe compatibility facades
```

`ogc-ipc` depends on the LWP facade to block synchronous callers and to wake them from IOS completion interrupts. It does not own a second scheduler.

## Crate roles

Large subsystems converge on:

```text
ogc-foo-driver   private Wii implementation and resource ownership
ogc-foo-sys      C ABI translation and thin forwarding
ogc-foo          safe Rust ownership, lifetimes, synchronization, and errors
```

`ogc-lwp-driver`, `ogc-lwp-sys`, and `ogc-lwp` are the first completed use of this pattern. `tuxedo-sys` is a compatibility-only consumer.

## Shared runtime contracts

`ogc-runtime` is a target-only no-std interface crate. It contains no device implementation and no compatibility return-code mapping.

It defines:

- `WiiError` with retained IOS, EXI, USB, and network status;
- `Lifecycle` and `SubsystemState`;
- `BlockDevice` and `AsyncOperation`;
- `DebugTransport` and `HidTransport`;
- `MonotonicClock` and `TimerService`;
- `DeferredExecutor` and `InterruptWake`;
- callback context, wake tokens, deadlines, and thread identifiers.

The LWP driver implements the clock, timer, deferred-executor, and interrupt-wake contracts. Device crates should use those interfaces rather than creating independent deadline queues or fake callback state.

## Callback rules

Hardware and IOS interrupt handlers may update bounded scheduler state and enqueue work. Application callbacks that can allocate, call IOS, or acquire application locks run through the LWP deferred worker. Each subsystem must document any callback that remains intentionally interrupt-context.

## Host policy

Runtime packages reject ordinary non-PowerPC builds. Host support is limited to packages marked `abi-only` or `tool` in `config/crate-roles.csv`.

Host-side code may inspect ABI, generate code, audit source, test pure protocol parsers, and build documentation. It must not claim successful Wii device operation.
