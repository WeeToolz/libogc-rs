# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 267. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network::sendto | UDP sendto rejects an invalid handle | Missing |  |
| ogc-network::sendto | UDP sendto reports byte count and queues datagram | Missing |  |
| ogc-network::recvfrom | UDP recvfrom rejects an invalid handle and preserves output | PASS | [log L521](runtime_probe/probe.log#L521), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network::recvfrom | UDP recvfrom returns the payload and source address | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-network::sendto — UDP sendto rejects an invalid handle (Missing)
- ogc-network::sendto — UDP sendto reports byte count and queues datagram (Missing)
- ogc-network::recvfrom — UDP recvfrom returns the payload and source address (Missing)
- Other failed check — UDP wrapper binds distinct loopback ports

## Coverage limits

- IPv4 loopback in Dolphin. Physical Wii networking, nonzero flags, and truncated datagrams are outside this run.

## Previous run

[Previous suite result](../2026-11-17-resolver-error-text/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| UDP recvfrom rejects an invalid handle and preserves output | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper binds distinct loopback ports | 0 pass, 0 fail | 0 pass, 1 fail |
| UDP wrapper deinitializes | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper initializes | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper receiver closes | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper sender closes | 0 pass, 0 fail | 1 pass, 0 fail |
| UDP wrapper sockets open | 0 pass, 0 fail | 1 pass, 0 fail |
