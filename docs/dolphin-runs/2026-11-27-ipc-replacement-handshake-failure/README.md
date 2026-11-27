# IOS replacement with a failed fresh handshake

Date: 2026-11-27. Dolphin 2609a. The existing `ios_reload_probe` passed 113
checks and failed 0 checks. The runner built DOL SHA-256
`c97a71d2e4264ff2227850274bd032cb99ddf8192383e20d2a16c6b68f39a6f1` from a
dirty worktree at `b9c4f74033848f8fcbc1ec8434b806f98a5b9056`. It used a private
copy of the Dolphin NAND and removed that copy after the run. No System Menu
handoff fixture was used. The final formatted-source DOL hash is
`dbd4ecf30ba745e0edfa58a4f236f2fec0bb5cffa5b2d8787500e9b651d97f1e`.

On the third reload iteration, the probe held a real STM event-hook request
and a formatted Bluetooth ACL request. It captured both in-flight request
addresses and their callback reservations. A feature-gated probe hook then
made the fresh `/dev/es` handshake return failure after the IOS reload. Both
captured request addresses remained tracked, both callbacks remained pending,
neither callback ran, and the formatter allocations remained live. The
aggregate tracker and reservation counts were three after the failed
handshake; the two captured request identities were present. A later real
`/dev/es` open/close retry retired the requests. Both old callbacks then ran
once with `IPC_ENOENT`; formatter allocations were freed without staged output
copyback. Two normal reload iterations in the same run passed the new-hook
release case.

This verifies the failed-handshake gate and retry behavior in Dolphin. It
does not simulate an actual `/dev/es` open failure, prove that IOS stopped DMA
to old buffers, or establish physical Wii cache behavior. The unexpected
third aggregate tracker after reload needs source-level concurrency review.
Physical Wii cache and DMA checks remain open.

Evidence: `suite.json` records the command, runner result, Dolphin version,
private NAND handling, and DOL hash. `probe.log` records all 113 named checks.
`probe-transport.bin` is the captured USB Gecko stream. `dolphin.log.gz` keeps
the raw Dolphin log without text conversion. Its uncompressed SHA-256 is
`4d558920acd7df4cd63dfe0c416fd017b7cee655381a92d937e197a9e81b6afd`. The
probe log SHA-256 is
`8062bbca842336ce4dc50601dce460eb527f508cc217b5989863e039fd92318e`; the
transport SHA-256 is `de498d494b00dee7a696993e57a7197f1a8877d5dde6db7eb33009a4015292f2`.
