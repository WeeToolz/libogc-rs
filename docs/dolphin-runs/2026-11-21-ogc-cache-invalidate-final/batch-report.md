# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 49. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::dc_flash_invalidate | dc_flash_invalidate preserves other HID0 bits and restores state | PASS | [log L100](bus_probe/probe.log#L100), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_block_invalidate | ic_block_invalidate preserves executable code behavior | PASS | [log L102](bus_probe/probe.log#L102), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_flash_invalidate | ic_flash_invalidate preserves other HID0 bits and restores state | PASS | [log L101](bus_probe/probe.log#L101), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_sync | ic_sync completes after instruction cache invalidation | PASS | [log L103](bus_probe/probe.log#L103), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
