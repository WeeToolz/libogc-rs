# GX indirect texture scale and order in Dolphin

Dolphin 2609a on macOS with Metal passed 287 checks with zero failures.
All 24 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 17](video_probe/pages/page-017.png). The pages show probe text.

The probe calls production `set_ind_tex_coord_scale` and `set_ind_tex_order`
inside captured display lists after setting the shared fields to zero. It
checks BP words `25000021`, `25004321`, `26000065`, and `26008765` for four
scale stages, and `27000011`, `27000d11`, `27028d11`, and `27ee8d11` for four
coordinate and map pairs. It also checks that an invalid scale stage emits no
extra command. The fixed words use the local C reference bit positions. The
capture checks command bytes, padding, and the buffer guard. Source and binary
identities are recorded in the identities file.

This is command-encoding evidence. It does not prove rendered indirect
texturing, all parameter combinations, or physical Wii behavior. HBC was not
under test. No production GX code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log contains repeated partial-send
and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-indirect-scale --timeout-seconds 180
```
