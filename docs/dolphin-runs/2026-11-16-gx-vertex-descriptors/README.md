# GX vertex descriptor checks in Dolphin

Dolphin 2609a on macOS with Metal passed 270 checks with zero failures.
All 22 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 20](video_probe/pages/page-020.png). The page shows probe text.

The probe saves the descriptor list, clears it, and checks every returned entry
is the null marker with type zero. It sets an out-of-order list containing a
position matrix index, indexed position, NBT normal, direct color, and indexed
texture 7. The expected output uses the C reference order. A texture entry
after the end marker must not appear. The probe then replaces NBT with a normal,
removes the color, checks the exact remaining entries and cleared tail, and
restores and checks the saved list. It calls `clear_vtx_desc`, `set_vtx_desc`,
`set_vtx_descv`, and `get_vtx_descv` on their production paths. The reference
`GX_SetVtxDescv`, `GX_GetVtxDescv`, and `GX_ClearVtxDesc` are in the local C
`libogc/gx.c`; its hash is in the identities file.

This checks software descriptor state. The setter and getter share state, so
these checks do not independently prove the emitted CP/XF commands or rendered
geometry. It does not cover every attribute, every input type, or physical Wii.
HBC was not under test. No production GX code changed.

The runner used CPU EFB access and the saved Metal wrapper. It stops Dolphin
after the probe summary, so `exit_code` is null. The stderr log includes
repeated partial-send and alignment warnings. All checks and page captures
completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-vertex-descriptors --timeout-seconds 180
```
