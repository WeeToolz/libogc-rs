# USB storage API checks without a device

## Result

Dolphin 2609a passed `usbstorage_capacity_probe` with 52 checks and no
failures. The DOL SHA-256 is
`7c0acfd5ce7bdd19ac97b84e51d414cc65b3a1c02947bad8c3bc67ccebd38910`.

The run checked safe Rust results, raw Rust status codes, and C ABI results
without a USB mass-storage device. It checked error mapping, output and handle
state preservation, no-mounted-drive flush, per-slot flush errors, C
deinitialization status, and shared BOT/SCSI result rules. It did not send a
successful transfer to a USB device.

The run used the current probe source from revision `90c2192` with local probe
changes. The DOL hash identifies the tested binary. The full run data is in
[`suite.json`](suite.json) and the saved log is
[`usbstorage_capacity_probe/probe.log`](usbstorage_capacity_probe/probe.log).
I inspected the final page; it shows the passing summary.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified
external fixtures, then run:

```sh
cargo xtask dolphin-validate \
  --probe usbstorage_capacity_probe \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir docs/dolphin-runs/2026-11-03-usb-capacity-api-current
```

See the [USB API audit](../2026-11-03-ogc-usbstorage-api-audit.md) for the
assertion-to-API map and remaining device-backed checks.
