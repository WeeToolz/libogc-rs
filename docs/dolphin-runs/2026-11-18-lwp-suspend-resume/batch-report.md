# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 432. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::suspend | LWP suspended worker stops running | PASS | [log L137](runtime_probe/probe.log#L137), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::resume | LWP resumed worker finishes | PASS | [log L138](runtime_probe/probe.log#L138), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::is_suspended | LWP suspended worker stops running | PASS | [log L137](runtime_probe/probe.log#L137), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::is_suspended | LWP resumed worker finishes | PASS | [log L138](runtime_probe/probe.log#L138), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::yield_now | LWP suspended worker stops running | PASS | [log L137](runtime_probe/probe.log#L137), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one cooperating worker. Nested suspension, detach, scheduling contention and physical Wii remain unverified.
