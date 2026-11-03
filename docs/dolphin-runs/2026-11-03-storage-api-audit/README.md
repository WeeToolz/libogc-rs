# Storage API audit: Dolphin launch attempt

## Target and expected result

The target was the registered `storage_probe` DOL. The probe should check the
CARD mount, file, persistence, delete, and cleanup paths in a private Dolphin
profile. The run did not request the destructive format path.

## Build

The first build stopped because `LIBOGC_MAD_PROBE_MP3` was not set. The retry
used the local external fixture for both MP3 variables:

```sh
LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-storage-api-audit
```

The DOL built. Its SHA-256 is
`6d8c751d6bbb4a0142c149b62bef140c666be3ab6d6bc5647619d6eb33d700f4`.
The fixture SHA-256 is
`c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`.

## Dolphin result

The runner stopped before it launched the DOL. Dolphin aborted while handling
`--version` with `SIGABRT`. A direct invocation reported that the Qt build
requires the `neon` processor feature. Therefore, this attempt provides no
runtime evidence for the probe or its APIs.

The next Dolphin run requires a Dolphin build that starts successfully on this
host. The storage group check remains separate: `cargo xtask check-group
storage` completed for 15 Wii runtime packages.
