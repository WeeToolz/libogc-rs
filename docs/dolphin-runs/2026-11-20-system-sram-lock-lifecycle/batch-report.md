# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 651. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::_sys_lock_sram | System SRAM lock returns access | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_sync_sram | System SRAM sync reports current state | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_unlock_sram | System SRAM unlock without write returns ready | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_lock_sram_ex | Extended SRAM lock returns access | PASS | [log L264](runtime_probe/probe.log#L264), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::_sys_unlock_sram_ex | Extended SRAM unlock without write returns ready | PASS | [log L265](runtime_probe/probe.log#L265), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — probe VI retraces complete within 100 ms
