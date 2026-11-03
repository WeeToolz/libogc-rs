# File-system API Dolphin run

- Command: `cargo xtask dolphin-validate --no-build --probe runtime_probe`
- Run ID: `1793690215-df544baf73e6`
- Source revision: `df544baf73e6366963831e2a6f3f6dd26144edb7`
- Worktree: clean
- Dolphin: 2609a
- DOL SHA-256: `57127fa585aef6e438eb7779cc0d3f370a24499f1870dbfcbe2569f86545221a`
- Result: 48 passes, 0 failures

The probe tested the Rust ISO file-system mount, volume-label, and unmount
facade with a controlled in-memory image. It also called every ISFS API in
`ogc-filesystem`. The ISFS checks covered service initialize, deinitialize,
and reinitialize; error returns with `/dev/fs` closed; unchanged output values
on errors; invalid file-handle errors; and async callback completion for seek
and file stats.

The closed-service format checks returned before they could submit the IOS
format command. The probe did not format NAND or change NAND files. It checks
ISFS rejection and cleanup paths, not successful ISFS file changes. The run
does not prove behavior on a physical Wii. `probe.log` contains the transcript,
and `probe-transport.bin` preserves the received bytes. `suite.json` and
`result.json` contain the runner results. The DOL and captured pages are saved
here.
