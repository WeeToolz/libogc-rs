# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 294. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_alarms | System alarm snapshot records handle and callbacks | PASS | [log L156](runtime_probe/probe.log#L156), [page-006](runtime_probe/pages/page-006.png) |
| ogc-system::hardware_proof | System hardware proof records Dolphin observations | FAIL | [log L138](runtime_probe/probe.log#L138), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::snapshot_hardware | System hardware proof records Dolphin observations | FAIL | [log L138](runtime_probe/probe.log#L138), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-system::hardware_proof — System hardware proof records Dolphin observations (FAIL)
- ogc-system::snapshot_hardware — System hardware proof records Dolphin observations (FAIL)

## Coverage limits

- Dolphin alarm bookkeeping and default emulated hardware values. Physical reset input, font region, hardware registers, and failure paths require separate evidence.
