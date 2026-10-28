# libsi Review

Date: 2026-10-01

Status: static C parity review complete; no confirmed source-level behavior bugs found in the reviewed crates.

## Scope

- `crates/hardware/ogc-si`
- `crates/hardware/ogc-si-sys`
- C reference: `C-sources/libogc/libogc/si.c` and `C-sources/libogc/gc/ogc/si.h`

## Reviewed behavior

- SI register selection, initialization, IRQ registration, and channel type startup.
- Transfer setup and completion, response copying, error reporting, delayed transfers, alarm ownership, queue scheduling, and synchronous completion.
- Polling control, sampling-rate tables, response-line gating, and polling-handler registration.
- Type caching, asynchronous callbacks, controller type decoding, and WaveBird wireless-ID handling.
- The safe Rust wrapper exports and raw-buffer safety boundaries.

The current Rust implementation matches the corresponding C control flow and values in these paths. I found no confirmed source-level behavior mismatch that requires a change.

## Validation boundary

This was a static comparison with the checked-in C sources. It does not establish live Wii or GameCube hardware behavior. No hardware verification is recorded by this review.

## Re-review evidence

Rechecked the complete public operation set against `C-sources/libogc/gc/ogc/si.h` and the matching implementations in `C-sources/libogc/libogc/si.c`. The higher-level `ogc-si` crate forwards each exported operation to `ogc-si-sys` (`crates/hardware/ogc-si/src/lib.rs:15-119`), so it adds no separate controller behavior.

The checked state and register paths align: `SI_Sync` completion and queue restart (`si.c:405-416`; `ogc-si-sys/src/lib.rs:1055-1063`), polling configuration and sampling tables (`si.c:433-501`; `ogc-si-sys/src/lib.rs:1077-1125`), status and response latching (`si.c:503-547`; `ogc-si-sys/src/lib.rs:1128-1177`), command register access (`si.c:549-557`; `ogc-si-sys/src/lib.rs:1179-1194`), and initialization (`si.c:773-798`; `ogc-si-sys/src/lib.rs:1462-1494`). Transfer scheduling, completion, async type callbacks, type decoding, and polling-handler registration were covered in the original review above. I found no additional confirmed mismatch in this re-review. Existing BUG FIX comments in Rust identify prior parity corrections; this review adds no fixes.
