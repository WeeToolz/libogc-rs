# Current IOS lifecycle probe

Run date: 2026-11-28 UTC. Dolphin 2609a. The `ios_reload_probe` passed 132 checks and
reported no failures. The DOL SHA-256 is
`82c2c71814932a3fac2f63950bb439d6fa508ecd9b91b6b5e54925bc16a09a7d`.

The probe checked the public IOS subsystem and reload entry points. It also
checked an injected ES launch failure, held raw and formatted IPC requests
across IOS replacement, cancellation callbacks and buffer cleanup, a failed
fresh-handshake retry, and fresh requests after recovery. The probe ran with
the Dolphin runner's private user profile. See `suite.json` and
`ios_reload_probe/probe.log` for the run record and check output.

This does not test a launch into an unexpected IOS version, real IOS transport
failure, physical Wii reload behavior, or the physical cache and DMA contracts.
