# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 659. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::di_verify_with_ticket_view | ES ticket-view verifier rejects null metadata | PASS | [log L282](runtime_probe/probe.log#L282), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::add_title_start | ES title add rejects null metadata | PASS | [log L283](runtime_probe/probe.log#L283), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::add_content_data | ES content add rejects empty input | PASS | [log L284](runtime_probe/probe.log#L284), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::export_title_init | ES title export rejects an empty output buffer | PASS | [log L285](runtime_probe/probe.log#L285), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::export_content_data | ES content export rejects an empty output buffer | PASS | [log L286](runtime_probe/probe.log#L286), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::sign | ES signing rejects null buffers | PASS | [log L287](runtime_probe/probe.log#L287), [page-011](runtime_probe/pages/page-011.png) |
| ogc-es-sys::verify_sign | ES signature verifier rejects null buffers | PASS | [log L288](runtime_probe/probe.log#L288), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
