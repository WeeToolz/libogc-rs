# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 750. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_init_ex | Video console_init_ex initializes and reuses its console buffer | PASS | [log L420](runtime_probe/probe.log#L420), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
