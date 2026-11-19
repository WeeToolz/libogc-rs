# GX indirect TEV helpers in Dolphin

Dolphin 2609a on macOS with Metal passed 291 checks with zero failures.
All 24 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 17](video_probe/pages/page-017.png). The page shows probe text.

The probe calls production `set_tev_ind_bump_st`, `set_tev_ind_bump_xyz`,
`set_tev_ind_repeat`, and `set_tev_ind_warp` inside captured display lists.
It checks BP words `1406ca32`, `1516d232`, `16100002` for the three-stage
bump operation; `17000671` for XYZ bump; `1816c000` for repeat; and
`1906c472`, `1a000402` for warp with both options enabled and disabled.
The expected fields use the local C reference bit positions. The capture
checks command bytes, padding, and buffer guards. Source and binary identities
are recorded in the identities file.

This is command-encoding evidence. It does not prove rendered bump or warp
effects, all inputs, or physical Wii behavior. HBC was not under test. No
production GX code changed. The runner stops Dolphin after the summary, so
`exit_code` is null. The stderr log contains repeated partial-send and
alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-indirect-helpers --timeout-seconds 180
```
