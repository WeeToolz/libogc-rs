# GX lighting channel commands in Dolphin

Dolphin 2609a on macOS with Metal passed 303 checks with zero failures.
All 25 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 22](video_probe/pages/page-022.png).

The probe calls production `set_chan_amb_color`, `set_chan_mat_color`, and
`set_chan_ctrl` in a captured display list. A primitive begin flushes the
pending state. The check expects XF words `100a=11223399`, `100d=123456dd`,
`100e=00005757`, and `1010=00005757`, followed by the primitive header.
The inputs check color and alpha updates, preservation of the other color
components, and the paired control registers. The capture checks command
bytes, padding, and buffer guards. The C implementation was used to derive
the expected words. Source and binary identities are in the identities file.

This is command-encoding evidence for selected inputs. Rendered lighting,
other inputs, and physical Wii behavior remain open. HBC was not under test.
No production GX code changed. The runner stops Dolphin after the summary,
so `exit_code` is null. The stderr log contains repeated partial-send and
alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-lighting-channels --timeout-seconds 180
```
