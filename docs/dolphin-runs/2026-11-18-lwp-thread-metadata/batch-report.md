# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 438. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::current_thread | LWP driver reports current thread | PASS | [log L141](runtime_probe/probe.log#L141), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_argument | LWP driver finds thread by its argument | PASS | [log L142](runtime_probe/probe.log#L142), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_argument | LWP driver removes joined thread metadata | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_handle_by_argument | LWP driver finds thread by its argument | PASS | [log L142](runtime_probe/probe.log#L142), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_handle_by_argument | LWP driver removes joined thread metadata | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_context_snapshot | LWP driver captures suspended thread context | PASS | [log L143](runtime_probe/probe.log#L143), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_context_snapshot | LWP driver removes joined thread metadata | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one suspended and joined worker. Register content accuracy, concurrent metadata access and physical Wii remain unverified.
