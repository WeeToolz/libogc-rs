# Aggregate Dolphin verification: network and audio corrections

## Result

- Dolphin build: `Dolphin df4bd44`, Metal backend
- Aggregate DOL: SHA-256 `47539b8634049b3b1459c5f4bf795b81c484c00501c8f09c90c5650a6a9e9ba7`
- Result: **1,896 passed, 0 failed**
- Page captures: 113
- Machine result: [`suite.json`](suite.json)
- Probe transcript: [`probe.log`](ogc_system_probe/probe.log)
- Probe result: [`result.json`](ogc_system_probe/result.json)

This run verifies the corrected network startup and error handling, DHCP addresses, numeric loopback resolution, raw UDP socket creation, stopped audio timing, ASND buffering, and playback callbacks. The suite also includes the earlier IOS, USB, GX, and DVD checks.

The audio counter value after `AIResetStreamSampleCount` remains nonzero in Dolphin and stays unchanged after the stream stops. The C and Rust APIs issue the same reset register write. This run checks stopped-state stability and records the counter; Dolphin does not verify that the hardware counter reset itself. Physical Wii evidence for this register behavior remains pending.

## Reproduction

From the repository root, build the development DOL:

```sh
cargo xtask build-dol ogc-system-probe --bin ogc-system-probe --dev --probe-features dolphin-automation
```

Then run the aggregate using the patched Dolphin Metal wrapper:

```sh
DOLPHIN_METAL_BIN="$PWD/target/dolphin-usb-build/Binaries/DolphinQt.app/Contents/MacOS/DolphinQt" \
LIBOGC_DOLPHIN_DEV_DOL="$PWD/target/xtask/custom-dols/ogc_system_probe-ogc-system-probe-dev.dol" \
LIBOGC_DOLPHIN_DEV_ELF="$PWD/target/xtask/dev-dol-build/powerpc-nintendo-none/release/ogc-system-probe" \
cargo xtask dolphin-validate --no-build --probe ogc_system_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir docs/dolphin-runs/2026-11-26-network-corrected \
  --timeout-seconds 360
```

The Dolphin run does not replace physical Wii checks. Physical Wii boot recovery, SD log persistence, wrapper report delivery, audio output, and the AI sample-counter reset remain pending.
