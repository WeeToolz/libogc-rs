# GX indirect TEV commands in Dolphin

Dolphin 2609a on macOS with Metal passed 285 checks with zero failures.
All 24 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 16](video_probe/pages/page-016.png). The page shows probe text.

The probe calls production `set_tev_indirect` and `set_tev_direct` inside a
captured display list. It checks BP words `151c735b` and `15000000` for a
mixed set of fields followed by direct mode, and `1f1fffff` for masked input
and stage values. These fixed words use the bit positions in the local C
reference. The capture checks command bytes, padding, and the buffer guard.
The C source and binary identities are recorded in the identities file.

This is command-encoding evidence. It does not prove rendered indirect
texturing, all parameter combinations, or physical Wii behavior. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log contains repeated partial-send
and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-indirect-tev --timeout-seconds 180
```
