# Runtime parity batch — 2026-10-02

Source reference: current uploaded libogc C tree. Validation in this environment is static because the Rust toolchain is not installed.

## Implemented

- `lwip-sys`: public `socklen_t` parity (`u32`) and removal of signed forwarding conversions; nearby socket ABI types rechecked.
- `wiikeyboard-sys`: C-style automatic 10 ms polling, 300-poll discovery retry, and lock-key modifier/LED behavior.
- `iso9660`: read-only devoptab registration plus file, seek, stat, directory, chdir-validation, statvfs, and device removal operations. Shared newlib filesystem ABI structs now live in `ogc-sysbase` and are reused by FAT and ISO9660.
- `aesnd-sys`: working software voice mixer, all eight source formats, frequency/delay/loop/stream/once handling, callback flow, mixer-time metrics, cache flush, and GameCube default-frequency parity.

## Verified as already present

`wpad-sys` already has the requested six-channel capacity, channel-5 acceptance, six-channel all-device dispatch, and handshake-gated `WPAD_Probe` behavior. No duplicate implementation was added.

## Remaining boundaries

- Keyboard function-key symbols and unbounded event-queue behavior are still open.
- ISO9660 does not write exact newlib `_reent->_errno` values because `ogc-sysbase::Reent` is intentionally opaque.
- AESND uses a CPU mixer instead of the C DSP program, so exact DSP interpolation/timing and hardware playback require probes.
- WPAD and all hardware-facing changes still require target/hardware validation.
