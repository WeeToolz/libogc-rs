# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 279. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-runtime::new | runtime thread ID returns its scheduler handle | PASS | [log L96](lwp_probe/probe.log#L96), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::new | runtime thread ID rejects the null sentinel | PASS | [log L97](lwp_probe/probe.log#L97), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
