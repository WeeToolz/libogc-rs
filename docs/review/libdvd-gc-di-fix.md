# GameCube DVD DI transport implementation

Implementer: `/root/worker_c`  
Scope: add direct-register GameCube DI operations in `ogc-dvd-sys`, while retaining the existing Wii `/dev/di` path.

## Changes

The GameCube path now selects DI registers at `0xCC006000` and issues read, seek, request-error, motor, disc-ID, and inquiry commands. It polls command completion, applies cache invalidation for DMA buffers, and routes DVD mount, read, seek, drive-control, reset, and shutdown operations through this backend when the runtime Hollywood revision is zero. Compile-time visibility and selection remain runtime-based because this workspace does not define separate GameCube and Wii build modes.

Regression coverage checks the C DI read command register words and maps C drive status codes to DVD states.

## Remaining gaps

This is partial transport parity. The C implementation in `C-sources/libogc/libogc/dvd.c` installs `__DVDInterruptHandler` on `IRQ_PI_DI` and unmasks the interrupt. The Rust implementation polls the DI start bit and leaves DI interrupt masks disabled. It therefore does not reproduce C interrupt-driven callback timing, interrupt-delivered cover-close events, or its timeout/alarm flow. Error handling is simplified and does not implement the full C retry and partial-transfer state machine.

The C GameCube mount path also includes drive unlock sequences, drive-model and firmware checks, extension setup, and patch code. The Rust mount path only resets, waits, spins up, checks drive status, and reads disc ID and inquiry data. These behaviors remain unimplemented and require independent review before this crate can be marked complete.

## Verification

- `cargo check -p ogc-dvd-sys --target powerpc-unknown-linux-gnu` passed.
- `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed. This compiles the target-only regression tests; it does not run them.
- `cargo check -p ogc-dvd --target powerpc-unknown-linux-gnu` passed.
- No hardware verification was performed.
