# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 56. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::data | WPAD empty channel data pointer is available | PASS | [log L96](wpad_probe/probe.log#L96), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::battery_level | WPAD empty channel battery level is zero | PASS | [log L97](wpad_probe/probe.log#L97), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::buttons_down | WPAD empty channel button states are zero | PASS | [log L98](wpad_probe/probe.log#L98), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::buttons_held | WPAD empty channel button states are zero | PASS | [log L98](wpad_probe/probe.log#L98), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::buttons_up | WPAD empty channel button states are zero | PASS | [log L98](wpad_probe/probe.log#L98), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::accel | WPAD empty channel acceleration is zero | PASS | [log L99](wpad_probe/probe.log#L99), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::ir | WPAD empty channel infrared state is zero | PASS | [log L100](wpad_probe/probe.log#L100), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::expansion | WPAD empty channel expansion type is none | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
