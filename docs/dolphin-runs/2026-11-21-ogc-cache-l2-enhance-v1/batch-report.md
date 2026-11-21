# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 66. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::l2_enhance | L2 enhancement enables the cache when HID4 supports it | PASS | [log L105](bus_probe/probe.log#L105), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
