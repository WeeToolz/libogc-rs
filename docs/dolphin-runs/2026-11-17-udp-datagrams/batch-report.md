# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 272. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network::sendto | UDP sendto rejects an invalid handle | PASS | [log L535](runtime_probe/probe.log#L535), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network::sendto | UDP sendto reports byte count and queues datagram | PASS | [log L536](runtime_probe/probe.log#L536), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network::recvfrom | UDP recvfrom rejects an invalid handle and preserves output | PASS | [log L533](runtime_probe/probe.log#L533), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network::recvfrom | UDP recvfrom returns the payload and source address | PASS | [log L541](runtime_probe/probe.log#L541), [page-022](runtime_probe/pages/page-022.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- IPv4 loopback in Dolphin. Physical Wii networking, nonzero flags, and truncated datagrams are outside this run.

## Previous run

[Previous suite result](../2026-11-17-udp-wrapper-dolphin/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| UDP recvfrom returns the payload and source address | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP sendto rejects an invalid handle | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP sendto reports byte count and queues datagram | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper binds distinct loopback ports | 0 pass, 1 fail | 1 pass, 0 fail |
| UDP wrapper binds loopback sockets | 0 pass, 0 fail | 1 pass, 0 fail |
