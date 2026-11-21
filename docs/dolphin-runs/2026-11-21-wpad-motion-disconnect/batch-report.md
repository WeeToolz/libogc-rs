# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 89. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_motion_plus | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_motion_plus | WPAD empty channel MotionPlus configuration succeeds | PASS | [log L111](wpad_probe/probe.log#L111), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_motion_plus | WPAD all-channel MotionPlus configuration succeeds without controllers | PASS | [log L112](wpad_probe/probe.log#L112), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_motion_plus | WPAD invalid channel MotionPlus configuration is rejected | PASS | [log L113](wpad_probe/probe.log#L113), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_motion_plus | WPAD disabled adapter rejects MotionPlus configuration | PASS | [log L164](wpad_probe/probe.log#L164), [page-006](wpad_probe/pages/page-006.png) |
| wpad-sys::set_motion_plus | WPAD disabled adapter rejects all-channel MotionPlus configuration | PASS | [log L165](wpad_probe/probe.log#L165), [page-006](wpad_probe/pages/page-006.png) |
| wpad-sys::disconnect | WPAD disconnect checks confirm an empty channel | PASS | [log L119](wpad_probe/probe.log#L119), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::disconnect | WPAD empty channel disconnect succeeds | PASS | [log L120](wpad_probe/probe.log#L120), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::disconnect | WPAD invalid channel disconnect is rejected | PASS | [log L125](wpad_probe/probe.log#L125), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::disconnect | WPAD disabled adapter rejects disconnect | PASS | [log L163](wpad_probe/probe.log#L163), [page-006](wpad_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- No Wii Remote is connected in Dolphin. The checks cover empty-channel, invalid-channel, disabled-adapter, and all-channel MotionPlus dispatch behavior. Controller disconnect and MotionPlus changes with a connected Remote remain unverified.
