# GX vertex format commands in Dolphin

Dolphin 2609a on macOS with Metal passed 302 checks with zero failures.
All 25 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 22](video_probe/pages/page-022.png). The page shows probe text.

The probe calls production `set_vtx_attr_fmt` and `set_vtx_attr_fmtv` in a
captured display list. A subsequent primitive begin flushes the pending vertex
format. The check expects CP registers `77=40000057`, `87=38000000`, and
`97=00000007`, followed by the primitive header. The input covers a signed
position format, a texture format whose fields cross two registers, and a
list entry after the null terminator that must be ignored. The capture checks
command bytes, padding, and buffer guards. Source and binary identities are
recorded in the identities file.

This is command-encoding evidence for selected fields. It does not prove
rendered vertex interpretation, all formats, or physical Wii behavior. HBC
was not under test. No production GX code changed. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log contains repeated
partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-vertex-format --timeout-seconds 180
```
