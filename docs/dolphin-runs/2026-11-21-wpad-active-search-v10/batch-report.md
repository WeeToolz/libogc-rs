# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 98. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD guest search enters inquiry state | PASS | [log L174](wpad_probe/probe.log#L174), [page-006](wpad_probe/pages/page-006.png) |
| wpad-sys::stop_search | WPAD stop_search clears guest inquiry state | PASS | [log L183](wpad_probe/probe.log#L183), [page-007](wpad_probe/pages/page-007.png) |
| wpad-sys::stop_search | WPAD stop_search clears general inquiry state | PASS | [log L195](wpad_probe/probe.log#L195), [page-007](wpad_probe/pages/page-007.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID search enters general inquiry state | PASS | [log L184](wpad_probe/probe.log#L184), [page-007](wpad_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
