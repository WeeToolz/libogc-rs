# Linux Dolphin socket-option comparison

The same `network_probe` DOL was run on Linux with the packaged Dolphin 2503
and a separate Dolphin 2503 build based on upstream commit
`9763c0a1e2b9db0c3861d25bc2f5a0ace6a15ee3`. The DOL SHA-256 in both runs
was `6dd866339f50fe87e5902cd7f97b95afea7a20302e7440abf16d37de1d210ee9`.
Both used a copied Wii NAND and valid page captures. The baseline result is
[recorded separately](2026-11-05-linux-network-baseline.md).

| Dolphin build | Passes | Failures | Valid page captures |
| --- | ---: | ---: | ---: |
| Packaged Linux 2503 | 46 | 6 | 6 |
| Linux socket-option experiment | 50 | 2 | 6 |

The experiment passed SO_REUSEADDR, IP_TTL, TCP_NODELAY and TCP_KEEPALIVE
round trips that failed in the baseline. SO_RCVBUF and SO_RCVTIMEO still
failed. Its final summary was `FAIL summary passes=50 fails=2`; this is not
an overall passing network result. The no-GUI experiment used X11 software
rendering and `Dolphin.Core.CPUThread=False` after separate CPU-thread runs
intermittently stalled before probe output. The packaged baseline used the
runner defaults, so the comparison changes both emulator code and this setting.

The [experimental patch](../../scripts/patches/dolphin-linux-network-options.patch)
adapts the saved macOS socket-option implementation to Linux and uses
TCP_KEEPIDLE. It converts Wii big-endian values and timeval fields, maps the
covered option names, calls real native socket operations, and propagates their
errors. It is an experiment, not complete Wii socket-option conformance or a
replacement for real-Wii verification. Apply it to the specified Dolphin commit
with `git apply --unidiff-zero` and build a separate emulator. The tested executable SHA-256
was `991e6b0735f99f282d9ba547332c28731338b08d86e1081528d6a65afa102311`.

The host kernel returned SO_RCVBUF 32768 after a 16384 request and returned a
receive timeout of 2 seconds, 28000 microseconds after a request for 2 seconds,
25000 microseconds. The probe requires exact values for both. These observations
are consistent with the remaining failures. A subsequent diagnostic run
instrumented the experimental emulator immediately after the native socket
calls, logging the Wii-format words sent to and returned from emulated IOS:

| Option | Set request | Successful get reply |
| --- | --- | --- |
| SO_RCVBUF | 16384 bytes | 32768 bytes |
| SO_RCVTIMEO | 2 seconds, 25000 microseconds | 2 seconds, 28000 microseconds |

That run used the same DOL hash, captured six valid pages, and again reported
50 passes and two failures. The diagnostic executable's SHA-256 was
`c511c2a6baa03c762e7ae7f054c055944e6cbf7b436eca51f474b57085a7287f`.
The trace was added after successful `setsockopt` or `getsockopt` in the
experimental helper and read the emulated request or reply words; it did not
change the socket values or assertions. The matched Linux values explain these
two failures at the emulator boundary. Do not treat native
Linux return values as proof of Wii behavior or change the assertions solely to
make this run pass. Wii behavior and the appropriate emulation of buffer sizes
and timeout precision remain to be verified.

Both runs used `cargo xtask dolphin-validate --no-build --probe network_probe
--timeout-seconds 600`, selecting the respective Dolphin executable. The no-GUI
launcher supplied X11, treated `--batch` as its always-on no-GUI mode, and
added `-C Dolphin.Core.CPUThread=False`. A new run must rebuild or identify the
same DOL, use a legitimate imported NAND, and inspect its own report. No NAND,
device keys, or local network addresses are included in this record.
