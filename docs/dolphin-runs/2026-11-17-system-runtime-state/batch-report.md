# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 298. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::new | System runtime state tracks Wii and GameCube input | PASS | [log L131](runtime_probe/probe.log#L131), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::note_stm_event | System runtime state tracks Wii and GameCube input | PASS | [log L131](runtime_probe/probe.log#L131), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::main_loop_step | System runtime state tracks Wii and GameCube input | PASS | [log L131](runtime_probe/probe.log#L131), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::power_callback_attempt | System runtime power cooldown extends after attempts | PASS | [log L132](runtime_probe/probe.log#L132), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::reset_plan | System runtime reset plan reflects target and requested action | PASS | [log L133](runtime_probe/probe.log#L133), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Runtime state decisions in Dolphin. Physical button events, reset execution, and hardware power transitions require separate evidence.
