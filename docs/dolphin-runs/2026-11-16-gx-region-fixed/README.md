# GX texture region invalidation in Dolphin

Dolphin 2609a on macOS with Metal passed 329 checks with zero failures. All
27 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 21](video_probe/pages/page-021.png).

The probe calls production `invalidate_tex_region` with regions from
`init_tex_cache_region`. Fixed BP words from the C reference check the
non-mipmapped and mipmapped cache sizes and both state flushes. Further cases
check an empty odd bank and a null region. The capture checks padding and
buffer guards. The C reference defines the mipmap flag as the first byte of
the fourth word in `GXTexRegion`.

The [before run](../2026-11-16-gx-region-before/video_probe/probe.log) passed
the non-mipmapped case and failed the mipmapped case: 327 passes, one failure.
The implementation read unrelated region fields for that flag. The fix reads
the flag byte and updates the existing test fixture to match the C layout.
The before-run DOL identity is in its [result](../2026-11-16-gx-region-before/video_probe/result.json).

This is command-encoding evidence for selected regions. Cache invalidation
effects on texture sampling and physical Wii behavior remain open. HBC was
not under test. The runner stops Dolphin after the summary, so `exit_code` is
null. The stderr log has repeated partial-send and alignment warnings. All
checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-region-fixed --timeout-seconds 180
```
