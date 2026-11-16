# GX line, point, and field-mode validation

Dolphin 2609a on macOS ran `video_probe` on Metal with **232 passes and zero
failures**. All nineteen captured pages were valid. The new checks call
`set_line_width`, `set_point_size`, and `set_field_mode`. The [final log](final/video_probe/probe.log),
[result](final/video_probe/result.json), [suite](final/suite.json),
[checks page](final/video_probe/pages/page-017.png), and
[summary page](final/video_probe/pages/page-019.png) record the result.

## Defect and reproduction

The line and point APIs update distinct fields in the shared line/point
register. The test establishes a baseline, disables display-list context
saving for this sequence, and checks fixed register words after changing the
line width and point size. It then checks both field-mode values and their
texture-mask flushes. The probe restores context saving and the normal line,
point, and field settings; the restoration completes in Dolphin.

The [before run](before/suite.json) passed 227 checks and failed two
field-mode checks. Both line and point checks passed. The [trace run](trace/suite.json)
also passed 227 and failed two. Its [log](trace/video_probe/probe.log)
records `0x226b220c`, `0x0f0000ff`, `0x68000001`, and `0x0f0000ff`, each
with opcode `0x61`. The C `GX_SetFieldMode` flushes the current indirect
texture mask around the field command; initialization sets that mask to
zero. The Rust implementation instead wrote `0x0f0000ff` unconditionally.

The fix writes the current indirect-mask shadow value at both flush points.
The final run repeats the original cases and adds a nonzero-mask case. It
uses the production indirect-stage configuration and flush path to establish
`0x0f000008`, then checks that both field-mode flushes retain it. A separate
check confirms that the no-indirect-stage setup is restored. These cases
would not pass if field mode always wrote zero or always wrote `0xff`.
The C reference is `GX_SetFieldMode` and `__GX_FlushTextureState` in
`libogc/gx.c`. Its source hash is in [identities.json](identities.json).

The final trace records `0x226b220c`, `0x0f000000`, `0x68000001`, and
`0x0f000000` after restoring the zero mask. Expected words are independent
fixed values. The capture helper checks complete bytes, padded size, zero
padding, buffer guards, and an unchanged GPU token. No runner, evidence
checker, or allowlist changed.

## Identity and scope

Final DOL SHA-256:

`159402815b4d706f4d1d6e054f62ba091d2768e13f8137cb5df6ef5998e88d7e`

The before and trace DOL hashes are recorded in their results; source hashes
for the before, trace, and final probe are in [identities.json](identities.json).
The source starts at `790d7ea`; the final source includes the two-word fix
and the added nonzero-mask case. The runs started on November 16 UTC, at
04:56, 04:57, and 04:59. The directory date is the local November 15 date.

The existing runner used a private profile, copied NAND, writable virtual SD,
Metal, and CPU EFB access. No USB passthrough or disc image was selected.
Each saved `dolphin.log` was copied from its ignored private profile. The
runner stopped Dolphin after summary and page capture; `exit_code: null` does
not mean normal application exit. No cleanup error was reported.

Raw stderr retains three DOL alignment warnings per run, and 12,142, 12,727,
and 13,028 partial-send warnings in the before, trace, and final runs. All
summaries and page captures arrived. The results cover command encoding and
completion of the test setup/restoration in Dolphin. Rendered line/point
widths, field rendering, indirect texture effects, and physical Wii behavior
remain open. HBC and physical devices were not under test.

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
target regression evidence. The source structure audit and inventory check
passed with 3,388 entries. Three evidence rows reduce the unverified count
from 1,583 to 1,580. `cargo xtask audit-stubs` failed with 1,586 findings:
1,580 unverified entries and six existing DSP probe mapping gaps. The
[complete audit log](completeness-audit.log) retains the result. No finding
or allowlist was suppressed.
