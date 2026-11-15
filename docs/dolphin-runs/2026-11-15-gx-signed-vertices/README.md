# Signed GX vertex capture fix and Dolphin validation

Dolphin 2609a on macOS ran the corrected `video_probe` on Metal with
**127 passes and zero failures**. All eleven captured pages were valid.
Twenty new checks cover ten signed position, normal, and texture-coordinate
APIs. The [final log](final/video_probe/probe.log),
[result](final/video_probe/result.json), [suite](final/suite.json),
[signed coordinate page](final/video_probe/pages/page-010.png), and
[summary page](final/video_probe/pages/page-011.png) record the result.

## Defect and controlled reproduction

The ten signed writers used direct signed volatile stores to the GP pipe.
They bypassed the shared `pipe_u8`/`pipe_u16` path that redirects writes into
an active display-list buffer. Their unsigned counterparts already used
that shared path. Signed vertex data therefore escaped capture.

The new probe first calls each signed writer with zero values and requires
exact captured zero bytes, a padded size of 32, zero padding, untouched
remaining storage, and an unchanged GPU token. Zero bytes act as GPU NOPs
if an affected writer bypasses capture. The probe runs the signed-value case
only after that API's zero capture succeeds. This prevents test patterns
from entering the GPU command stream after a known capture failure.

The [before run](before/suite.json) passed all 107 prior checks and failed
all ten zero-capture checks. It did not run the ten signed-value cases;
each omission is recorded as `Signed boundary case not run: zero capture
failed`. Twelve captured pages were valid. Its DOL SHA-256 was:

`41844c52a2ac6ef216837569f2daa4f969f1da79e7bc3ec163f2da9126003341`

The production fix routes each signed value through the existing byte or
halfword writer after a same-width unsigned cast. The cast retains the
two's-complement bit pattern; the shared writer now handles capture and the
ordinary pipe path. No new transport or encoder was added.

The probe source is byte-identical in the before and final runs. Only the
ten production writer bodies changed. The final run passes all original
107 checks, all ten zero-capture checks, and all ten signed-value checks.
The failing run and its complete evidence are retained under `before/`.

## API cases

| APIs | Fixed signed-value expectations |
| --- | --- |
| `position3s16`, `position3s8` | Minimum, maximum, and minus one, in X/Y/Z order. |
| `position2s16`, `position2s8` | Minimum and maximum, in X/Y order. |
| `normal3s16`, `normal3s8` | Maximum, minus one, and minimum, in X/Y/Z order. |
| `tex_coord2s16`, `tex_coord2s8` | Maximum and minimum, in S/T order. |
| `tex_coord1s16`, `tex_coord1s8` | Minus one, with exact one- or two-byte width. |

Expectations use fixed bytes, including `80 00`, `7f ff`, and `ff ff` for
16-bit values, and `80`, `7f`, and `ff` for 8-bit values. They do not call
the production encoder to calculate expected output. The C contract is the
corresponding inline `GX_Position*s*`, `GX_Normal3s*`, and `GX_TexCoord*s*`
functions in `gc/ogc/gx.h`; its hash is in [identities.json](identities.json).

## Scope and run identity

Final DOL SHA-256:

`6b8b96628ac03b28729c634ae3524081f05896b0b0e1e5fdfa9cef0dd8d4d32b`

The source starts at `d6d7f12` with the new probe, then the signed-writer
fix. [identities.json](identities.json) records separate before/final source,
ELF, and DOL hashes, the compiler, Dolphin executable, C header, external MP3
fixtures, and source NAND tree identity. The suites record launch arguments
and run times. Existing evidence line references were moved to match the
shorter production bodies; no acceptance rule was changed.

The existing runner used a private profile, copied NAND, and writable
virtual SD. CPU EFB access was enabled. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logs. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. Both saved `dolphin.log` files are copied
out of the ignored profiles. No runner code changed.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. Raw
stderr retains three DOL alignment warnings in each run, 8,212 partial-send
warnings before the fix, and 7,477 after it. Complete summaries and pages
arrived despite those warnings.

These checks prove signed data capture and byte encoding in Dolphin.
The captured vertex data is not replayed as geometry. Vertex attribute
formats, rasterized signed geometry, and physical Wii behavior remain open.
No HBC program or physical device was under test.

## Reproduce

Set the two external MP3 fixture variables as specified in
[dependencies](../../dependencies.md). From the repository root, use a new
artifact directory:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

To repeat the failing production behavior, use the parent production source
with the probe from this batch in a separate checkout. Keep fixture identities,
Dolphin settings, and probe source the same. The zero checks must fail; the
signed-value cases must not run after those failures.

## Repository checks

The probe build, formatting check, `cargo clippy --all-targets`, and
`cargo test` passed. The host command ran 50 tests; it does not execute the
PowerPC GX crate. The before/final Dolphin runs provide the target regression
and acceptance evidence. The source structure audit and inventory check
passed with 3,388 entries. Ten new evidence rows reduce the `unverified`
count from 1,636 to 1,626. Existing line references were refreshed after the
production bodies became shorter.

`cargo xtask audit-stubs` failed with 1,632 findings: 1,626 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains these failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
