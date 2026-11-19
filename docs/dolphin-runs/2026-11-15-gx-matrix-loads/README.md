# GX matrix-loading API checks in Dolphin

Dolphin 2609a on macOS ran `video_probe` on Metal with **107 passes and zero
failures**. All ten captured pages were valid. Eighteen new checks call seven
production matrix-loading APIs. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite report](suite.json),
[matrix page](video_probe/pages/page-008.png), and
[summary page](video_probe/pages/page-010.png) record the result.

## API checks

| API | Cases and fixed expectations |
| --- | --- |
| `load_pos_mtx_imm` | Slots 3 and 9. Twelve row-major float words, including translation; exact XF destination and transfer count. |
| `load_pos_mtx_idx` | Source indices `0x1234` and `0xabcd`, with destination slots 3 and 9. Exact opcode, source field, twelve-word count, and position memory address. |
| `load_nrm_mtx_imm` | Slots 3 and 9. Nine words from the first three columns of a 3x4 matrix; translation is omitted. Exact normal memory address and transfer count. |
| `load_nrm_mtx_imm3x3` | Slots 3 and 9. All nine row-major float words and exact normal memory address. |
| `load_nrm_mtx_idx3x3` | Source indices `0x1234` and `0xabcd`, with slots 3 and 9. Exact indexed opcode, source field, nine-word count, and normal memory address. |
| `load_tex_mtx_imm` | Both 2x4 and 3x4 formats, each at texture slot 30 and post-transform slot 67. Exact eight/twelve words, transfer count, and memory bank. |
| `load_tex_mtx_idx` | Both formats and memory banks, with distinct source indices. Exact opcode, source field, eight/twelve-word count, and destination. |

The matrix contains distinct positive and negative values from 1 through 12.
Fixed IEEE-754 word constants provide independent expectations. They are not
calculated by the production matrix encoder. A shared helper converts those
constants to big-endian bytes and combines them with fixed command headers.
The existing capture check verifies every command byte, exact padded size,
zero padding, untouched remaining storage, and an unchanged GPU token.
A missing command cannot match the expected size and bytes.

The C reference covers the corresponding seven `GX_Load*` functions and
`WriteMtxPS4x3`, `WriteMtxPS3x3from4x3`, `WriteMtxPS3x3`, and
`WriteMtxPS4x2` in `libogc/gx.c`. Its source hash is in
[identities.json](identities.json).

## Limits

These checks cover PowerPC execution and command encoding in Dolphin.
The captured commands are not replayed. In particular, the indexed source
values do not identify a configured matrix array in this test. No indexed
memory read is submitted, and no successful indexed fetch is claimed.
GPU matrix memory contents, rendered vertex/normal/texture transforms,
array stride behavior, invalid destinations, and physical Wii behavior
remain open. The screenshots show probe text. Existing EFB, display-list
replay, viewport, scissor, and projection checks also pass in this run.
HBC and physical devices were not under test. No production GX or Dolphin
runner code changed. The new cases passed on their first Dolphin run.

## Run identity and setup

DOL SHA-256:

`5c3d954915922e57a4cb13c0b385762deb7d3c0fd680f04fb929acf5ae923ec6`

The source starts at `48a9f03` with the probe change in this batch.
[identities.json](identities.json) records the full revision, source hashes,
ELF and DOL hashes, compiler, Dolphin executable, external MP3 fixtures,
C reference, and source NAND tree identity. The suite records its launch
arguments and start and finish times.

The existing runner used a private profile with copied NAND and writable
virtual SD. CPU EFB access was enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logs. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. [dolphin.log](video_probe/dolphin.log) is
copied out of the ignored private profile.

The runner stopped Dolphin after the summary and page capture completed.
`exit_code: null` does not mean normal application exit. No cleanup error
was reported. Raw stderr retains three DOL section-alignment warnings and
6,731 partial-send warnings. The complete probe summary and all ten pages
arrived despite those warnings; a pass does not mean warning-free execution.

## Reproduce

Set `LIBOGC_MAD_REFERENCE_MP3` and `LIBOGC_MAD_PROBE_MP3` as specified in
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
PowerPC probe. The Dolphin result above is the target runtime evidence.
The source structure audit and inventory check passed with 3,388 entries.
Seven evidence rows reduce the `unverified` count from 1,643 to 1,636.
`cargo xtask audit-stubs` failed with 1,642 findings: 1,636 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains those failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
