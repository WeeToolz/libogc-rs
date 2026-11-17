# GX pixel-engine control commands in Dolphin

Dolphin 2609a on macOS with Metal passed 301 checks with zero failures.
All 25 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 20](video_probe/pages/page-020.png). The page shows probe text.

The probe calls production `set_alpha_update`, `set_dither`, `set_dst_alpha`,
and `set_z_comp_loc` inside captured display lists after setting known state.
It checks BP words `41000118`, `4100011c`, `4100010c`, and `41000108` for
alpha and dither changes; `420001a5` and `4200005a` for destination alpha;
and `43000040` and `43000000` for depth comparison location. The capture
checks command bytes, padding, and buffer guards. Source and binary identities
are recorded in the identities file.

This is command-encoding evidence. It does not prove rendered alpha, dithering,
depth comparison effects, all inputs, or physical Wii behavior. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log contains repeated partial-send
and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-pe-control --timeout-seconds 180
```
