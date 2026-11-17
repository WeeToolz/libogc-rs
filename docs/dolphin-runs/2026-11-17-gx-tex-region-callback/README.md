# GX texture region callback in Dolphin

Dolphin 2609a on macOS with Metal passed 338 checks with zero failures. All
28 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 25](video_probe/pages/page-025.png).

The probe installs a test callback through production `set_tex_region_callback`,
then calls `load_tex_obj` for a map 5 RGBA8 object. It checks the object pointer,
map argument, call count, and six fixed BP words for the returned region. It
also checks a null callback result, the replacement return value, restoration
of the previous callback, padding, and buffer guards. The image address is
captured and is not read by GX. Expected command words follow the C reference.

This is callback and command-encoding evidence for selected inputs. Actual
texture sampling, other callbacks, and physical Wii behavior remain open.
HBC was not under test. No production GX code changed. The runner stops
Dolphin after the summary, so `exit_code` is null. The stderr log has repeated
partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-17-gx-tex-region-callback --timeout-seconds 180
```
