# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 749. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_init_ex | Video console_init_ex initializes and reuses its console buffer | FAIL | [log L420](runtime_probe/probe.log#L420), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-video::console_init_ex — Video console_init_ex initializes and reuses its console buffer (FAIL)
