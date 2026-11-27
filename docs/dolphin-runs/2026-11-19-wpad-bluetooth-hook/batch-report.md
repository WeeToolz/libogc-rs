# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 41. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::bluetooth_diagnostics | WPAD Bluetooth diagnostics report ready adapter | PASS | [log L102](wpad_probe/probe.log#L102), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_hid_next_event | WPAD Bluetooth event queue is empty without HID devices | PASS | [log L104](wpad_probe/probe.log#L104), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_hid_state | WPAD Bluetooth invalid HID slot is empty | PASS | [log L103](wpad_probe/probe.log#L103), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_shutdown_complete | WPAD Bluetooth shutdown query reports complete | PASS | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::bluetooth_switch_calibration | WPAD Switch calibration rejects invalid slot | PASS | [log L105](wpad_probe/probe.log#L105), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_bluetooth_poll_hook | WPAD Bluetooth poll hook observes scan boundary | PASS | [log L87](wpad_probe/probe.log#L87), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin adapter with no connected Bluetooth HID controller only. Pairing, calibration with a Switch controller and physical Wii remain unverified.
