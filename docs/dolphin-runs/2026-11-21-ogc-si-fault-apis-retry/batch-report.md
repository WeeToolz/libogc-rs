# Dolphin evidence report

Probe `pad_state_probe` reported `pass`.
Passed checks 71. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si-sys::si_fault_install | SI raw fault install accepted one step | PASS | [log L76](pad_state_probe/probe.log#L76), [page-002](pad_state_probe/pages/page-002.png) |
| ogc-si-sys::si_fault_clear | SI raw fault clear resets installed count | PASS | [log L78](pad_state_probe/probe.log#L78), [page-002](pad_state_probe/pages/page-002.png) |
| ogc-si-sys::si_fault_release | SI raw fault release finds no held operation | PASS | [log L79](pad_state_probe/probe.log#L79), [page-002](pad_state_probe/pages/page-002.png) |
| ogc-si-sys::si_fault_report | SI raw fault report returns installed count | PASS | [log L77](pad_state_probe/probe.log#L77), [page-002](pad_state_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
