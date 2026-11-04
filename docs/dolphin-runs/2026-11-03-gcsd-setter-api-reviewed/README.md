# SD Gecko API audit rerun preflight

## Result

The fresh `fat_sd_probe` validation did not start the DOL. The Dolphin runner
stopped when its `--version` preflight exited with signal 6. A direct arm64
version check also exited with status 134 and printed:

```text
Incompatible processor. This Qt build requires the following features:
    neon
```

The host reports Darwin 27.2.0 and `arm64`. The Dolphin executable is a
universal binary. The cause of this preflight failure is unknown. This result
does not change the previous completed run at
[`2026-11-03-gcsd-setter-api-final`](../2026-11-03-gcsd-setter-api-final/suite.json),
which passed 45 checks. This rerun provides no new probe evidence.

## Commands

The DOL build completed before the runner preflight:

```sh
LIBOGC_MAD_PROBE_MP3=${WIIDEV_ROOT}/Devtools/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask build-dol ogc-system-probe --bin fat_sd_probe \
  --probe-features dolphin-automation --no-dev \
  --output target/xtask/probes-dolphin/fat_sd_probe.dol
```

The validation command was:

```sh
cargo xtask dolphin-validate --no-build --probe fat_sd_probe \
  --dolphin ${DOLPHIN_BIN} \
  --artifact-dir docs/dolphin-runs/2026-11-03-gcsd-setter-api-reviewed
```

The runner returned status 1 with:

```text
Error: "${DOLPHIN_BIN} [\"--version\"] exited with signal: 6 (SIGABRT)"
```

The direct version check was repeated with `arch -arm64` and returned status
134 with the same Qt processor message. The observation time was
`2026-11-04 01:26:10 UTC`. No Dolphin probe process started and no probe
artifacts were produced by this attempt.
