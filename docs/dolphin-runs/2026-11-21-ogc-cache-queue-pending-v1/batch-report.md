# Dolphin evidence report

Probe `bus_probe` reported `probe_fail`.
Passed checks 65. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_queue_length | lc_queue_length observes pending locked-cache DMA | FAIL | [log L114](bus_probe/probe.log#L114), [page-004](bus_probe/pages/page-004.png) |
| ogc-cache::lc_queue_wait | lc_queue_wait drains pending locked-cache DMA | FAIL | [log L115](bus_probe/probe.log#L115), [page-004](bus_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-cache::lc_queue_length — lc_queue_length observes pending locked-cache DMA (FAIL)
- ogc-cache::lc_queue_wait — lc_queue_wait drains pending locked-cache DMA (FAIL)
