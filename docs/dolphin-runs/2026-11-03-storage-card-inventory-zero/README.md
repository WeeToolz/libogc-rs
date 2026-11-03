# CARD inventory Dolphin launch attempt

## Build

The command was:

```sh
LIBOGC_MAD_PROBE_MP3=[external tools]/libogc-rs-fixtures/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[external tools]/libogc-rs-fixtures/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-storage-card-inventory-zero
```

The Dolphin probe DOL built. Its SHA-256 is
`124eb10c1c17eae30e7b42493881fdd2c960aab1745a74fef3b6311f817b2a71`.

## Dolphin result

The runner stopped before it launched the DOL. The available Dolphin binary
exited with `SIGABRT` while handling `--version`. This run provides no runtime
evidence for the CARD checks. The emulator must start before the probe can run.
