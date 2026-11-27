# `lwbt-sys::init` and `shutdown` Dolphin evidence

- Date: 2026-11-27
- Pinned source revision: `5fee9564`; the worktree had the probe and evidence changes during the run.
- Dolphin: 2609a (`/Applications/Dolphin.app/Contents/MacOS/Dolphin`)
- Probe: `runtime_probe`
- DOL SHA-256: `9077c649398df616fba236e980c24251485d51e8fd0badaf72f4cbf9afc2a3ac`
- Result: pass, 917 checks, 0 failures, 108041 ms.
- Named results: `PASS lwBT void init forwards the successful initialization result` and `PASS lwBT void shutdown forwards the successful shutdown result`.
- Command: `cargo xtask dolphin-validate --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 600 --artifact-dir target/dolphin-validation/lwbt-void-init-shutdown-2026-11-27`

The probe sets a known `ERR_VAL` status, calls each void wrapper, and checks that `last_status()` changes to the successful result. It uses the existing lwBT section in `runtime_probe`; it adds no new fixture or runner.

The preserved transport output is [probe-transport.bin](runtime_probe/probe-transport.bin), SHA-256 `bb8cb318127cbaff4773e021ed9038f3e39b3f107e359689efb7f20664b97daa`.

This confirms the public wrapper path under Dolphin. The run does not test controller startup, controller shutdown failure, or physical Bluetooth behavior. Those checks remain open.
