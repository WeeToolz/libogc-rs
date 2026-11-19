# GX depth-mode command checks

Dolphin 2609a on macOS ran `video_probe` on Metal with **216 passes and zero
failures**. All seventeen captured pages were valid. Three new checks call
`set_z_mode` with disabled comparison, maximum comparison and update, and
the normal restored setting. They compare fixed BP words `0x40000000`,
`0x4000001f`, and `0x40000017`, respectively. The shared capture check
also verifies padding, buffer guards, and an unchanged GPU token. The C
reference `GX_SetZMode` in `libogc/gx.c` defines these bit fields.

The [probe log](video_probe/probe.log), [result](video_probe/result.json),
[suite](suite.json), and [identities](identities.json) record the run.
The DOL SHA-256 is
`7af3ce5a8d5c682cda69484476c27ba62aef389e4677c674666f925b5f7e713f`.
The source starts at `cc37c44` plus the probe change. No production code,
runner, evidence checker, or allowlist changed.

The runner used a private profile, copied NAND, writable virtual SD, and the
existing Metal wrapper with CPU EFB access enabled. It stopped Dolphin after
summary and page capture; `exit_code: null` does not mean normal application
exit. Raw stderr retains two DOL alignment warnings and 11,535 partial-send
warnings. The complete summary and all seventeen pages arrived. The saved
`dolphin.log` comes from the ignored private profile.

This verifies command encoding during PowerPC execution in Dolphin. The
captured commands are not replayed. Depth comparison effects, all comparison
functions, and physical Wii behavior remain open. No HBC program or physical
device was under test.

Set the external MP3 fixture variables described in
[dependencies](../../dependencies.md), then run from the repository root:

```sh
cargo xtask build-dolphin-probes
export DOLPHIN_METAL_BIN=/path/to/Dolphin
cargo xtask dolphin-validate --no-build --probe video_probe \
  --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" \
  --artifact-dir /path/to/new-run --timeout-seconds 180
```

The source structure audit and inventory check passed with 3,388 entries.
The unverified count fell from 1,587 to 1,586. Formatting, Clippy, and the
50 default host tests passed. `cargo xtask audit-stubs` failed with 1,592
findings: 1,586 unverified entries and six existing DSP probe mapping gaps.
The [complete audit log](completeness-audit.log) retains the result.
