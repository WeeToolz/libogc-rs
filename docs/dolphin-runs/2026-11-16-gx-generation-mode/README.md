# GX generation mode commands in Dolphin

Dolphin 2609a on macOS with Metal passed 307 checks with zero failures.
All 26 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 22](video_probe/pages/page-022.png).

The probe calls seven production APIs in captured display lists. It checks
front and back cull mapping and the invalid-input guard; co-planar enable and
disable with BP masks; channel, texture generator, TEV, and indirect stage
counts and their deferred BP and XF writes; and line and point texture
offsets, including masked inputs and preservation of the coordinate size.
The expected command words come from the C reference. The capture also checks
padding and buffer guards. The probe sets known state before capture. Source,
binary, and fixture identities are in the identities file.

This is command-encoding evidence for selected inputs. Rendered effects and
physical Wii behavior remain open. The invalid cull guard is a Rust safety
difference: the C function indexes its lookup table without a bounds check.
HBC was not under test. No production GX code changed. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log contains repeated
partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-generation-mode --timeout-seconds 180
```
