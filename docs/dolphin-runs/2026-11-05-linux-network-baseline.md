# Linux Dolphin network baseline

The standard `network_probe` completed on Debian 13 with Dolphin 2503, using a
private copy of a Wii NAND and a virtual X11 display. At repository revision
`938f62b50dfa7f68bd0c767527f8ee2762004cb4`, the DOL SHA-256 was
`6dd866339f50fe87e5902cd7f97b95afea7a20302e7440abf16d37de1d210ee9`.
The result was **46 passes and 6 failures**, with six valid page captures.
The failures were the SO_REUSEADDR, SO_RCVBUF, SO_RCVTIMEO, IP_TTL,
TCP_NODELAY, and TCP_KEEPALIVE socket-option round trips. Other checks in the
probe passed. This result is emulator evidence, not Wii hardware evidence.

The command was `cargo xtask dolphin-validate --no-build --probe network_probe
--timeout-seconds 600`, with the selected Dolphin executable, display, and NAND
configured in the environment. The longer timeout accommodates software-rendered
page capture. A future comparison should use the same DOL hash and check the
complete probe summary and page captures.

Source inspection of Dolphin 2503 (upstream commit
`9763c0a1e2b9db0c3861d25bc2f5a0ace6a15ee3`) shows that its IOS socket
implementation copies Wii option bytes directly to native socket calls, has
incomplete option mappings, and returns success without applying TCP_NODELAY.
The Dolphin log recorded unsupported or invalid option errors for the timeout,
TTL, and TCP options. This supports an emulator-side explanation but does not
independently prove every individual failure's cause.

A separate native Linux check requested SO_RCVBUF 16384 and received 32768.
The probe currently expects an exact round trip, so a Linux emulator adaptation
also needs to account for the host's buffer semantics. The documented macOS
patch and its earlier controlled comparison are described in
[`2026-11-04-network-fixed/README.md`](2026-11-04-network-fixed/README.md).
