# GX cache invalidation checks in Dolphin

Dolphin 2609a on macOS ran `video_probe` on Metal with **223 passes and zero
failures**. All eighteen captured pages were valid. Seven new checks cover
three GX APIs. The [verified log](verified/video_probe/probe.log),
[result](verified/video_probe/result.json), [suite](verified/suite.json),
[command page](verified/video_probe/pages/page-016.png), and
[summary page](verified/video_probe/pages/page-018.png) record the result.

| API | Observable checks |
| --- | --- |
| `inv_vtx_cache` | Two captures each contain the exact one-byte `0x48` command. |
| `invalidate_tex_all` | Two captures each contain the mask flush, two exact cache words, and a second mask flush. |
| `invalidate_texture_cache` | Two captures check the same four-word sequence through the public wrapper. |

The capture helper checks every byte, exact padded size, zero padding, buffer
guards, and an unchanged GPU token. A separate direct submission calls
`invalidate_tex_all` and `inv_vtx_cache`, then sends token `0x7531`. The
probe waits for completion and checks the token. This shows that subsequent
commands completed in Dolphin; it does not measure actual cache contents.
The wrapper is checked by capture only.

## Initial failures and trace

The [initial run](suite.json) passed 219 checks and failed four texture-cache
checks. It incorrectly expected mask word `0x0f0000ff`, the value before GX
initialization finishes. The [trace run](trace/suite.json) also passed 219
and failed four. Its [log](trace/video_probe/probe.log) records the captured
words `0x0f000000`, `0x66001000`, `0x66001100`, `0x0f000000`, each with
opcode `0x61`. The C initialization calls `__GX_SetIndirectMask(0)`; its
`GX_InvalidateTexAll` flushes that mask around the same two cache words.
The expected mask was corrected to the post-initialization value.

The [corrected run](final/suite.json) passed 223 checks. The final
[verified run](verified/suite.json) repeats them with an explicit call to
`inv_vtx_cache` so the existing evidence checker can map it. Both corrected
runs used DOL SHA-256
`83e068ea71b9675a9ab10b30b3088c978b6ccdecf85c1560bc213bad36b4879d`.
The trace DOL SHA-256 is
`30aacaf29fb1894ea48a4d1cb322840d479d423b790a2f54102c5c1a7454ee33`;
the initial DOL SHA-256 is
`05116edcacc5176376fbc8bd6c57603978b1cff1331468938322f4f24ed3d7ea`.
The initial probe source hash was not saved. The trace and verified source
identities are in [identities.json](identities.json). No production GX,
runner, evidence checker, or allowlist changed.

## Scope and reproduction

The source starts at `ab54d7b` with the probe changes in this batch. The
existing runner used a private profile, copied NAND, writable virtual SD,
Metal, and CPU EFB access. No USB passthrough or disc image was selected.
Each saved `dolphin.log` was copied out of its ignored profile. The runner
stopped Dolphin after summary and page capture; `exit_code: null` does not
mean normal application exit. No cleanup error was reported.

Raw stderr retains two DOL alignment warnings and 11,729 partial-send warnings
in the initial run; three and 12,285 in the trace; three and 12,173 in the
corrected run; and three and 10,767 in the verified run. Each run delivered
its summary and page captures. HBC and physical devices were not under test.

These results cover PowerPC command encoding and completion of a subsequent
GPU token in Dolphin. Cached texture or vertex data, actual invalidation
effects during rendering, and physical Wii behavior remain open.

Set both MP3 fixture variables in [dependencies](../../dependencies.md),
then run from the repository root with a new artifact directory:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

The probe build, formatting check, `cargo clippy --all-targets`, and the
50 default host tests passed. The source structure audit and inventory check passed with 3,388 entries.
Three evidence rows reduce the unverified count from 1,586 to 1,583.
`cargo xtask audit-stubs` failed with 1,589 findings: 1,583 unverified
entries and six existing DSP probe mapping gaps. The [complete audit log](completeness-audit.log)
retains the result. No finding or allowlist was suppressed.
