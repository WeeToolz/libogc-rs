# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 469. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_register_reset_func | System reset registration tracks the same live record | PASS | [log L221](runtime_probe/probe.log#L221), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::sys_unregister_reset_func | System reset registration tracks the same live record | PASS | [log L221](runtime_probe/probe.log#L221), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_reset_registration | System reset registration tracks the same live record | PASS | [log L221](runtime_probe/probe.log#L221), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin registration, duplicate registration and removal of one inert reset record. Callback ordering, invocation, concurrent mutation and physical Wii remain unverified.
