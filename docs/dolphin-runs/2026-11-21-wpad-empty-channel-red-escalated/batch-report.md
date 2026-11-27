# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 70. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::control_speaker | WPAD empty channel speaker control succeeds | FAIL | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::rumble | WPAD empty channel rumble control succeeds | FAIL | [log L115](wpad_probe/probe.log#L115), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::control_speaker — WPAD empty channel speaker control succeeds (FAIL)
- wpad-sys::rumble — WPAD empty channel rumble control succeeds (FAIL)

## Coverage limits

- Dolphin has no connected Wii Remote. This run checks only valid empty-channel results.
