# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 652. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::_sys_lock_sram | System SRAM lock returns access | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_sync_sram | System SRAM sync reports current state | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_unlock_sram | System SRAM unlock without write returns ready | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_lock_sram_ex | Extended SRAM lock returns access | PASS | [log L264](runtime_probe/probe.log#L264), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_unlock_sram_ex | Extended SRAM unlock without write returns ready | PASS | [log L265](runtime_probe/probe.log#L265), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
