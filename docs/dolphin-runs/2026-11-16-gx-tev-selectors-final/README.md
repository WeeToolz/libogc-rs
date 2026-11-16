# GX TEV selector commands in Dolphin

Dolphin 2609a on macOS with Metal passed 283 checks with zero failures.
All 24 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 16](video_probe/pages/page-016.png). The page shows probe text.

The probe calls `set_tev_k_color_sel`, `set_tev_k_alpha_sel`,
`set_tev_swap_mode`, and `set_tev_swap_mode_table` on the production paths.
It checks BP words `f8000009`, `f900000f`, `f8000129`, `f8000b29`,
`f806cb29`, and `f86ecb29` for a table and even and odd stage selectors.
It checks `cb00000e` for raster and texture swap selection. The expected
fields follow the local C reference; its hash is in the identities file.
The capture checks the command bytes, padding, and buffer guard.

The [first run](../2026-11-16-gx-tev-selectors/suite.json) had 282 passes
and one failed selector check. Its setup left the selectors for stages 6 and
7 at their nonzero GX initialization values; they share the second table
word. The final probe explicitly clears those fields before capture, while
retaining the expected words. The rebuilt DOL passed. The earlier failure is
retained and is not counted as a pass.

This is command-encoding evidence. It does not prove the rendered effect of
these selectors, every input value, or physical Wii behavior. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr logs contain repeated partial-send
and alignment warnings. Both runs captured valid pages.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-tev-selectors-final --timeout-seconds 180
```
