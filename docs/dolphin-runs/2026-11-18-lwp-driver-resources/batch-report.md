# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 451. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::allocate_workspace | LWP driver workspace preserves separate allocations | PASS | [log L134](runtime_probe/probe.log#L134), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::release_workspace | LWP driver workspace allocates after release | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-driver::close_queue | LWP driver closing queue cancels waiter | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin workspace allocation and cancellation of one queue waiter. Exhaustion, cancellation races and physical Wii remain unverified.
