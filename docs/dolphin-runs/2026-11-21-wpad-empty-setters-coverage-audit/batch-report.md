# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 78. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_data_format | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_data_format | WPAD empty channel data format update returns success | PASS | [log L107](wpad_probe/probe.log#L107), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_v_res | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_v_res | WPAD empty channel retains IR resolution before Remote connection | PASS | [log L108](wpad_probe/probe.log#L108), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_idle_thresholds | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_idle_thresholds | WPAD empty channel idle thresholds update returns success | PASS | [log L109](wpad_probe/probe.log#L109), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The probe has no connected Wii Remote. It checks setter results in a valid empty channel and retains the Rust IR coordinate setting in memory. It does not test data-format reports, idle-threshold filtering, or camera output with a Remote.
