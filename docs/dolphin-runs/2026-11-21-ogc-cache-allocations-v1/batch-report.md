# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 60. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::lc_alloc | lc_alloc enables locked cache and zeros one requested line | PASS | [log L105](bus_probe/probe.log#L105), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::lc_alloc_no_invalidate | lc_alloc_no_invalidate zeros one requested line | PASS | [log L107](bus_probe/probe.log#L107), [page-004](bus_probe/pages/page-004.png) |
| ogc-cache::lc_alloc_one_tag | lc_alloc_one_tag zeros one tag without changing adjacent lines | PASS | [log L104](bus_probe/probe.log#L104), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::lc_alloc_tags | lc_alloc_tags zeros one nonzero tag and preserves adjacent lines | PASS | [log L103](bus_probe/probe.log#L103), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
