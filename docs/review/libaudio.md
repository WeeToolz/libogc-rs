# libaudio Review

Date: 2026-10-01

Status: static review of the AUDIO API implementation is complete. Two confirmed C parity bugs are fixed. Target hardware was not tested.

## Scope

- `crates/hardware/ogc-audio`
- `crates/hardware/ogc-audio-sys`
- C reference: `C-sources/libogc/libogc/audio.c` and `C-sources/libogc/gc/ogc/audio.h`

## Reviewed behavior

- Initialization and callback registration, including GameCube stream interrupt setup.
- Stream volume, sample rate, trigger, play state, and sample counter operations.
- DSP sample rate and source conversion timing.
- DMA setup, start/stop, status, byte count, length, and address operations.
- Safe wrapper forwarding and snapshot construction.

## BUG FIX

`AUDIO_InitDMA` in C disables interrupts while it updates three DSP registers. Rust performed the related read/modify/write operations without that guard, so an interrupt could occur while the transfer configuration was only partly updated. Rust now holds `ogc_sys::InterruptGuard` for all three updates.

Rust used the Wii AI register base `0xCD006C00` on both consoles. C uses `0xCC006C00` on GameCube and `0xCD006C00` on Wii. Rust now selects the base from the detected Hollywood revision, with a target test for both addresses.

The C implementation compiles the stream callback handler only for GameCube. Rust exposes callback registration on Wii but does not install a Wii handler. The C header declares this API unconditionally, so the Wii behavior remains an uncertain scope difference.

## Validation boundary

`cargo xtask check-group audio` passed, and `git diff --check` passed. The register-base selection test is PowerPC-gated. Full-file rustfmt checks report existing formatting differences in unchanged code. Target hardware behavior was not tested.
