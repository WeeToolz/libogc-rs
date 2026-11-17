# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 272. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_recvfrom | Raw UDP recvfrom preserves bytes and sender | PASS | [log L463](runtime_probe/probe.log#L463), [page-019](runtime_probe/pages/page-019.png) |
| ogc-network-sys::net_recvfrom | Raw UDP recvfrom leaves outputs unchanged on would-block | FAIL | [log L452](runtime_probe/probe.log#L452), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-network-sys::net_recvfrom — Raw UDP recvfrom leaves outputs unchanged on would-block (FAIL)

## Coverage limits

- IPv4 loopback in Dolphin with full-size address storage. This run leaves truncated datagrams and physical Wii networking open.
