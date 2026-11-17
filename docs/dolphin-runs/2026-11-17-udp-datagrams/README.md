# UDP wrapper checks in Dolphin

The probe sends a fixed IPv4 loopback datagram and checks its bytes, the
unchanged buffer guard, and the sender address. It checks invalid handles
and verifies that a rejected receive leaves its outputs unchanged.

The first Dolphin run failed because the probe expected getsockname to report
the 16-byte storage size. The backend reports the 8-byte IOS address size.
The corrected run checks that length and separately checks the 16-byte
recvfrom address length. See the [first run](../2026-11-17-udp-wrapper-dolphin/batch-report.md)
and the [corrected report](batch-report.md).

Physical Wii behavior, nonzero flags, and truncated datagrams need separate
checks.
