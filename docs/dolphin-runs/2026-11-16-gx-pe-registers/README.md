# GX pixel-engine register readback in Dolphin

Dolphin 2609a on macOS ran `video_probe` on Metal with **250 passes and zero
failures**. The runner validated all 21 captured pages. Eighteen new checks
cover seven production pixel-engine APIs and register restoration. The
[probe log](video_probe/probe.log), [result](video_probe/result.json),
[suite](suite.json), [register page](video_probe/pages/page-006.png), and
[summary page](video_probe/pages/page-021.png) record the result. Page 7 has
overlapping text; use the complete probe log for those check values.

## API checks

| API | Observed Dolphin register behavior |
| --- | --- |
| `poke_alpha_mode` | Two fixed function/threshold pairs replace both bytes of PE register 3. |
| `poke_dst_alpha` | Enable values 2 and 255 exercise low-bit masking, with distinct alpha bytes in PE register 2. |
| `poke_color_update` | Disabling and enabling changes only bit 3 of PE register 1. |
| `poke_alpha_update` | Disabling and enabling changes only bit 4 of PE register 1. |
| `poke_dither` | Disabling and enabling changes only bit 2 of PE register 1. |
| `poke_blend_mode` | A zero baseline and fixed blend, logic, and subtract modes yield `0x0000`, `0x64a1`, `0x31e2`, and `0xff01` in PE register 1. |
| `poke_z_mode` | Three cases check the C reference's final-assignment behavior: PE register 0 contains only the update bit, either `0x0000` or `0x0010`. |

The probe reads the hardware-mapped 16-bit PE registers after each API call.
For the three single-bit setters, it compares all other bits with the value
read immediately before the call. The other checks use fixed independent
expected words. Actual and expected values are retained in the log. Before
the checks, the probe saves PE registers 0 through 3; afterward, it writes
back those saved values and verifies all four by readback. No GPU work is
submitted during this sequence.

The reference functions are the corresponding `GX_Poke*` functions in
`libogc/gx.c`. Its hash is in [identities.json](identities.json). The
`GX_PokeZMode` C function's final assignment discards the comparison fields;
the probe checks that behavior and does not claim functional depth-test
coverage. No production GX, runner, evidence checker, or allowlist changed.
All new cases passed on the first Dolphin run.

## Scope and run identity

DOL SHA-256:

`bd3936099b09d81dc98d984737b8af345b5a86d140895752e5556a80c075618f`

The source starts at `7140471` with the probe change in this batch.
[identities.json](identities.json) records source, DOL, ELF, compiler,
Dolphin executable, C reference, external MP3 fixtures, and NAND identities.
The suite records launch arguments and run times.

The existing runner used a private profile, copied NAND, writable virtual SD,
Metal, and CPU EFB access. No USB passthrough or disc image was selected.
[dolphin.log](video_probe/dolphin.log) was copied from the ignored private
profile. The runner stopped Dolphin after summary and page capture;
`exit_code: null` does not mean normal application exit. No cleanup error
was reported. Raw stderr retains three DOL alignment warnings and 14,068
partial-send warnings. The complete summary and all pages arrived.

These results show register write and readback behavior in Dolphin. They do
not prove blending, alpha, dithering, or depth effects on EFB pixels, nor
physical Wii behavior. HBC and physical devices were not under test.

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
host tests do not execute the PowerPC GX crate. The Dolphin run is the target
runtime evidence. The source structure audit and inventory check passed with
3,388 entries. Seven evidence rows reduce the unverified count from 1,580 to
1,573. `cargo xtask audit-stubs` failed with 1,579 findings: 1,573 unverified
entries and six existing DSP probe mapping gaps. The [complete audit log](completeness-audit.log)
retains the result. No finding or allowlist was suppressed.
