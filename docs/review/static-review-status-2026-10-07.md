# Static parity status — 2026-10-07

This is a historical snapshot. Its totals and handoffs describe that date.
For current status, run `cargo xtask review-status` and read
[the implementation status](../implementation-status.md).

This snapshot follows `docs/review/REVIEW-WORKFLOW.md`. The ledger is a static C/Rust source and ABI parity ledger. Hardware and runtime acceptance are separate validation and do not by themselves keep a crate in `needs_re_review`.

## Snapshot totals

- Workspace crates: **83**
- `reviewed`: **79**
- `needs_re_review`: **4**
- `parity_blocked`: **0**
- `review_incomplete`: **0**

## Re-reviews accepted in this pass

The following 25 crates passed the fresh source re-review and moved from `needs_re_review` to `reviewed`:

`asnd-sys`, `iso9660`, `libdb-sys`, `lwbt-sys`, `lwip-sys`, `ogc-alloc`, `ogc-dvd`, `ogc-dvd-sys`, `ogc-fat`, `ogc-gcsd`, `ogc-gx`, `ogc-gx-sys`, `ogc-ipc`, `ogc-lwp`, `ogc-lwp-driver`, `ogc-lwp-sys`, `ogc-network`, `ogc-network-sys`, `ogc-system-sys`, `ogc-usb`, `ogc-usb-sys`, `ogc-usbstorage`, `tinysmb-sys`, `tuxedo-sys`, and `wiikeyboard-sys`.

No confirmed non-safety C-parity mismatch remains in the reviewed source/ABI scope for these crates. Documented safety-only deviations remain accepted.

## Additional re-reviews completed

`ogc-es` and `ogc-es-sys` passed the raw IOS result review. `ogc-ios` and `ogc-ios-sys` passed the cache invalidation sequence review. `wpad-sys` passed the idle-threshold behavior review. The PowerPC target checks compiled test code with `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn'`; tests did not run.

## Fix handoffs awaiting independent re-review

The AESND borrow split, MAD bit-length conversions, and USB-storage close-error propagation fixes compile or pass their focused tests as recorded in their review notes. All four rows (`aesnd`, `aesnd-sys`, `mad-sys`, and `ogc-usbstorage-sys`) are `needs_re_review` until an independent reviewer accepts the complete fix diffs.

USB host-side unit tests ran for `ogc-usbstorage-lifecycle`; the `ogc-usbstorage-sys` test code compiled for PowerPC but could not run without target hardware/runtime. No hardware verification is claimed.

## Runtime follow-up

PowerPC compilation, Dolphin execution, Wii hardware timing, callback races, physical media, USB devices, Bluetooth controllers, network adapters, and audio output remain useful acceptance tests. They are tracked separately from the static parity result because the repository workflow explicitly places hardware verification outside the static review ledger.
