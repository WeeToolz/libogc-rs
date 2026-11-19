# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 444. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::init_queue | LWP driver queue signal wakes one waiter | PASS | [log L147](runtime_probe/probe.log#L147), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::init_queue | LWP driver queue broadcast wakes remaining waiter | PASS | [log L148](runtime_probe/probe.log#L148), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::queue_signal | LWP driver queue signal wakes one waiter | PASS | [log L147](runtime_probe/probe.log#L147), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::queue_broadcast | LWP driver queue broadcast wakes remaining waiter | PASS | [log L148](runtime_probe/probe.log#L148), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::queue_sleep | LWP driver queue signal wakes one waiter | PASS | [log L147](runtime_probe/probe.log#L147), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::queue_sleep | LWP driver queue broadcast wakes remaining waiter | PASS | [log L148](runtime_probe/probe.log#L148), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin two cooperating waiters. Larger wait sets, cancellation races and physical Wii remain unverified.
