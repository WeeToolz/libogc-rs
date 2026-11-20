# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 589. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::snapshot_reset_system | Raw reset snapshot returns stable state | PASS | [log L245](runtime_probe/probe.log#L245), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::set_reset_debug_hook | Reset debug hook setter returns and restores the previous hook | PASS | [log L252](runtime_probe/probe.log#L252), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_get_hollywood_revision | Hollywood revision query marks its hardware snapshot | PASS | [log L246](runtime_probe/probe.log#L246), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::sys_hbc_reload_stub_present | HBC reload-stub query returns a stable result | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_reset_button_down | Reset button query returns its documented boolean value | PASS | [log L247](runtime_probe/probe.log#L247), [page-009](runtime_probe/pages/page-009.png) |
| ogc-system-sys::sys_runtime_fp_disabled_stage | Runtime FP disabled stage remains clear | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_stdio_report_uses_stdout | System stdio report mode query returns stable state | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
