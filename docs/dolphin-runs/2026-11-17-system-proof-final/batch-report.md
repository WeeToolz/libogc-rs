# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 295. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_alarms | System alarm snapshot records handle and callbacks | PASS | [log L162](runtime_probe/probe.log#L162), [page-006](runtime_probe/pages/page-006.png) |
| ogc-system::hardware_proof | System hardware proof records Dolphin observations | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::snapshot_hardware | System hardware proof records Dolphin observations | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin alarm bookkeeping and default emulated hardware values. Physical reset input, font region, hardware registers, and failure paths require separate evidence.

## Previous run

[Previous suite result](../2026-11-17-system-proof/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| System hardware proof records Dolphin observations | 0 pass, 1 fail | 1 pass, 0 fail |
