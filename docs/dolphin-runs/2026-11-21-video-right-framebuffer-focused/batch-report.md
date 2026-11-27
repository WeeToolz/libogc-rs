# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 749. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::set_next_right_framebuffer | Video right framebuffer setter records and restores its pointer | PASS | [log L418](runtime_probe/probe.log#L418), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
