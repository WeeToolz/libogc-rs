# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 442. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::signal | LWP queue signal wakes one waiter | PASS | [log L145](runtime_probe/probe.log#L145), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::broadcast | LWP queue broadcast wakes remaining waiter | PASS | [log L146](runtime_probe/probe.log#L146), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::sleep | LWP queue signal wakes one waiter | PASS | [log L145](runtime_probe/probe.log#L145), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp::sleep | LWP queue broadcast wakes remaining waiter | PASS | [log L146](runtime_probe/probe.log#L146), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin two cooperating waiters. Larger wait sets, cancellation races and physical Wii remain unverified.
