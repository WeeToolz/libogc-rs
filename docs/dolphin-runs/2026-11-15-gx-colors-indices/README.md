# GX color and vertex-index API checks in Dolphin

Dolphin 2609a on macOS ran `video_probe` on Metal with **155 passes and zero
failures**. All twelve captured pages were valid. Twenty-eight new checks
cover fourteen color and vertex-index APIs. The [final log](final/video_probe/probe.log),
[result](final/video_probe/result.json), [suite](final/suite.json),
[index page](final/video_probe/pages/page-011.png), and
[summary page](final/video_probe/pages/page-012.png) record the result.

## API checks

| APIs | Cases and independent expected bytes |
| --- | --- |
| `color4u8` | Distinct RGBA channels, then maximum/zero/high-bit channels. Exact four-byte channel order. |
| `color3u8` | Distinct RGB channels, then maximum/zero/high-bit channels. Exact three-byte size and order. |
| `color3f32` | Normalized inputs 0.5/0.25/0.75 produce 127/63/191. Inputs 1/0/0.125 produce 255/0/31. This checks multiplication by 255 and truncation. |
| `color1u32`, `color1u16` | Two fixed packed values for each width. Exact big-endian byte order and two-/four-byte size. |
| `position1x8`, `normal1x8`, `color1x8`, `tex_coord1x8`, `matrix_index1x8` | Indices `0x12` and `0xff`. Exactly one captured byte for each API and input. |
| `position1x16`, `normal1x16`, `color1x16`, `tex_coord1x16` | Indices `0x1234` and `0xffff`. Exactly two big-endian captured bytes for each API and input. |

The existing shared capture check verifies every byte, exact padded size,
zero padding, untouched remaining storage, and an unchanged GPU token.
No expected conversion uses the production encoder. The corresponding
inline functions in `gc/ogc/gx.h` provide the C contract. Its source hash is
in [identities.json](identities.json).

The initial probe also passed 155 checks with zero failures and twelve valid
pages. It selected index writers as API function pointers. The final source
uses equivalent noncapturing closures with explicit public API calls so that
the existing evidence checker can map each API. It repeats all cases. The
initial report and logs remain at this directory's root. No production code,
runner code, evidence checker, or allowlist changed.

## Scope and identity

Final DOL SHA-256:

`008bf1a81635c1dbf27a1a778c84b2ba01d8e4e7eec842e0be7f87a3990a54bd`

Initial DOL SHA-256:

`37ee2537495e3e4a0e3d5738893bffb2f27ee6c60898fb418568b0067a8181d0`

The source starts at `9838ca3` with the probe changes in this batch.
[identities.json](identities.json) records both source/DOL/ELF identities,
compiler, Dolphin executable, C header, external MP3 fixtures, and source
NAND tree identity. Each suite records launch arguments and run times.

The runner used a private profile, copied NAND, and writable virtual SD,
with CPU EFB access enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logs. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. Both saved `dolphin.log` files are copied out
of their ignored private profiles.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. Each
raw stderr retains three DOL alignment warnings. The initial run has 8,482
partial-send warnings; the final run has 6,815. Both delivered their complete
summary and pages despite those warnings.

These results cover capture and data encoding during PowerPC execution in
Dolphin. The captured vertex data is not replayed. Indexed array reads,
rendered colors, matrix selection during rendering, out-of-range or NaN
float color conversion, and physical Wii behavior remain open. The
screenshots show probe text. No HBC program or physical device was under test.

## Reproduce

Set the two MP3 fixture variables as specified in
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
PowerPC GX crate. The Dolphin results above provide runtime evidence.
The source structure audit and inventory check passed with 3,388 entries.
Fourteen new evidence rows reduce the `unverified` count from 1,626 to 1,612.
`cargo xtask audit-stubs` failed with 1,618 findings: 1,612 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains those failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
