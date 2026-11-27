# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 98. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD guest search queues an inquiry and marks it active | PASS | [log L119](wpad_probe/probe.log#L119), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search clears guest inquiry state | PASS | [log L120](wpad_probe/probe.log#L120), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search clears general inquiry state | PASS | [log L122](wpad_probe/probe.log#L122), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID search queues a general inquiry and marks it active | PASS | [log L121](wpad_probe/probe.log#L121), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
