# libdsp Review

Date: 2026-10-01

Status: reviewed. Independent static re-review is complete. The reviewed Rust task fields and layout match the current C declarations, and no confirmed mismatches remain in this scope. Target hardware was not tested.

## Scope

- `crates/hardware/ogc-dsp`
- `crates/hardware/ogc-dsp-sys`
- C references: `../../C-sources/libogc/libogc/dsp.c`, `../../C-sources/libogc/gc/ogc/dsp.h`, and the DSP shutdown sequence in `../../C-sources/libogc/libogc/system.c`

The review covers initialization, callbacks, mailbox access, register controls, task queue ordering and removal, boot and execution mailbox sequences, cancellation, rude-task scheduling, wrapper forwarding, and the `DspTask` representation.

## BUG FIX: Preserve volatile task-field access

C declares `state`, `prio`, and `flags` as `volatile u32` in `gc/ogc/dsp.h:115-118`. Commit `1d18ccc` represents each field with `VolatileU32`, a `repr(transparent)` wrapper over `UnsafeCell<u32>`, and uses explicit volatile reads and writes throughout the scheduler and public task operations. This preserves each field's four-byte size and alignment and its C position. I checked all production reads and writes; they use the wrapper accessors.

## BUG FIX: Correct the task-layout assertion

The C task has two address groups. Each group contains one pointer and two `u32` fields (`dsp.h:120-126`). Commit `7204dc0` corrects the expected-size formula to use two `u32` fields per group. On the 32-bit PowerPC ABI, the fields have offsets 0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 38, 40, 44, 48, 52, 56, and 60, and the structure is 64 bytes with four-byte alignment. The Rust declaration uses `repr(C)` and keeps the C field order.

The Rust crate also has defensive null checks and a `shutdown` helper. The C shutdown sequence is in `system.c:736-753`; these Rust additions are not counted as mismatches in the reviewed DSP API.

## Verification boundary

`cargo check -p ogc-dsp -p ogc-dsp-sys --tests --target powerpc-unknown-linux-gnu` passed. This compiles the test code but does not run it. `cargo xtask check-group hardware` currently fails because `ogc-video` has two unused statics under `-D warnings` (`CONSOLE_BUFFER_POOL_START` and `CONSOLE_BUFFER_POOL_END`); the DSP crates checked before that failure. No hardware verification was performed.

## Wii shutdown wait diagnostics — 2026-10-15

The AESND probe run `1792110541-513590000` sent its final report. The user
then observed `INFO exit entering System Menu reset`, with VI frames still
advancing and no reset callback trace. Network cleanup and watchdog cleanup had returned. The reset path calls DSP shutdown before its reset callbacks,
so the DSP shutdown waits are the next hardware checks. This is not evidence
that the System Menu opened.

`shutdown_with_progress` now shares the production shutdown sequence with
`shutdown`. Its optional observer reports the DMA wait, mailbox wait, reset
wait, and completion. The writes and wait conditions still follow C
`libogc/system.c::__dsp_shutdown`. The Wii system reset path forwards these
observations through `SYS_RESET_TRACE_DSP` (stage 7). The observer does not
replace a hardware operation or report a successful shutdown early.

The user's photo from run `1792111703-453368000` shows DSP shutdown stage 2
with control value 2052 (`0x0804`). Stage 2 precedes the C-equivalent mailbox
busy wait on DSP register 2 bit `0x8000`. The control value is the expected
DSP reset and halt bits. The probe did not yet record register 2, so the exact
mailbox value is unknown. The observer now supplies the mailbox word at each
stage, and the reset trace records it at stage 2 as
`SYS_RESET_TRACE_DSP_MAILBOX` (stage 12). The wait condition and reset path
remain unchanged.

The agent DOL build passes. The exact blocking hardware wait is not yet
verified. System Menu return remains open. This addition is diagnostic; it
does not resolve the release requirement for a successful return on the Wii.
