# GX viewport, scissor, and projection command checks

Dolphin 2609a on macOS ran `video_probe` on Metal with **89 passes and zero
failures**. All eight captured pages were valid. Twelve new checks call five
production GX APIs. The [final probe log](final/video_probe/probe.log),
[result](final/video_probe/result.json), [suite report](final/suite.json),
[command page](final/video_probe/pages/page-007.png), and
[summary page](final/video_probe/pages/page-008.png) record the result.

## API checks and scope

| API | Cases and fixed expectations |
| --- | --- |
| `set_viewport` | Nonzero origin and 320 by 240 dimensions; changed origin and 64 by 32 dimensions with reversed depth range. Exact XF address, count, and all six IEEE-754 words. |
| `set_viewport_jitter` | Fields zero and one. Exact half-pixel change in Y origin; all other words stay fixed. |
| `set_scissor` | Nonzero origin and distinct dimensions; zero dimensions; maximum unsigned origins. Exact BP register words, coordinate bias, inclusive bounds, and wrapping arithmetic. |
| `set_scissor_box_offset` | Zero offset, mixed positive/negative offsets, and negative coordinates below the bias. Exact half-unit and ten-bit field encoding. |
| `load_projection_mtx` | Perspective and orthographic selection from a matrix with distinct elements. Exact XF address, count, selected columns, and integer projection type. |

Each case calls its public API inside the existing display-list capture path.
The shared check compares the complete command against independent fixed
bytes, then checks exact padded size, zero padding, untouched remaining
storage, and an unchanged pixel-engine token. A missing command cannot match
the expected size and bytes. The C reference functions are `GX_SetViewport`,
`GX_SetViewportJitter`, `GX_SetScissor`, `GX_SetScissorBoxOffset`, and
`GX_LoadProjectionMtx` in `libogc/gx.c`. Its source hash is in
[identities.json](identities.json).

These checks prove target execution and command encoding in Dolphin. The
captured commands are not replayed for a rendered-image comparison. They do
not prove clipping, viewport rasterization, projection of geometry, physical
Wii behavior, or every possible input. The screenshots show probe text.
The earlier display-list replay and EFB checks also pass in this run. HBC
and physical devices were not under test. No production GX or runner code
changed.

## Initial failure and correction

The [initial run](suite.json) passed the 77 existing checks and failed all
12 new checks. Its DOL SHA-256 was
`fa7559f19719afc8f98c669276396abd028e44179c6eb75760db73dc5d3e36b7`.
The new helper incorrectly expected 32 additional flush bytes.
`end_disp_list` only pads the emitted bytes; the earlier token capture test
receives flush bytes from `set_draw_sync`. The helper now checks the rounded
command length without those extra bytes. Command expectations and guard
checks remain unchanged. The final run repeats all original cases and passes.
The failed run, its log, and all nine captured pages are retained here.

## Run identity and setup

Final DOL SHA-256:

`fc227ad2eb1b43686c6ab67cdffcc4169c89e4ea11ea622e7aafcf825aaada96`

The source starts at `b0d6cf6` with the probe change in this batch.
[identities.json](identities.json) records the full source revision, source
hashes, ELF and DOL hashes, compiler, Dolphin executable, external MP3
fixtures, C reference, and source NAND tree identity. Each suite records its
launch arguments and start and finish times.

The existing runner used a private profile, copied NAND, and writable virtual
SD card. It enabled CPU EFB access. The existing
[Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh) selected Metal
and video logging. No USB passthrough or disc image was selected. The normal
Dolphin profile was preserved. Each `video_probe/dolphin.log` is copied out
of its ignored private profile.

The runner stopped Dolphin after summary and page capture. `exit_code: null`
does not mean normal application exit. No cleanup error was reported. The
initial stderr contains three DOL alignment warnings and 5,359 partial-send
warnings; final stderr contains three alignment warnings and 4,878 partial-send
warnings. Both runs delivered their complete summary and pages. The raw logs
are retained without removing those warnings.

## Reproduce

Set `LIBOGC_MAD_REFERENCE_MP3` and `LIBOGC_MAD_PROBE_MP3` as specified in
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
PowerPC probe. The Dolphin run above is the target runtime evidence.
The source structure audit and inventory check passed with 3,388 API entries.
Five evidence rows reduce the `unverified` count from 1,648 to 1,643.
`cargo xtask audit-stubs` failed with 1,649 findings: 1,643 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains these failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
