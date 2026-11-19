# GX texture preload commands in Dolphin

Dolphin 2609a on macOS with Metal passed 332 checks with zero failures. All
27 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 24](video_probe/pages/page-024.png).

The probe calls production `preload_entire_texture` for a 4-by-4 RGBA8
texture with and without mipmaps. Fixed BP words from the C reference check
the base transfer, two mip levels, address changes, bank selection, tile
counts, and texture-state flushes. It also checks null pointers, padding,
and buffer guards. The image address is captured and is not read by GX.

The [first run](../2026-11-17-gx-texture-preload/video_probe/probe.log) failed
both command comparisons. The [diagnostic trace](../2026-11-17-gx-texture-preload-trace/video_probe/probe.log)
showed that the preload words matched, but the expected flush mask was zero
while an earlier probe left it at one. The final probe sets a known mask
before capture. No production GX code changed.

This is command-encoding evidence for selected inputs. Actual texture memory
transfers, other formats and sizes, and physical Wii behavior remain open.
HBC was not under test. The runner stops Dolphin after the summary, so
`exit_code` is null. The stderr log has repeated partial-send and alignment
warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-17-gx-texture-preload-fixed --timeout-seconds 180
```
