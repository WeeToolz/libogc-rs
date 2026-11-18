# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 306. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::init_checked | Checked initialization preserves completed startup | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Repeat initialization after successful Dolphin startup. Failed and concurrent startup and physical hardware need separate checks.
