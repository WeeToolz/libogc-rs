# USB storage close flush-failure fix handoff

Implementer: `/root/worker_b`. Independent reviewer: `/root/reviewer_4` (re-review pending).

## BUG FIX

C `USBStorage_Close` continues resource cleanup and closes the USB transport without a close-time flush. Rust retains its added mounted-LUN flush step, but a flush error no longer exits the close policy. The policy attempts flushes for all mounted LUNs, then calls `close_transport`; when that succeeds, it returns success so the caller releases the backend slot and clears the handle, matching C's close result on this path.

Regression coverage checks that one or more flush failures still lead to transport close and successful policy completion. The separate transport-close-error behavior remains unchanged.

## Validation

Validation: `cargo test -p ogc-usbstorage-lifecycle` passed all 6 tests. `RUSTFLAGS="-D warnings" cargo check -p ogc-usbstorage-sys --target powerpc-unknown-eabi` passed and compiled the lifecycle adapter. Independent static review is pending. No hardware verification was performed.
