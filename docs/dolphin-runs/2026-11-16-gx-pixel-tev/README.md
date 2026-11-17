# GX pixel format and TEV default commands in Dolphin

Dolphin 2609a on macOS with Metal passed 317 checks with zero failures.
All 26 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 23](video_probe/pages/page-023.png).

The probe calls production `set_pixel_fmt` and `set_tev_op` in captured display
lists. Fixed BP words from the C reference check RGB565 format and deferred
multisample state, V8 format and YUV field selection, and modulate and replace
TEV defaults on a nonzero stage. An invalid pixel format emits no command;
this is a Rust safety difference from the unchecked C lookup. The probe sets
known state before capture. It also checks padding and buffer guards. Source,
binary, and fixture identities are in the identities file.

This is command-encoding evidence for selected inputs. Rendered effects, other
formats and operations, and physical Wii behavior remain open. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log contains repeated partial-send
and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-pixel-tev --timeout-seconds 180
```
