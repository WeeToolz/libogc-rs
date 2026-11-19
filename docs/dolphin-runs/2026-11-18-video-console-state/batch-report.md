# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 481. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_get_default | Video default console supplies its font | PASS | [log L230](runtime_probe/probe.log#L230), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::console_select | Video console selection exposes metrics and position | PASS | [log L228](runtime_probe/probe.log#L228), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::console_set_font | Video console window clips and font copies | PASS | [log L229](runtime_probe/probe.log#L229), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::console_set_window | Video console window clips and font copies | PASS | [log L229](runtime_probe/probe.log#L229), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::console_get_metrics | Video console selection exposes metrics and position | PASS | [log L228](runtime_probe/probe.log#L228), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::console_get_position | Video console selection exposes metrics and position | PASS | [log L228](runtime_probe/probe.log#L228), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin temporary console state and clipped window. Rendering, framebuffer writes, concurrent selection and physical Wii remain unverified.
