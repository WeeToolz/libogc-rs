# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 64. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_data_format | WPAD empty channel data format update succeeds | PASS | [log L100](wpad_probe/probe.log#L100), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_v_res | WPAD empty channel IR resolution update succeeds | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
