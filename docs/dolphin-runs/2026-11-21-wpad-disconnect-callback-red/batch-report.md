# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 98. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_disconnect_callback | WPAD disconnect callback receives a valid Bluetooth disconnect channel | FAIL | [log L206](wpad_probe/probe.log#L206), [page-009](wpad_probe/pages/page-009.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::set_disconnect_callback — WPAD disconnect callback receives a valid Bluetooth disconnect channel (FAIL)
