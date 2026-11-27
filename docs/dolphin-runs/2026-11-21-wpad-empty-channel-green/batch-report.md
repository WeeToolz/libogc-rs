# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 72. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::control_speaker | WPAD empty channel speaker control succeeds | PASS | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::rumble | WPAD empty channel rumble control succeeds | PASS | [log L115](wpad_probe/probe.log#L115), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin has no connected Wii Remote. This run checks only valid empty-channel results.
