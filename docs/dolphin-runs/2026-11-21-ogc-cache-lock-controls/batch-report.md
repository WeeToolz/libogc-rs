# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 55. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_disable | lc_disable clears only the locked-cache enable bit | PASS | [log L98](bus_probe/probe.log#L98), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::lc_enable | lc_enable restores locked-cache enable after disable | PASS | [log L99](bus_probe/probe.log#L99), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::lc_flush_queue | lc_flush_queue leaves an initially empty queue empty | PASS | [log L100](bus_probe/probe.log#L100), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
