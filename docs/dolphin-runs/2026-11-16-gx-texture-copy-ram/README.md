# GX EFB texture copy in Dolphin

Dolphin 2609a on macOS with Metal and EFB copies to RAM enabled passed 277
checks with zero failures. All 23 captured pages were valid. See the
[probe log](video_probe/probe.log), [result](video_probe/result.json),
[suite](suite.json), [identities](identities.json), and
[page 22](video_probe/pages/page-022.png). The page shows probe text.

The probe writes a 4 by 4 EFB pattern at (100, 100), with red, green, blue,
and yellow quadrants. It calls production `set_tex_copy_src`,
`set_tex_copy_dst`, and `copy_tex` to copy into one aligned RGBA8 tile.
After GX completion and CPU cache invalidation, it compares all 64 bytes with
the independently constructed tiled image. It draws the copied texture and
checks four EFB pixels against the quadrant colors and one outside pixel
against black. The pixel values are in the probe log. The local C copy
reference hash and the DOL and tool identities are recorded in identities.

The [first run](../2026-11-16-gx-texture-copy/suite.json) used the same DOL
and default EFB copy configuration. It passed the rendered-pixel check but
failed the RAM byte comparison: 276 passes and one failure. The second run
changed the wrapper to add `Graphics.Hacks.EFBToTextureEnable=False`; it
passed both checks. This comparison is consistent with the default copy mode
keeping a texture copy without providing the expected RAM bytes. It does not
establish the exact internal Dolphin mechanism. The first failure remains in
its saved log and is not counted as a pass.

This covers one non-clearing RGBA8 4 by 4 copy and its rendered result. Other
formats, dimensions, clear mode, physical Wii, and HBC remain untested. No
production GX code changed. The runner stops Dolphin after the summary, so
`exit_code` is null. Both stderr logs contain repeated partial-send and
alignment warnings. Both runs captured valid pages.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-16-gx-texture-copy-ram/dolphin-metal-ram.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-texture-copy-ram --timeout-seconds 180
```
