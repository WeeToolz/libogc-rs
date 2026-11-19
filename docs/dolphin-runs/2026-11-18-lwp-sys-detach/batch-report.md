# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 453. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::detach_thread | LWP raw detached worker finishes without join | PASS | [log L141](runtime_probe/probe.log#L141), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_yield_thread_abi | LWP raw detached worker finishes without join | PASS | [log L141](runtime_probe/probe.log#L141), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one detached worker. Detach and join races, cleanup failure and physical Wii remain unverified.
