# GX display-list API checks in Dolphin

Dolphin 2609a on macOS ran `video_probe` with **77 passes and zero failures**
on Metal. The ten new display-list checks passed. All seven captured pages
were valid. The [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite report](suite.json),
[display-list page](video_probe/pages/page-006.png), and
[summary page](video_probe/pages/page-007.png) record the result.

## API checks

The new `probe_display_lists` calls six production APIs directly:

| API | Observable check |
| --- | --- |
| `begin_disp_list` | Commands go to an aligned capture buffer and do not change the GPU token. |
| `end_disp_list` | The result is 64 bytes with exact zero padding. Overflow and inactive capture return zero. |
| `call_disp_list` | GPU replay changes the pixel-engine token to each captured value. |
| `set_draw_sync` | Direct and captured commands use distinct tokens, with exact BP register bytes. |
| `get_draw_sync` | Pixel-engine readback matches the direct token, each replay token, and the final recovery token. |
| `tex_mode_sync` | The captured command is the exact five-byte BP write to register `0x63`. |

The probe first submits token `0x1357` and waits for GX completion. For each
of `0x2468` and `0xace1`, it captures texture sync and token commands into
64 bytes of a 96-byte aligned buffer. It checks all 15 command bytes, all
49 zero bytes, and the 32-byte guard. The fixed expected BP opcodes and
registers match the C reference contract. No expected bytes come from the
production encoder under test.

Capture must leave the prior GPU token unchanged. The probe flushes the
captured bytes from the CPU cache, submits the list, waits for GX completion,
and checks the pixel-engine token. The buffer stays live until completion.
Using two tokens checks replacement and repeated capture.

A separate 32-byte capture cannot hold the token commands and their flush.
It must return zero, preserve all 64 guard bytes, and leave the GPU token
unchanged. A second end call must return zero. A final direct token `0x3579`
must reach the GPU, which checks FIFO restoration after overflow.

These are Dolphin results. They do not prove physical Wii behavior,
texture-cache invalidation effects, rendered display-list geometry, nested
capture, or saved GX context restoration. The texture sync check proves its
command encoding only. The screenshots show probe text, not a separate
rendered-image comparison. HBC and physical devices were not under test.
No production GX code or runner code changed.

## Run identity and setup

The DOL SHA-256 is:

`75e5df97ac22a7aa6fada1fc65fe5df974b9ec3b66d6103a0712c3d2fa04d527`

The source starts at `2dc52f93cd4ecc82566e788bf8eda3d987170d9f` with the
probe change in this batch. [identities.json](identities.json) records source,
ELF, DOL, Dolphin executable, compiler, MP3 fixture, C reference, and source
NAND identities. The [Dolphin log](video_probe/dolphin.log) is copied out of
the ignored private profile. The runner used a copied NAND and private
writable virtual SD card. It did not select USB passthrough or a disc image.
The normal Dolphin profile was preserved.

The existing [Metal wrapper](../2026-11-15-gx-efb-metal/dolphin-metal.sh)
selects Metal and video logs. The runner enables CPU EFB access. The previous
EFB report explains why this backend is used. This batch did not repeat the
OpenGL comparison. The runner stopped Dolphin after the summary and page
capture completed. `exit_code: null` does not claim normal application exit.
No cleanup error was reported.

The raw stderr log contains three DOL section-alignment warnings and 3,811
`Partial sends might not be handled properly` warnings. The probe transport
still delivered its complete summary and all seven pages. These warnings are
retained; a pass does not claim that Dolphin ran without warnings.

## Reproduce

Set both external MP3 fixture variables as specified in
[`docs/dependencies.md`](../../dependencies.md). Then run from the repository
root with a new artifact directory:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

The first build found a missing unsafe block around the cache flush. The
block was corrected before the successful build and Dolphin run.

## Repository checks

The probe build, `cargo fmt --all --check`, `cargo clippy --all-targets`,
and `cargo test` passed. The host command ran 50 tests; it does not execute
the PowerPC probe. The Dolphin result above is its runtime evidence.

The source structure audit and inventory check passed with 3,388 API entries.
The six evidence rows reduce the `unverified` count from 1,654 to 1,648.
`cargo xtask audit-stubs` failed with 1,654 findings: 1,648 unverified API
entries and six existing missing mappings for `dsp-task-probe`. The
[complete audit log](completeness-audit.log) retains the failures. No finding
or allowlist was suppressed. Source, report, and metadata diff checks passed;
raw Dolphin logs retain their original whitespace.
