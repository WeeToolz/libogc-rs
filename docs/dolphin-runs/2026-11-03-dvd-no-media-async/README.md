# DVD no-media API Dolphin run

- Command: `cargo xtask dolphin-validate --no-build --probe dvd_probe`
- Run ID: `1793689037-dce5e10ef120`
- Source revision: `dce5e10ef120df2d8ac3da2876dce6198b8cd0bd`
- Worktree: clean
- Dolphin: 2609a
- DOL SHA-256: `db5465242be0b8905c0d20f5aa6a5985e604aad24d52562edb48541f2cd4a5fa`
- Result: 47 passes, 0 failures

The run had no disc. It checked the facade paths for asynchronous mount,
inquiry, block-safe read, drive-control, seek, and stream-stop. It also checked
synchronous drive-control, GCM offset updates, seek, and stream-stop. It
checked callback delivery after submission. Inquiry returned success without
media. Async control, seek, and stream-stop reported the cover-closed error;
their synchronous forms returned `-1`. The sync GCM offset update returned
one transferred unit. The async GCM offset update returned success.

This run verifies these APIs in Dolphin with no media. It does not verify
actual disc reads, real drive behavior, or motor control. The focused DVD
probe records those limits. The full transcript is `probe.log`; `suite.json`
and `result.json` contain the runner results. `probe-transport.bin` preserves
the received transcript bytes. The DOL, Dolphin log, and captured pages are
also saved here.
