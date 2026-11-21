# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 773. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_wc24cleanup | WC24 cleanup permits network startup and socket use | PASS | [log L1105](runtime_probe/probe.log#L1105), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network-sys::net_wc24cleanup | WC24 cleanup probe deinitializes | PASS | [log L1106](runtime_probe/probe.log#L1106), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network-sys::net_wc24cleanup | WC24 cleanup permits network startup and socket use | PASS | [log L1105](runtime_probe/probe.log#L1105), [page-041](runtime_probe/pages/page-041.png) |
| ogc-network-sys::net_wc24cleanup | WC24 cleanup probe deinitializes | PASS | [log L1106](runtime_probe/probe.log#L1106), [page-041](runtime_probe/pages/page-041.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
