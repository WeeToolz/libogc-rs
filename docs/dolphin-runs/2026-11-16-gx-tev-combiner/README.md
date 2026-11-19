# GX TEV combiner command checks in Dolphin

Dolphin 2609a on macOS with Metal passed 271 checks with zero failures.
All 23 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 16](video_probe/pages/page-016.png). The page shows probe text.

The probe initializes inactive TEV stage 5 to known state. It calls the
production `set_tev_color_in`, `set_tev_alpha_in`, `set_tev_color_op`, and
`set_tev_alpha_op` inside one display list. It checks the BP opcode and words
`ca001234`, `cb0029c0`, `caad1234`, and `cb4729c0`, in order. These values
check the four input fields, stage masking, subtract parameters, comparison
operation encoding, and retention of the input fields when the operation
changes. They use the bit positions in the local C `GX_SetTevColorIn`,
`GX_SetTevAlphaIn`, `GX_SetTevColorOp`, and `GX_SetTevAlphaOp` reference;
the reference file hash is in the identities file. The capture also checks
padding and its buffer guard.

This is command-encoding evidence. It does not prove the rendered effect of
each operation, all stage or input values, or physical Wii behavior. HBC was
not under test. No production GX code changed. The runner stops Dolphin after
the summary, so `exit_code` is null. The stderr log includes repeated
partial-send and alignment warnings. All checks and page captures completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-tev-combiner --timeout-seconds 180
```
