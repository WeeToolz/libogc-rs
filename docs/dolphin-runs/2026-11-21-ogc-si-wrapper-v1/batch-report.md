# Dolphin evidence report

Probe `input_probe` reported `probe_fail`.
Passed checks 58. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si::init | SI wrapper repeat init preserves the active GameCube channel | PASS | [log L141](input_probe/probe.log#L141), [page-004](input_probe/pages/page-004.png) |
| ogc-si::transfer_commands | SI wrapper transfers commands and retains the active GameCube channel | FAIL | [log L142](input_probe/probe.log#L142), [page-004](input_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-si::transfer_commands — SI wrapper transfers commands and retains the active GameCube channel (FAIL)
- Other failed check — input collection completed without source, capture, or cleanup errors
