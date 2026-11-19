# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 459. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-sysbase::current_reent | Newlib errno is isolated per thread | PASS | [log L235](runtime_probe/probe.log#L235), [page-009](runtime_probe/pages/page-009.png) |
| ogc-sysbase::errno_location | Newlib errno is isolated per thread | PASS | [log L235](runtime_probe/probe.log#L235), [page-009](runtime_probe/pages/page-009.png) |
| ogc-sysbase::set_reent_errno | Newlib errno is isolated per thread | PASS | [log L235](runtime_probe/probe.log#L235), [page-009](runtime_probe/pages/page-009.png) |
| ogc-sysbase::describe_errno | Newlib errno classification maps known and unknown codes | PASS | [log L236](runtime_probe/probe.log#L236), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin parent and one worker with separate errno values. Concurrent reuse, cleanup errors and physical Wii remain unverified.
