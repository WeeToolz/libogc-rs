# Follow-on parity pass — 2026-10-02

Source reference: uploaded current `libogc-master.zip`. Rust baseline: the uploaded Rust source plus the compile-proven `dvd-keyboard-lwp-ipc-api-parity.diff` batch.

Implemented in this pass:

- `ogc-bt-sync`: L2CAP Configuration Request option accumulation across continuation segments and C-style non-success Configuration Response continuation/retry behavior.
- `ogc-lwp-driver` / `ogc-lwp-sys`: zero-timeout condition semantics, condition-slot destruction/reuse, mutex destruction/reuse, and unchecked C-style timespec conversion.
- `ogc-gcsd`: remaining generic write-protect command encoding and multiwrite-stop token/idle behavior. Existing EXI wait and CRC-delay fixes were verified as already present.
- `ogc-usb-sys`: C's HID-only legacy enumeration fallback and unknown-positive-handle IOS close fallback, including the asynchronous close path.

The changes keep policy centralized: Bluetooth continuation state lives per link/channel, SD command wire formatting uses a shared helper, and USB host selection remains in the v5 layout helper.

Validation in this environment is static only. `git diff --check` is required before handoff; the downstream compile-capable environment should run the full `-D warnings` build and relevant host tests. Bluetooth, SD Gecko, USB, and scheduler edge behavior still require target/hardware validation.
