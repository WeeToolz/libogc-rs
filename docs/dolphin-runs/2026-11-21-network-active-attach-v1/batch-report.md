# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 769. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_attach_active | raw network attach opens active IOS socket service | PASS | [log L1099](runtime_probe/probe.log#L1099), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network-sys::net_attach_active | raw network attach opens active IOS socket service | PASS | [log L1099](runtime_probe/probe.log#L1099), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network::attach_active | network wrapper attaches and opens a socket | PASS | [log L1100](runtime_probe/probe.log#L1100), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network::attach_active | network attach probe deinitializes | PASS | [log L1101](runtime_probe/probe.log#L1101), [page-041](runtime_probe/pages/page-041.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
