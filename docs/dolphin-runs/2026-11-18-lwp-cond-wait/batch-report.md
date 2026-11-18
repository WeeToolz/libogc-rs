# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 455. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::timed_wait | LWP condition timeout reacquires mutex | PASS | [log L115](runtime_probe/probe.log#L115), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::wait | LWP condition signal wakes waiting worker | PASS | [log L116](runtime_probe/probe.log#L116), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one timed wait and one signaled waiter. Multiple waiters, cancellation races and physical Wii remain unverified.
