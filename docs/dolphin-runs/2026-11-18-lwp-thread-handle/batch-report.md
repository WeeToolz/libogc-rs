# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 430. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::current_thread | LWP current thread handle is live | PASS | [log L134](runtime_probe/probe.log#L134), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::create | LWP created thread returns its handle | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::create | LWP thread runs and returns its argument | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::raw | LWP created thread returns its handle | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::into_raw | LWP created thread returns its handle | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one worker thread with driver-owned stack. Suspension, detach, scheduling contention and physical Wii remain unverified.
