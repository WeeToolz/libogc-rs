# GX float and unsigned vertex-data API checks

Dolphin 2609a on macOS ran `video_probe` on Metal with **181 passes and zero
failures**. All fourteen captured pages were valid. Twenty-six new checks
cover thirteen float and unsigned vertex-data APIs. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json),
[vertex page](video_probe/pages/page-012.png), and
[summary page](video_probe/pages/page-014.png) record the result.

## API checks

| APIs | Cases and fixed expectations |
| --- | --- |
| `position3f32` | Distinct positive, negative, and fractional XYZ inputs; a second input with negative zero. Exact twelve IEEE-754 bytes in XYZ order. |
| `position2f32`, `tex_coord2f32` | Inputs 1.5/-2.25 and negative-zero/0.5. Exact eight bytes in XY or ST order. |
| `normal3f32` | Inputs -0.5/0.25/-1 and 0/1/0.5. Exact twelve bytes in XYZ order. |
| `tex_coord1f32` | Inputs 1.5 and negative zero. Exactly four bytes, including the zero sign bit. |
| `position3u16`, `position3u8` | Two distinct XYZ sets with zero, high-bit, and maximum unsigned values. Exact byte order and six-/three-byte size. |
| `position2u16`, `position2u8` | Two XY sets with zero, high-bit, and maximum values. Exact four-/two-byte size. |
| `tex_coord2u16`, `tex_coord2u8` | Two ST sets with zero, high-bit, and maximum values. Exact four-/two-byte size. |
| `tex_coord1u16`, `tex_coord1u8` | Two values for each width. Exact two-byte big-endian encoding or single-byte encoding. |

The float expectations are fixed IEEE-754 bytes. In particular, negative
zero must produce `80 00 00 00`. No expected float bytes are calculated
with the production writer. The existing shared capture check verifies
every byte, exact padded size, zero padding, untouched remaining storage,
and an unchanged GPU token. A missing or shortened write cannot match the
expected size and bytes.

The C contract is the corresponding inline position, normal, and texture
coordinate writers in `gc/ogc/gx.h`. Its source hash is recorded in
[identities.json](identities.json). All new cases passed on the first run.
No production GX code, runner code, or acceptance rule changed.

With the signed-writer and color/index groups already in this probe, the
same run now reports two passing cases for each of all 37 raw vertex-data
writers: 74 capture checks in total. Rendered geometry and physical
hardware acceptance remain open.

## Scope and run identity

DOL SHA-256:

`9d87c8afa16f93c37733a30fe7b8eed68a1dbc955bbad0ff9648f05a65daa05d`

The source starts at `c43831f` with the probe change in this batch.
[identities.json](identities.json) records source, ELF and DOL hashes,
compiler, Dolphin executable, C header, external MP3 fixtures, and source
NAND tree identity. The suite records launch arguments and run times.

The existing runner used a private profile, copied NAND, and writable virtual
SD. CPU EFB access was enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logging. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. [dolphin.log](video_probe/dolphin.log) is
copied out of the ignored private profile.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. The
raw stderr retains two DOL section-alignment warnings and 9,698 partial-send
warnings. The complete summary and all fourteen pages arrived despite those
warnings.

These checks cover capture and data encoding during PowerPC execution in
Dolphin. The captured data is not replayed as geometry. Vertex attribute
format interpretation, rendered position/normal/texture behavior, NaN,
infinity, subnormal float data, and physical Wii behavior remain open.
The screenshots show probe text. HBC and physical devices were not under test.

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
PowerPC GX crate. The Dolphin run above provides runtime evidence.
The source structure audit and inventory check passed with 3,388 entries.
Thirteen new evidence rows reduce the `unverified` count from 1,612 to 1,599.
All 37 raw vertex-data API entries now have focused probe evidence, with two
passing cases each in this run.

`cargo xtask audit-stubs` failed with 1,605 findings: 1,599 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains those failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
