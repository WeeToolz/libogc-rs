# IPC cancellation regression after vector range guard

Date: 2026-11-27. Dolphin 2609a. Probe: `ios_reload_probe`.

The probe passed **112 checks with zero failures**. The final DOL SHA-256 is
`779ca304903f193e0e2bba88f5b04b6451cc73053e46f73c3e381ea22deb9721`.
The runner used a private Dolphin user profile and NAND copy. The source tree
was at `ff74a0dbe23a24f4bdaa2b0d196742132693ac71` with the vector range guard
under test.

This run rechecked the production IOS replacement path. It held real raw and
formatted asynchronous requests, replaced IOS, delivered each cancellation
once, restored request and deferred callback capacity, avoided formatter
output copyback, and accepted a fresh request. The full probe transport is in
[`probe-transport.bin`](ios_reload_probe/probe-transport.bin), SHA-256
`c06015a949bd20e5e8f5fed2adef40f31a9fbe8234fa8436af2dcff8dd59d742`.

The final run used:
`cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 180`.

This run did not submit an actual zero-vector request. The zero-vector range
branch is covered by the host contract test in `tests/ogc-ipc-ioctl-buffers.rs`.
Physical Wii cache, DMA, and IOS replacement behavior remain unverified.
