# GX texture coordinate scale commands in Dolphin

Dolphin 2609a on macOS with Metal passed 273 checks with zero failures.
All 23 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 16](video_probe/pages/page-016.png). The page shows probe text.

The probe calls the production `set_tex_coord_scale_manually`,
`set_tex_coord_cyl_wrap`, and `set_tex_coord_bias` APIs. For inactive coordinate
7, it changes flags with manual mode disabled and requires no BP writes. It
then enables manual scale with dimensions 128 by 32, changes wrap and bias,
and disables manual mode. It compares all six emitted BP words in order:
`3e02007f`, `3f01001f`, `3e00007f`, `3f03001f`, `3e01007f`, `3f02001f`.
A second capture checks zero and one sizes as `3e00ffff` and `3f000000`.
The expected fields follow the local C reference functions, whose file hash
is in the identities file. The capture also checks padding and its buffer guard.

This is command-encoding evidence. It does not prove rendered texture
coordinates, all values and coordinate IDs, or physical Wii behavior. HBC
was not under test. No production GX code changed. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log includes repeated
partial-send and alignment warnings. All checks and page captures completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-texcoord-scale --timeout-seconds 180
```
