# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 766. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::raw | LWP mutex handle retains initialized raw value | PASS | [log L112](runtime_probe/probe.log#L112), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::raw | LWP mutex handle closes | PASS | [log L113](runtime_probe/probe.log#L113), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
