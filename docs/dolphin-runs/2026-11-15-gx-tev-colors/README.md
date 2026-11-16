# GX TEV color, alpha, fog, and clip command checks

Dolphin 2609a on macOS ran `video_probe` on Metal with **196 passes and zero
failures**. All fifteen captured pages were valid. Fifteen new checks call
seven production GX APIs. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json),
[TEV page](video_probe/pages/page-013.png),
[alpha/fog/clip page](video_probe/pages/page-014.png), and
[summary page](video_probe/pages/page-015.png) record the result.

## API checks

| API | Cases and independent expectations |
| --- | --- |
| `set_tev_color` | Registers 0 and 3, with distinct RGBA values. Exact RA/BG field order, register addresses, and three identical BG writes. |
| `set_tev_color_s10` | Registers 0 and 3, with minus 1024, 1023, minus one, and minus 17. Exact eleven-bit channel fields and repeated BG writes. |
| `set_tev_k_color` | Registers 0 and 3. Exact constant-color bank bit, channel fields, register addresses, and repeated BG writes. |
| `set_tev_k_color_s10` | Registers 0 and 3. Exact constant-color bank bit and signed eleven-bit fields. |
| `set_alpha_compare` | Two distinct comparisons and combine operations, with different reference bytes. Exact BP word for register `0xf3`. |
| `set_fog_color` | Two RGB values and an alpha-only change from zero to 255. Exact register `0xf2` words; alpha does not affect output. |
| `set_clip_mode` | Enable and disable. Exact XF address `0x1005`, transfer count, and mode word. |

The shared BP helper serializes fixed expected register words, then calls
the existing command capture check. Expected words are independent constants,
not calculated by a production encoder. Every case verifies the complete
command bytes, exact padded size, zero padding, untouched remaining storage,
and an unchanged GPU token. The repeated BG writes are part of the checked
C contract, not ignored padding.

The C reference functions are `GX_SetTevColor`, `GX_SetTevColorS10`,
`GX_SetTevKColor`, `GX_SetTevKColorS10`, `GX_SetAlphaCompare`,
`GX_SetFogColor`, and `GX_SetClipMode` in `libogc/gx.c`. Its hash is in
[identities.json](identities.json). No production code, runner code,
evidence checker, or allowlist changed. All new cases passed on the first run.

## Scope and run identity

DOL SHA-256:

`0708196f6a2a39587370897df28264bf3ada498e7234df9da9add1a2c1f540c2`

The source starts at `ca4794b` with the probe changes in this batch.
[identities.json](identities.json) records the full revision, source hashes,
ELF and DOL hashes, compiler, Dolphin executable, C reference, external MP3
fixtures, and source NAND tree identity. The suite records launch arguments
and run times.

The existing runner used a private profile, copied NAND, and writable virtual
SD, with CPU EFB access enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logs. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. [dolphin.log](video_probe/dolphin.log) is
copied out of the ignored private profile.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. Raw
stderr retains two DOL alignment warnings and 10,573 partial-send warnings.
The complete summary and all fifteen pages arrived despite those warnings.

These results prove command capture and encoding during PowerPC execution
in Dolphin. The captured commands are not replayed for an image comparison.
Rendered TEV colors, alpha-test rejection, fog blending, clip effects,
other register IDs and inputs, and physical Wii behavior remain open.
The screenshots show probe text. No HBC program or physical device was under
test. Existing EFB, matrix, display-list, and vertex-data checks also pass.

## Reproduce

Set the two MP3 fixture variables as specified in
[dependencies](../../dependencies.md). From the repository root, use a new
artifact directory:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

## Repository checks

The probe build, formatting check, `cargo clippy --all-targets`, and
`cargo test` passed. The host command ran 50 tests; it does not execute the
PowerPC GX crate. The Dolphin result above is the runtime evidence.
The source structure audit and inventory check passed with 3,388 entries.
Seven new evidence rows reduce the `unverified` count from 1,599 to 1,592.
`cargo xtask audit-stubs` failed with 1,598 findings: 1,592 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains those failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
