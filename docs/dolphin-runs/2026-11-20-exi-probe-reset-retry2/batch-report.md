# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 740. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi-sys::hardware_exi_reg_base | raw EXI base selects the Wii register window | PASS | [log L467](runtime_probe/probe.log#L467), [page-018](runtime_probe/pages/page-018.png) |
| ogc-exi-sys::probe_reset | raw EXI probe reset reaches stable channel results | PASS | [log L468](runtime_probe/probe.log#L468), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
