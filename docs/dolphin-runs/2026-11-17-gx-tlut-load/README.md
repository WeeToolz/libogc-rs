# GX TLUT callback and load commands in Dolphin

Dolphin 2609a on macOS with Metal passed 335 checks with zero failures. All
28 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [pages 24](video_probe/pages/page-024.png) and [25](video_probe/pages/page-025.png).

The probe installs a test callback through `set_tlut_region_callback`, calls
`load_tlut`, and restores the previous callback. Fixed BP words from the C reference check the
source address, region configuration, and two texture-state flushes. The probe
checks the callback name and count, updated region fields, callback replacement,
null object and null callback-region errors, padding, and buffer guards. The
palette address is captured and is not read by GX.

This is command-encoding and callback evidence for selected inputs. Actual
palette transfers, indexed texture sampling, other callback behavior, and
physical Wii remain open. HBC was not under test. No production GX code
changed. The runner stops Dolphin after the summary, so `exit_code` is null.
The stderr log has repeated partial-send and alignment warnings. All checks
and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-17-gx-tlut-load --timeout-seconds 180
```
