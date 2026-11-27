# Dolphin evidence report

Probe `bus_probe` reported `probe_fail`.
Passed checks 48. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::dc_flash_invalidate | dc_flash_invalidate sets the DC flash invalidate bit | PASS | [log L97](bus_probe/probe.log#L97), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_block_invalidate | ic_block_invalidate preserves executable code behavior | PASS | [log L99](bus_probe/probe.log#L99), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_flash_invalidate | ic_flash_invalidate sets the IC flash invalidate bit | FAIL | [log L98](bus_probe/probe.log#L98), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_sync | ic_sync completes after instruction cache invalidation | PASS | [log L100](bus_probe/probe.log#L100), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-cache::ic_flash_invalidate — ic_flash_invalidate sets the IC flash invalidate bit (FAIL)
