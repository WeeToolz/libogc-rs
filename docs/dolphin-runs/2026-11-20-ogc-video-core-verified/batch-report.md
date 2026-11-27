# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 582. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_clear | Video console clear returns | PASS | [log L274](runtime_probe/probe.log#L274), [page-011](runtime_probe/pages/page-011.png) |
| ogc-video::console_blit_rgba | Video RGBA blit rejects invalid image sizes | PASS | [log L275](runtime_probe/probe.log#L275), [page-011](runtime_probe/pages/page-011.png) |
| ogc-video::get_current_line | Video current line query returns | PASS | [log L247](runtime_probe/probe.log#L247), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::get_next_field | Video next field query returns | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
