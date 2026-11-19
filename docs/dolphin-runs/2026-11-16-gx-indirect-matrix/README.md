# GX indirect texture matrix and tile in Dolphin

Dolphin 2609a on macOS with Metal passed 294 checks with zero failures.
All 25 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 17](video_probe/pages/page-017.png). The page shows probe text.

The probe calls production `set_ind_tex_matrix` and `set_tev_ind_tile` inside
captured display lists. It checks `09080200`, `0a100700`, and `0b700000` for
signed matrix coefficients and exponent, and checks that a null matrix emits
no command. It checks `06c00020`, `07820000`, `08400000`, and `150b82b6` for
tile spacing and wrap fields. The fixed words use the local C reference bit
positions. The capture checks command bytes, padding, and buffer guards.
Source and binary identities are recorded in the identities file.

This is command-encoding evidence. It does not prove rendered indirect
texturing, all matrices or tile sizes, or physical Wii behavior. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log contains repeated partial-send
and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-indirect-matrix --timeout-seconds 180
```
