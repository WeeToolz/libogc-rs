# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 761. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video-sys::set_next_right_framebuffer | Video raw right framebuffer setter records and restores its pointer | PASS | [log L419](runtime_probe/probe.log#L419), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
