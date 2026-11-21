# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 74. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::control_speaker | WPAD empty channel speaker control succeeds | PASS | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::control_speaker | WPAD invalid channel rejects speaker control | PASS | [log L116](wpad_probe/probe.log#L116), [page-004](wpad_probe/pages/page-004.png), [log L121](wpad_probe/probe.log#L121), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::rumble | WPAD empty channel rumble control succeeds | PASS | [log L115](wpad_probe/probe.log#L115), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::rumble | WPAD invalid channel rejects rumble control | PASS | [log L117](wpad_probe/probe.log#L117), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin has no connected Wii Remote. The checks cover only valid empty-channel no-ops and invalid-channel rejection.
