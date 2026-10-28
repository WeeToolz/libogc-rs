# USB storage close policy independent audit

Reviewer: `thread-current/reviewer_4`  
Scope: `ogc-usbstorage-lifecycle` flush-before-close policy only. Other USB-storage findings were not re-reviewed.

## Finding confirmed

The existing `docs/review/libusbstorage.md` finding is supported by both close paths:

- C `USBStorage_Close` in `../../C-sources/libogc/libogc/usbstorage.c:552-573` resets global mount bookkeeping, closes the USB device when open (ignoring the return value), destroys the mutex and alarm, frees sector-size and transfer buffers, clears the handle, and returns zero. It does not issue a SCSI cache synchronization or flush command.
- Rust `ogc-usbstorage-lifecycle::close` at `crates/services/ogc-usbstorage-lifecycle/src/lib.rs:12-32` calls `flush_lun` for each mounted LUN before `close_transport`. If any flush fails, it returns that error before transport close.
- The Wii backend's lifecycle adapter at `crates/services/ogc-usbstorage-sys/src/wii_backend.rs:995-1033` maps `flush_lun` to `synchronize_cache_locked` and calls `usb::close_device` only through `close_transport`. An error from the lifecycle policy returns at line 1019 before handle/slot cleanup at lines 1023-1031.

Therefore Rust adds a close-time flush and can leave the handle and transport open when the flush fails; C always proceeds with resource cleanup and returns success. This is a confirmed observable lifecycle difference. The exact consequences of skipping the flush depend on the device's cache behavior and were not tested here.

## Result and validation

This independent audit confirms the existing `ogc-usbstorage-lifecycle` finding. No implementation files were changed. No builds or tests were run. No hardware verification was performed.
