# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 273. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_recvfrom | Raw UDP recvfrom preserves bytes and sender | PASS | [log L463](runtime_probe/probe.log#L463), [page-019](runtime_probe/pages/page-019.png) |
| ogc-network-sys::net_recvfrom | Raw UDP recvfrom leaves outputs unchanged on would-block | PASS | [log L452](runtime_probe/probe.log#L452), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- IPv4 loopback in Dolphin with full-size address storage. This run leaves truncated datagrams and physical Wii networking open.

## Previous run

[Previous suite result](../2026-11-17-raw-recvfrom-red/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| Raw UDP recvfrom leaves outputs unchanged on would-block | 0 pass, 1 fail | 1 pass, 0 fail |
