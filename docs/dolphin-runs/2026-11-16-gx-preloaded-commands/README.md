# GX preloaded texture commands in Dolphin

Dolphin 2609a on macOS with Metal passed 325 checks with zero failures. All
27 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 24](video_probe/pages/page-024.png).

The probe calls production `load_tex_obj_preloaded` with an 8-by-4 RGBA8
object and explicit texture region for map 5. Fixed BP words from the C
reference check the texture mode, LOD, size, address, and two region words.
The check also verifies region mutation, buffer guards, and padding. Separate
calls check that null object, null region, and invalid map return errors and
emit no commands. The [earlier TEV order run](../2026-11-16-gx-tev-order-preloaded/README.md)
checks that a preloaded object supplies automatic coordinate scale.

This is command-encoding evidence for selected inputs. Indexed textures, TLUT
callbacks, actual texture sampling through this path, other maps, and physical
Wii behavior remain open. HBC was not under test. No production GX code
changed. The runner stops Dolphin after the summary, so `exit_code` is null.
The stderr log contains repeated partial-send and alignment warnings. All
checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-preloaded-commands --timeout-seconds 180
```
