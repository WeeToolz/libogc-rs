# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 622. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::launch_title | ES launch rejects null ticket view | PASS | [log L253](runtime_probe/probe.log#L253), [page-010](runtime_probe/pages/page-010.png) |
| ogc-es-sys::launch_title_background | ES background launch rejects null ticket view | PASS | [log L254](runtime_probe/probe.log#L254), [page-010](runtime_probe/pages/page-010.png) |
| ogc-es-sys::identify | ES identify rejects invalid ticket size | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| ogc-es-sys::verify_with_ticket_view | ES verify rejects null signed metadata | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
