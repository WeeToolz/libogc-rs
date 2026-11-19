# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 291. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_sram | SRAM snapshots reject a held lock | PASS | [log L178](runtime_probe/probe.log#L178), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram | Standard SRAM snapshot matches locked data | PASS | [log L179](runtime_probe/probe.log#L179), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram_ex | SRAM snapshots reject a held lock | PASS | [log L178](runtime_probe/probe.log#L178), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram_ex | Extended SRAM snapshot matches locked data | PASS | [log L182](runtime_probe/probe.log#L182), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram_unlock | Standard SRAM snapshot matches locked data | PASS | [log L179](runtime_probe/probe.log#L179), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram_unlock | Extended SRAM snapshot matches locked data | PASS | [log L182](runtime_probe/probe.log#L182), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::sync_sram | SRAM sync snapshot reports the actual result | PASS | [log L183](runtime_probe/probe.log#L183), [page-007](runtime_probe/pages/page-007.png) |
| ogc-system::snapshot_sram_sync | SRAM sync snapshot reports the actual result | PASS | [log L183](runtime_probe/probe.log#L183), [page-007](runtime_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin SRAM buffer snapshots and synchronization status. Physical SRAM persistence and hardware failure paths need separate evidence.
