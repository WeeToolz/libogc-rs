# GX textured rectangle checks in Dolphin

Dolphin 2609a on macOS with Metal passed 266 checks with zero failures.
All 22 captured pages were valid. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), and [identities](identities.json)
record the run. [Page 21](video_probe/pages/page-021.png) shows the probe text;
the rendered pixel comparisons are the EFB readbacks in the probe log.

The probe calls `configure_textured_2d`, `load_tex_obj`, and
`draw_textured_rect` on their production paths. It draws a 32 by 32 rectangle
at (32, 32) from an aligned 4 by 4 tiled RGBA8 texture. It checks pixels at
(40, 40), (56, 40), (40, 56), and (56, 56), and checks that (110, 20)
remains black. The first texture gives red, green, blue, and yellow. After the
texture data pointer changes and the cache is invalidated, the same positions
give magenta, cyan, white, and RGB (64, 128, 192). The fixed ARGB values are
recorded in the log. The probe also checks null objects, an unsupported map,
and zero-width geometry. GX finish events guard the reads and buffer lifetime.
The rectangle helper itself loads map zero, so the rendered checks cover the
combined load and draw path; they do not isolate the direct load call.

The run uses CPU EFB access and the saved Metal wrapper. The runner records
`exit_code: null` because it stops Dolphin after the probe summary. The stderr
log contains repeated partial-send and alignment warnings; the summary and
all page captures were complete. This result does not establish other texture
formats, filtering, maps, boundaries, display-copy output, or physical Wii
behavior. HBC was not under test. No production GX code changed.

To reproduce, set the two MP3 fixture environment variables from
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-textured-render --timeout-seconds 180
```
