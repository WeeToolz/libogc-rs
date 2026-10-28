# Remaining Validation Notes

Date: 2026-07-02

## Hardware validation

- `ogc-di`: validate `/dev/di` identify, reset, motor, partition open/close, encrypted read, unencrypted read, BCA, report key, and error paths on a real Wii.
- `iso9660`: validate mounts against real disc images and malformed image fixtures once DI-backed media reads are exercised.
- `aesnd` and `asnd`: validate callback timing, DMA cadence, looping, channel reuse, and audible output on hardware.
- `modplay`: validate tracker playback through the Wii audio path, including timing, looping, effects, and callback behavior.
- `wpad-sys`: validate expansion and report sequencing on real controllers and extensions.
- `tinysmb-sys`: validate long-running sessions, reconnect behavior, handle exhaustion, and error propagation against real SMB servers.

## Post-validation release decisions

- Decide the public ABI snapshot policy before publishing external stability guarantees.
- Decide which wrapper crates receive semver guarantees first.
- Document host-model behavior as test compatibility unless hardware validation proves an equivalent Wii contract.

## Post-validation cleanup

- Review each PowerPC-only `allow(...)` added for strict builds and classify it as accepted compatibility debt or required refactor work.
- Replace `static_mut_refs` allowances with raw-pointer or interior-mutability designs where doing so improves safety without breaking libogc compatibility.
- Remove unreachable-code allowances where cfg structure can express the same target split cleanly.

## Post-validation build and packaging

- Decide whether probe/example allocators should stay centralized through `ogc-system-probe` or be owned explicitly by each binary.
- Decide whether `build-dols` with `RUSTFLAGS='-D warnings'` is a permanent release gate for all target crates.
- Decide how hardware-validation commands should be represented in `xtask` once a repeatable Wii test path exists.
