# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 293. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_config | System configuration snapshot reflects SRAM fixture | PASS | [log L175](runtime_probe/probe.log#L175), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_arenas | System arena snapshot matches runtime bounds | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin runtime SRAM and arena values. Wireless IDs, setter history, hardware persistence, and physical Wii behavior need separate evidence.
