# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 758. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-sysbase::setDefaultDevice | setDefaultDevice selects a registered default device | PASS | [log L1096](runtime_probe/probe.log#L1096), [page-041](runtime_probe/pages/page-041.png) |
| ogc-sysbase::setDefaultDevice | setDefaultDevice ignores out-of-range indexes | PASS | [log L1097](runtime_probe/probe.log#L1097), [page-041](runtime_probe/pages/page-041.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
