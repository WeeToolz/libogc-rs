# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 639. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwbt-sys::new | Bluetooth PCB allocation initializes fields | PASS | [log L241](runtime_probe/probe.log#L241), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::arg | Bluetooth PCB argument setter stores value | PASS | [log L242](runtime_probe/probe.log#L242), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::received | Bluetooth PCB receive setter clears callback | PASS | [log L243](runtime_probe/probe.log#L243), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::disconnected | Bluetooth PCB disconnect setter clears callback | PASS | [log L244](runtime_probe/probe.log#L244), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::free | Bluetooth PCB free releases a slot | PASS | [log L245](runtime_probe/probe.log#L245), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
