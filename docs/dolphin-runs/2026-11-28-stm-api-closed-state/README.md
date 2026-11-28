# STM power API closed-state checks

Run date: 2026-11-28 UTC. Dolphin 2609a. The `stm_api_probe` passed 22 checks and
reported no failures. The DOL SHA-256 is
`8e25e782e632020ef055fd167005ec543bd89294e0b2d2a6c161ee8ddfbfc4f2`.

After the probe closed STM, it called the public raw and `try_` forms of reboot,
standby shutdown, and idle shutdown. All six calls returned the expected
`STM_ENOTINIT` error. These calls did not submit power-control requests. The
probe saved and restored the VI control register around these checks.

This covers only closed-interface rejection. Successful reboot and shutdown
requests and physical Wii behavior remain unverified. See `suite.json` and
`stm_api_probe/probe.log` for the complete run record and checks.
