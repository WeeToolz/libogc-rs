# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 751. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_init | Video console_init initializes once and restores the probe display | PASS | [log L422](runtime_probe/probe.log#L422), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
