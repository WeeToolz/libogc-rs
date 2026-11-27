# Dolphin evidence report

Probe `bus_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_load_blocks | lc_load_blocks transfers one cache line into locked cache | PASS | [log L113](bus_probe/probe.log#L113) |
| ogc-cache::lc_store_blocks | lc_store_blocks transfers one cache line and preserves guard bytes | PASS | [log L114](bus_probe/probe.log#L114) |
| ogc-cache::lc_load_data | lc_load_data transfers rounded nonzero data into locked cache | PASS | [log L111](bus_probe/probe.log#L111) |
| ogc-cache::lc_store_data | lc_store_data copies rounded nonzero data and preserves guard bytes | FAIL | [log L112](bus_probe/probe.log#L112) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-cache::lc_store_data — lc_store_data copies rounded nonzero data and preserves guard bytes (FAIL)
