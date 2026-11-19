# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 287. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::sram_counter_bias | SRAM field getters read supplied signed and unsigned values | PASS | [log L165](runtime_probe/probe.log#L165), [page-006](runtime_probe/pages/page-006.png) |
| ogc-system::sram_display_offset_h | SRAM field getters read supplied signed and unsigned values | PASS | [log L165](runtime_probe/probe.log#L165), [page-006](runtime_probe/pages/page-006.png) |
| ogc-system::sram_language | SRAM field getters read supplied signed and unsigned values | PASS | [log L165](runtime_probe/probe.log#L165), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Checks fields of supplied SRAM records in Dolphin. Physical SRAM storage and persistence require separate evidence.
