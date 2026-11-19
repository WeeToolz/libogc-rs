# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 485. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_set_power_callback | System power callback dispatch observes cooldown | PASS | [log L227](runtime_probe/probe.log#L227), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::sys_do_power_cb | System power callback dispatch observes cooldown | PASS | [log L227](runtime_probe/probe.log#L227), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::snapshot_power_dispatch | System power callback dispatch observes cooldown | PASS | [log L227](runtime_probe/probe.log#L227), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::sys_set_reset_callback | System reset callback setter returns previous handler | PASS | [log L228](runtime_probe/probe.log#L228), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin synchronous callback dispatch and one cooldown interval. Hardware button events, concurrent callback replacement and physical Wii remain unverified.
