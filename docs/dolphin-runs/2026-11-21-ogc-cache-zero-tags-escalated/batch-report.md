# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 51. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_alloc_tags | locked cache zero-tag allocation preserves data | PASS | [log L80](bus_probe/probe.log#L80), [page-002](bus_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
