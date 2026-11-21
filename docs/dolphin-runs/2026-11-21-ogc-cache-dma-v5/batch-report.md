# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 65. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_load_blocks | lc_load_blocks transfers one cache line into locked cache | PASS | [log L117](bus_probe/probe.log#L117), [page-004](bus_probe/pages/page-004.png) |
| ogc-cache::lc_store_blocks | lc_store_blocks transfers one cache line and preserves guard bytes | PASS | [log L118](bus_probe/probe.log#L118), [page-004](bus_probe/pages/page-004.png) |
| ogc-cache::lc_load_data | lc_load_data transfers rounded nonzero data into locked cache | PASS | [log L111](bus_probe/probe.log#L111), [page-004](bus_probe/pages/page-004.png) |
| ogc-cache::lc_store_data | lc_store_data copies one nonzero cache line and preserves guard bytes | PASS | [log L116](bus_probe/probe.log#L116), [page-004](bus_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
