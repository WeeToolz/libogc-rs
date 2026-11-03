# DVD no-media API Dolphin run

- Command: `cargo xtask dolphin-validate --no-build --probe dvd_probe`
- Run ID: `1793689282-fb392dfffcff`
- Source revision: `fb392dfffcff03ce7d7f063e9910f24fd1d303a9`
- Worktree: clean
- Dolphin: 2609a
- DOL SHA-256: `96c2fc9402f4e5a86e4e513e5bb15eeb5a68860ba64103f299bc47acd623bbd6`
- Result: 53 passes, 0 failures

The run had no disc. It checked the facade paths for asynchronous mount,
inquiry, block-safe read, drive-control, seek, stream-stop, and spin-up. It
also checked synchronous drive-control, GCM offset updates, seek, stream-stop,
spin-up, and the system `stop_motor` API. It checked callback delivery after
submission. Inquiry returned success without media. Async control, seek, and
stream-stop reported the cover-closed error; their synchronous forms returned
`-1`. Spin-up completed in both forms, and `stop_motor` returned zero. The
sync GCM offset update returned one transferred unit. The async GCM offset
update returned success.

This run verifies these calls in Dolphin with no media. It does not verify
actual disc reads or physical drive motor behavior. The full transcript is
`probe.log`; `suite.json` and `result.json` contain the runner results.
`probe-transport.bin` preserves the received transcript bytes. The DOL,
Dolphin log, and captured pages are also saved here.
