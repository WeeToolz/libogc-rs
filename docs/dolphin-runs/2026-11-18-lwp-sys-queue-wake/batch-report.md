# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 448. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::thread_signal | LWP raw queue signal wakes one waiter | PASS | [log L149](runtime_probe/probe.log#L149), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::thread_broadcast | LWP raw queue broadcast wakes remaining waiter | PASS | [log L150](runtime_probe/probe.log#L150), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_thread_signal_abi | LWP ABI queue signal wakes one waiter | PASS | [log L151](runtime_probe/probe.log#L151), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_thread_broadcast_abi | LWP ABI queue broadcast wakes remaining waiter | PASS | [log L152](runtime_probe/probe.log#L152), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin two cooperating waiters per queue. Larger wait sets, cancellation races and physical Wii remain unverified.
