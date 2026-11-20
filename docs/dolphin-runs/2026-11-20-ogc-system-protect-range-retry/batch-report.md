# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 614. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_protect_range | System protection call records its arguments | PASS | [log L253](runtime_probe/probe.log#L253), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::snapshot_protect_range | System protection snapshot returns recorded arguments | PASS | [log L254](runtime_probe/probe.log#L254), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
