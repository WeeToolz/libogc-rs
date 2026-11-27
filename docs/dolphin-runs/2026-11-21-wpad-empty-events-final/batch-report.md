# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 59. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::read_event | WPAD empty channel read reports no controller | PASS | [log L102](wpad_probe/probe.log#L102), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::flush | WPAD empty channel flush reports no controller | PASS | [log L103](wpad_probe/probe.log#L103), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::dropped_events | WPAD empty channel dropped-event count is zero | PASS | [log L104](wpad_probe/probe.log#L104), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
