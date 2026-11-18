# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 434. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::suspend_thread | LWP driver nested suspension stops worker | PASS | [log L139](runtime_probe/probe.log#L139), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::resume_thread | LWP driver nested suspension stops worker | PASS | [log L139](runtime_probe/probe.log#L139), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::resume_thread | LWP driver final resume lets worker finish | PASS | [log L140](runtime_probe/probe.log#L140), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_is_suspended | LWP driver nested suspension stops worker | PASS | [log L139](runtime_probe/probe.log#L139), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::thread_is_suspended | LWP driver final resume lets worker finish | PASS | [log L140](runtime_probe/probe.log#L140), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin nested suspension of one cooperating worker. Detach, scheduling contention and physical Wii remain unverified.
