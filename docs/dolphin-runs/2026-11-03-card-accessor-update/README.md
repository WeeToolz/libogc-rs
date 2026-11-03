# CARD accessor probe update

This run builds the GameCube CARD probe after adding a mount-request slot check,
direct `ogc-card-sys` calls, and exact receiver paths for CARD wrapper calls.
The probe checks invalid-channel sector-size and create results. It also runs
raw transfers through both sys and wrapper calls. This report has no new
Dolphin runtime result.

## Build

Command:

```sh
LIBOGC_MAD_PROBE_MP3=<external Layer III fixture> \
LIBOGC_MAD_REFERENCE_MP3=<verified reference fixture> \
cargo xtask build-dolphin-probes
```

The reference fixture SHA-256 matched the required value in
`docs/dependencies.md`. The build completed and wrote
`target/xtask/probes-dolphin/card_probe.dol` with SHA-256
`c065aac180ba99524bd9cc5e50c55d379fffa6668b1c659ed7efff3a8ca75e0d`.

## Dolphin result

Command:

```sh
cargo xtask dolphin-validate --probe card_probe --no-build \
  --dolphin <Dolphin executable> \
  --artifact-dir target/dolphin-validation/card-accessor-update
```

Dolphin exited before launch with `SIGABRT`. Running the executable with
`--version` reported that its Qt build requires NEON, which this host does not
provide. The DOL did not run, so this update has no new CARD runtime checks.
The prior 93-check GameCube virtual-card run remains historical evidence only.

## Other checks

- `cargo xtask check-inventory` passed with 3,388 entries.
- `cargo xtask build-dolphin-probes` passed.
- Strict target Clippy passed for all `ogc-system-probe` binaries.
- Physical GameCube card behavior remains untested.
