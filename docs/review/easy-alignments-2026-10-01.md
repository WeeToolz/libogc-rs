# Easy C parity alignments — 2026-10-01

This pass used the current uploaded Rust tree and uploaded `libogc-master` C source. It intentionally targeted small, direct source mismatches rather than large missing subsystems. No Cargo toolchain or hardware execution was available in this environment.

## `ogc-usbgecko`

- Removed the Rust-only elapsed-time limits from EXI lock acquisition and USB Gecko transfer/flush loops. C can wait indefinitely; Rust now does the same at the API behavior level while yielding between failed lock/ready attempts.
- Moved the 512 KiB flash boundary check into each flash read/write command (`address > 0x7FFFF`), matching C. This restores C's partial-prefix behavior for requests that cross the end of flash.
- Replaced the scheduler-only yield after a byte-program command with a 20 microsecond tick-based delay before verification, matching C `usleep(20)`.
- Used wrapping `u32` address advancement so overflow follows C unsigned arithmetic instead of debug-build overflow semantics.

## `iso9660`

- Added unmangled `extern "C"` exports for `ISO9660_Mount`, `ISO9660_Unmount`, and `ISO9660_GetVolumeLabel`, forwarding to the existing Rust implementations.
- Added a compile-time function-pointer signature test for those C entry points.
- The much larger devoptab/filesystem-device implementation gap remains open.

## Validation status

Static source comparison and whitespace checks only. A build-capable agent should run formatting, crate checks, tests, and target builds. USB Gecko timing still needs real hardware validation.

## `wiikeyboard-sys`

- The USB removal callback now always reaches the USB-layer event callback after `USBKeyboard_Open`, even when the higher-level keyboard layer was not initialized. The high-level event queue remains correctly gated.

## `ogc-usb-sys`

- `USB_GetAsciiString` now requests 255 descriptor bytes like C, applies the caller size only to output, and preserves any low byte whose UTF-16 high byte is zero.
- Added public `USB_MAXPATH = 64`, matching C `IPC_MAXPATH_LEN`.
- The defensive null/zero-length output guard remains intentional.
