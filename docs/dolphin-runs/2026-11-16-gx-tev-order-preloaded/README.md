# TEV order and preloaded texture scale in Dolphin

Dolphin 2609a on macOS with Metal passed 323 checks with zero failures. All
27 captured pages were valid. The added check loads a 4-by-8 texture into
map 2 through `load_tex_obj_preloaded`, then checks the scale and wrap words
emitted for coordinate 3. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 24](video_probe/pages/page-024.png).

The [prior report](../2026-11-16-gx-tev-order-final/README.md) describes the
original failure, the fix, and direct, indirect, manual, and null-texture
checks. This run adds command-encoding evidence for the selected preloaded
texture. It does not cover every preloaded texture operation or physical Wii.
HBC was not under test. The runner stops Dolphin after the summary, so
`exit_code` is null. The stderr log has repeated partial-send and alignment
warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-tev-order-preloaded --timeout-seconds 180
```
