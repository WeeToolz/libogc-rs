# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 662. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::add_ticket | ES ticket add rejects invalid size | PASS | [log L289](runtime_probe/probe.log#L289), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::delete_ticket | ES ticket delete rejects null view | PASS | [log L290](runtime_probe/probe.log#L290), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::reimport_title_init | ES title reimport rejects null metadata | PASS | [log L291](runtime_probe/probe.log#L291), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
