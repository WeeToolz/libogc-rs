# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 36. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::dc_invalidate_range | cache invalidate preserves flushed data | PASS | [log L76](bus_probe/probe.log#L76), [page-002](bus_probe/pages/page-002.png) |
| ogc-cache::dc_flush_range_no_sync | cache no-sync operations preserve data | PASS | [log L77](bus_probe/probe.log#L77), [page-002](bus_probe/pages/page-002.png) |
| ogc-cache::dc_store_range_no_sync | cache no-sync operations preserve data | PASS | [log L77](bus_probe/probe.log#L77), [page-002](bus_probe/pages/page-002.png) |
| ogc-cache::lc_load_data | locked cache zero-length data operations return zero | PASS | [log L78](bus_probe/probe.log#L78), [page-002](bus_probe/pages/page-002.png) |
| ogc-cache::lc_store_data | locked cache zero-length data operations return zero | PASS | [log L78](bus_probe/probe.log#L78), [page-002](bus_probe/pages/page-002.png) |
| ogc-cache::lc_queue_wait | locked cache wait accepts maximum queue depth | PASS | [log L79](bus_probe/probe.log#L79), [page-002](bus_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The run checks cache maintenance on one aligned buffer and zero-length locked-cache transfers. It does not check cache enable or disable control, nonzero locked-cache transfers, or physical hardware timing.
