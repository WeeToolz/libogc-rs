# GX copy, field-mask, bounding-box, and Z-texture commands

Dolphin 2609a on macOS ran `video_probe` on Metal with **213 passes and zero
failures**. All seventeen captured pages were valid. Seventeen new checks
call five production GX APIs. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json),
[filter and depth page](video_probe/pages/page-015.png), and
[summary page](video_probe/pages/page-017.png) record the result.

## API checks

| API | Cases and independent expectations |
| --- | --- |
| `set_copy_clear` | Two distinct RGBA/depth inputs. Exact AR and GB packing in registers `0x4f`/`0x50`, and 24-bit depth masking in `0x51`. |
| `set_copy_filter` | All four AA/vertical-filter enable combinations. Exact six-word sequence, default values, four-bit sample masks, and six-bit coefficient masks. Null disabled tables are accepted. Two additional checks require null enabled tables to emit no commands. |
| `set_field_mask` | Disabled, even only, odd only, and both with upper input bits set. Exact independent bits in register `0x44`. |
| `clear_bounding_box` | Two calls. Each must emit both exact reset words, `0x550003ff` and `0x560003ff`. |
| `set_z_texture` | Z8/disable, Z16/add, and Z24/replace. Exact operation and format fields in `0xf5` and masked 24-bit bias in `0xf4`. |

The sample table has distinct low nibbles and set upper bits. The vertical
coefficients have low six-bit values 1 through 7 and set upper bits. Fixed
expected register words check ordering and masking without calling a
production encoder. Enabled tables remain live through each call. Disabled
tables are null so the run also checks that their pointers are not read.

The shared BP expectation helper now accepts six words to cover the filter
sequence. It still uses the existing command capture check. Each case checks
all command bytes, exact padded size, zero padding, untouched remaining
storage, and an unchanged GPU token. The 196 earlier checks also pass with
this helper change.

The C contracts are `GX_SetCopyClear`, `GX_SetCopyFilter`, `GX_SetFieldMask`,
`GX_ClearBoundingBox`, and `GX_SetZTexture` in `libogc/gx.c`. Its hash is in
[identities.json](identities.json). Enabled null-table rejection is a Rust
safety behavior; no C null-table acceptance is claimed. No production GX,
runner, evidence checker, or allowlist changed. All new cases passed on the
first Dolphin run.

## Scope and run identity

DOL SHA-256:

`07ce378881a1193cc84d5d02870377d2f6a46349ed686d2b25c234ed7b3f60c9`

The source starts at `2697f6a` with the probe change in this batch.
[identities.json](identities.json) records the full source revision, source
hashes, DOL/ELF hashes, compiler, Dolphin executable, C reference, external
MP3 fixtures, and source NAND tree identity. The suite records launch
arguments and start and finish times.

The existing runner used a private profile with copied NAND and writable
virtual SD. CPU EFB access was enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logs. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. [dolphin.log](video_probe/dolphin.log) is
copied out of the ignored private profile.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. Raw
stderr retains two DOL alignment warnings and 11,650 partial-send warnings.
The complete summary and all seventeen pages arrived despite those warnings.

These results cover PowerPC execution and command encoding in Dolphin. The
captured commands are not replayed. Actual EFB clear/copy results, filtered
image pixels, field rendering, bounding-box readback after rendering,
Z-texture depth effects, and physical Wii behavior remain open. Screenshots
show probe text. No HBC program or physical device was under test.

## Reproduce

Set both external MP3 fixture variables as specified in
[dependencies](../../dependencies.md). From the repository root, select a
new artifact directory:

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
PowerPC GX crate. The Dolphin result above provides runtime evidence.
The source structure audit and inventory check passed with 3,388 entries.
Five new evidence rows reduce the `unverified` count from 1,592 to 1,587.
`cargo xtask audit-stubs` failed with 1,593 findings: 1,587 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains those failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
