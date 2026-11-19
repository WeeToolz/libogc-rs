# GX fog commands in Dolphin

Dolphin 2609a on macOS with Metal passed 312 checks with zero failures.
All 26 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 18](video_probe/pages/page-018.png).

The probe calls production `set_fog` and `set_fog_range_adj` in captured
display lists. Fixed expected BP words from the C reference check perspective
and orthographic coefficients, a degenerate perspective interval, RGB color,
range-table packing and masking, center wrap, and disabling with a null table.
The capture also checks padding and buffer guards. Source, binary, and fixture
identities are in the identities file.

This is command-encoding evidence for selected inputs. Rendered fog, other
inputs, and physical Wii behavior remain open. HBC was not under test. No
production GX code changed. The runner stops Dolphin after the summary, so
`exit_code` is null. The stderr log contains repeated partial-send and
alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-fog --timeout-seconds 180
```
