# GX solid rectangle rendered-pixel validation

Dolphin 2609a on macOS ran `video_probe` on Metal with **260 passes and zero
failures**. The runner validated all 22 captured pages. Ten new checks cover
solid 2D setup, rectangle rendering, color-write control, blending, scissor
clipping, and test setup/restoration. The [final probe log](final/video_probe/probe.log),
[result](final/video_probe/result.json), [suite](final/suite.json),
[checks page](final/video_probe/pages/page-020.png), and
[summary page](final/video_probe/pages/page-022.png) record the result.

## Pixel checks

The probe sets an RGB8/Z24 EFB format, a full-mode viewport and scissor, and
an orthographic y-down projection. It reads pixels at `(40,40)`, `(60,40)`,
and `(110,20)` through `peek_argb`, after each GPU completion. The first two
are inside the rectangle; the third is outside. Full-alpha read mode makes
the expected alpha byte `0xff`.

| Step | Expected pixel results and API exercised |
| --- | --- |
| Change TEV color inputs to zero, then draw red | The inside pixels and outside pixel are black. This establishes that the color combiner was changed before the next setup. |
| Call `configure_solid_2d`, then `draw_solid_rect` with opaque red | Both inside pixels are red and the outside pixel stays black. |
| Disable `set_color_update` and draw blue | Both inside pixels stay red. Re-enable color updates for later steps. |
| Set `set_blend_mode` to no blending and draw transparent green | Both inside pixels become green. |
| Set alpha blending and draw transparent red | Both inside pixels stay green. |
| Draw opaque red with alpha blending | Both inside pixels become red. |
| Restrict scissor to the first part of the rectangle and draw opaque green | `(40,40)` becomes green, `(60,40)` stays red, and the outside pixel stays black. |

The changed-combiner step is checked before reconfiguration; a no-op
`configure_solid_2d` would leave the black-only combiner active. The two
transparent-source steps distinguish disabled blending from alpha blending.
The disabled color-write step checks the output rather than only a setter
call. Every rectangle call and GPU completion result is checked. The probe
restores the full-mode scissor and initialized depth setting at the end; the
remaining 2D pipeline state is owned by this probe until it exits.

The four new evidence records are for `configure_solid_2d`, `draw_solid_rect`,
`set_color_update`, and `set_blend_mode`. The scissor result adds rendered
coverage to its earlier command checks. The C reference covers the GX blend,
color-update, and scissor setters; the solid-rectangle facade is a Rust API.
The C source hash is in [identities.json](identities.json).

## Identity and scope

Final DOL SHA-256:

`9bc0cd4af039be9eb767de9d47149534d83f02c5eaeb7c2d3795c978e0405589`

The source starts at `adb9271` plus the final probe change. The source, DOL,
ELF, compiler, Dolphin executable, C reference, external MP3 fixture, and
source NAND hashes are in [identities.json](identities.json). The initial
[run](suite.json) passed 256 checks before the stronger combiner and blend
cases were added. Its DOL SHA-256 is
`8cb28d2efa273d4bf580658c6734ee5b8b39d317ba7465394ca6c758dab3f82d`.
Its probe source hash was not recorded. No production GX, runner, evidence
checker, or allowlist changed.

The runner used a private profile, copied NAND, writable virtual SD, Metal,
and CPU EFB access. No USB passthrough or disc image was selected. Both
saved `dolphin.log` files were copied out of their ignored profiles. The
runner stopped Dolphin after summary and page capture; `exit_code: null` does
not mean normal application exit. No cleanup error was reported. Raw stderr
retains three DOL alignment warnings per run and 14,080 and 14,646 partial-
send warnings in the initial and final runs. Both delivered their summaries
and pages.

These are pixel readback results in Dolphin with the recorded RGB8 format and
sample coordinates. The screenshots show probe text, not the rendered EFB.
Other formats, blend factors, alpha values, geometry, display-copy output,
and physical Wii behavior remain open. HBC and physical devices were not
under test.

## Reproduce

Set both MP3 fixture variables described in [dependencies](../../dependencies.md),
then run from the repository root with a new artifact directory:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

The probe build, formatting, Clippy, and 50 default host tests passed. Those
host tests do not execute the PowerPC GX crate. The Dolphin runs provide the
rendered-pixel evidence. The source structure audit and inventory check
passed with 3,388 entries. Four evidence rows reduce the unverified count
from 1,573 to 1,569. `cargo xtask audit-stubs` failed with 1,575 findings:
1,569 unverified entries and six existing DSP probe mapping gaps. The
[complete audit log](completeness-audit.log) retains the result. No finding
or allowlist was suppressed.
