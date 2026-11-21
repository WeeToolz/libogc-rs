# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 733. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::add_content_start | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_content_start | ES add_content_start rejects a closed service | PASS | [log L667](runtime_probe/probe.log#L667), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_content_finish | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_content_finish | ES add_content_finish rejects a closed service | PASS | [log L668](runtime_probe/probe.log#L668), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_title_cancel | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_title_cancel | ES add_title_cancel rejects a closed service | PASS | [log L669](runtime_probe/probe.log#L669), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_title_finish | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::add_title_finish | ES add_title_finish rejects a closed service | PASS | [log L670](runtime_probe/probe.log#L670), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_content_begin | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_content_begin | ES export_content_begin rejects a closed service | PASS | [log L671](runtime_probe/probe.log#L671), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_content_end | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_content_end | ES export_content_end rejects a closed service | PASS | [log L672](runtime_probe/probe.log#L672), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_title_done | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::export_title_done | ES export_title_done rejects a closed service | PASS | [log L673](runtime_probe/probe.log#L673), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::delete_title | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::delete_title | ES delete_title rejects a closed service | PASS | [log L674](runtime_probe/probe.log#L674), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::delete_title_content | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::delete_title_content | ES delete_title_content rejects a closed service | PASS | [log L675](runtime_probe/probe.log#L675), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::set_uid | ES service closes before guard checks | PASS | [log L666](runtime_probe/probe.log#L666), [page-027](runtime_probe/pages/page-027.png) |
| ogc-es-sys::set_uid | ES set_uid rejects a closed service | PASS | [log L676](runtime_probe/probe.log#L676), [page-027](runtime_probe/pages/page-027.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin checks the closed service guards. Successful requests, IOS side effects, and physical Wii behavior remain outside this batch.
