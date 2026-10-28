# libpad Review

Date: 2026-10-01

Status: static C parity review complete; one confirmed bug was fixed. No other confirmed defects remain in the reviewed C-compatible behavior.

## Scope

- `crates/services/ogc-input`
- `crates/services/ogc-input-sys`
- C reference: `C-sources/libogc/libogc/pad.c` and `C-sources/libogc/gc/ogc/pad.h`

## Reviewed behavior

- `PADStatus` layout, status decoding for specs 0 through 2, origin adjustment, stick and trigger clamping.
- Initialization, reset and recalibration state, asynchronous type and origin callbacks, polling, response handling, motor commands, PAD reads, scan state, and button accessors.
- Safe wrapper buffer sizes, raw pointer boundaries, and exported PAD constants and functions.

## BUG FIX

`_PADDisableRecalibration` in C masks interrupts while it reads and updates the recalibration flag in low memory. Rust previously performed the same read and write without masking interrupts. An interrupt could change another bit in the byte between those operations, and the write could discard that change. Rust now uses the shared interrupt guard around the read and write. A regression check covers preservation of the other bits and the returned previous flag.

`PAD_Recalibrate` in the checked-in C source has an empty body and always returns success without recalibrating. Rust keeps the intended reset and calibration path. This is a deliberate bug fix to the C behavior and is marked at the implementation.

## Existing Rust additions

The Rust crates also provide bounded PAD trace diagnostics, debug hooks, and a scan helper that returns decoded status. The Rust sampling callback is invoked by its polling handler; the checked-in C sampling handler is empty. These existing additions were retained as requested.

## Validation boundary

The C and Rust code were compared statically. `cargo xtask check-group input` passed, and the workspace host-test command passed for its nine host-capable packages. The new target-only test code could not be compiled because the bare-metal target does not provide Rust's `test` crate. `cargo xtask check-inventory` is currently blocked by a stale `ogc-gx::get_tex_buffer_size` evidence line associated with separate, uncommitted GX work. No hardware verification is recorded by this review.
