# GX vertex array and primitive commands in Dolphin

Dolphin 2609a on macOS with Metal passed 298 checks with zero failures.
All 25 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 22](video_probe/pages/page-022.png). The page shows probe text.

The probe captures production `set_array`, `begin`, and `end` commands. It
checks the position array base and stride CP registers, NBT mapping to the
normal array registers, and no output for an invalid attribute. It checks a
triangle primitive header with masked vertex format and big-endian count,
with no additional command from `end`. The address is an encoded value in a
display list and is not dereferenced. The expected bytes follow the local C
reference. The capture checks padding and the buffer guard; source and binary
identities are in the identities file.

This is command-encoding evidence. It does not prove indexed vertex fetching,
rendered geometry, all formats, compiler reordering, or physical Wii behavior.
HBC was not under test. No production GX code changed. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log contains repeated
partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-vertex-array --timeout-seconds 180
```
