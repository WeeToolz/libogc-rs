# GX EFB color and depth checks in Dolphin

Dolphin 2609a on macOS ran the current `video_probe` with **67 passes and
zero failures** on Metal. All six captured pages were valid. The
[probe log](final/video_probe/probe.log), [result](final/video_probe/result.json),
[suite report](final/suite.json), and [EFB screenshot](final/video_probe/pages/page-005.png)
record the result. The screenshots show probe text, not an independent
comparison of rendered images.

## Checks and limits

The new `probe_efb_access` calls the production `poke_argb`, `peek_argb`,
`poke_z`, `peek_z`, and `poke_alpha_read` APIs. After GX initialization, the
probe sets RGB8/Z24 format and requires a completed GX finish event. It writes
three distinct colors and three distinct 24-bit depth values at `(8, 8)`,
`(9, 8)`, and `(8, 9)`. It checks each value against fixed expected values.
It then replaces the first pixel and checks that the other two pixels retain
their color and depth. Changing alpha read mode from `GX_READ_FF` to
`GX_READ_00` must change only the returned alpha byte.

These checks add Dolphin evidence for five APIs. They do not prove physical
Wii behavior, other pixel formats, blending, depth testing, boundary
coordinates, or performance. No HBC program or physical device is under test.
No production GX implementation was changed.

## Controlled backend comparison

All runs below except the old-DOL baseline use this DOL SHA-256:

`96ea9e12957829a1bae59f02f49716521a00dc4231454248378a612822c37e0f`

| Run | Configuration | Passes | Failures |
| --- | --- | ---: | ---: |
| [Old-DOL baseline](../2026-11-15-gx-efb-baseline/suite.json) | OpenGL, existing pre-pull DOL | 52 | 0 |
| [Initial current DOL](../2026-11-15-gx-efb/suite.json) | OpenGL, default CPU EFB access | 63 | 4 |
| [First setting attempt](../2026-11-15-gx-efb-enabled/suite.json) | OpenGL, unrecognized `GFX` system name | 63 | 4 |
| [Diagnostic trace](../2026-11-15-gx-efb-trace/suite.json) | Same setting, memory and video logs | 63 | 4 |
| [CPU access enabled](../2026-11-15-gx-efb-cpu-access/suite.json) | OpenGL, `Graphics.Hacks.EFBAccessEnable=True` | 65 | 2 |
| [Metal](suite.json) | CPU access enabled, Metal | 67 | 0 |

The old DOL hash was
`af312b69ce5f1774335640c099d1e861f7cce3e6ce4d736642c3ead252926fea`.
Its source identity was not verified; it is only an environment baseline.
The current DOL also runs the TPL, frame configuration, and MEM2 FIFO checks
added before this batch. Thus the difference from 52 is not all new coverage
from this change.

With default CPU EFB access, all EFB reads returned zero. The local Dolphin
source declares that setting false. Its command-line parser maps the graphics
system to `Graphics` and silently ignores the attempted `GFX` name. The
runner now explicitly enables CPU EFB access. A regression test failed before
that runner change and passed afterward.

With CPU access enabled, OpenGL returned the exact RGB bytes and the expected
alpha read behavior. The three depth reads returned `0x891a2b`, `0xd5e6f8`,
and `0xb2a190`, instead of `0x123456`, `0xabcdef`, and `0x654321`. The depth
and replacement checks failed. The same DOL passed all checks when only the
video backend changed to Metal. This isolates a backend-dependent result;
the exact cause inside OpenGL remains open. The expected depth values were
not changed. The OpenGL failures remain failures.

## Reproduce

Use the matching nightly compiler and external MP3 fixtures specified in
`docs/dependencies.md`. The first two build attempts stopped because one or
both fixture variables were unset. With both variables set, the build passed.
The selected fixture hash is in [identities.json](identities.json).

From the repository root:

```sh
export LIBOGC_MAD_REFERENCE_MP3=/path/to/reference.mp3
export LIBOGC_MAD_PROBE_MP3=/path/to/probe.mp3
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

The [Metal wrapper](dolphin-metal.sh) selects Metal and enables video logs.
It delegates execution to the existing runner and executable. It requires
`DOLPHIN_METAL_BIN`; an unset variable stops execution. To repeat OpenGL,
pass the real Dolphin executable directly to `--dolphin`. The runner enables
CPU EFB access in both cases. Use a new artifact directory for each run.

The runner uses a private profile with a copied NAND and writable virtual SD.
The normal Dolphin profile is preserved. No USB passthrough or disc image was
selected. Reports contain the source NAND path, launch arguments, timeout,
DOL hash, run times, and page paths. The ELF, executable, source files, compiler,
and source NAND tree identities are in `identities.json`. The tested source
starts at `b2569dd47a74b93fd3cd87612094828ad3e97731` with the probe and runner
changes in this commit. Each saved `video_probe/dolphin.log` copies the log
from its ignored private profile. The trace wrapper requires
`DOLPHIN_TRACE_BIN` and adds memory-map, video, and pixel-engine logging.

The first sandboxed launch stopped at Dolphin `--version` with SIGABRT.
Subsequent launches used approved display and local socket access. Each run
completed its summary and page capture. The runner then stopped Dolphin;
`exit_code: null` is not a normal application exit claim. No cleanup error
was reported. Review the final summary and page validation before accepting
a repeated run.

## Final initialization guard check

The final source calls the EFB test only after successful FIFO initialization.
This prevents EFB commands after an initialization failure. The DOL was rebuilt
and the [final Metal run](final/suite.json) again passed 67 checks with zero
failures and six valid pages. Its DOL SHA-256 is
`c15a156267c41b615ad0e800ee5e8e1115b48a699d625ae287d818169c8f352f`. The controlled OpenGL/Metal comparison above
uses the earlier DOL; no final-DOL OpenGL result is claimed.

## Repository checks

The probe build, `cargo fmt --all --check`, `cargo clippy --all-targets`,
`cargo clippy -p xtask --all-targets -- -D warnings`, `cargo test` (50 tests),
and `cargo test -p xtask` (122 tests) passed. The default host tests do not
execute the PowerPC probe. Its acceptance evidence is the Dolphin run above.
The inventory check passed with 3,388 API entries. Five entries now have
focused probe evidence; the `unverified` total fell from 1,659 to 1,654.

The source structure audit passed. The full `cargo xtask audit-stubs` command
failed with 1,660 findings: 1,654 unverified APIs and six missing mappings for
the existing `dsp-task-probe`. See [the complete audit log](completeness-audit.log).
This batch does not complete the repository-wide API audit. No finding or
allowlist was suppressed. Diff checks pass for source, reports, and metadata.
Raw Dolphin logs retain their original trailing spaces.
