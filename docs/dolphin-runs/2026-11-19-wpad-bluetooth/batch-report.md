# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 39. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::bluetooth_diagnostics | WPAD Bluetooth diagnostics report ready adapter | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_hid_next_event | WPAD Bluetooth event queue is empty without HID devices | PASS | [log L103](wpad_probe/probe.log#L103), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_hid_state | WPAD Bluetooth invalid HID slot is empty | PASS | [log L102](wpad_probe/probe.log#L102), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::bluetooth_shutdown_complete | WPAD Bluetooth shutdown query reports complete | PASS | [log L114](wpad_probe/probe.log#L114), [page-005](wpad_probe/pages/page-005.png) |
| wpad-sys::bluetooth_switch_calibration | WPAD Switch calibration rejects invalid slot | PASS | [log L104](wpad_probe/probe.log#L104), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_bluetooth_poll_hook | WPAD Bluetooth poll hook observes scan boundary | PASS | [log L105](wpad_probe/probe.log#L105), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — empty channel probe preserves output on no controller
- Other failed check — empty channel event read reports no controller

## Coverage limits

- Dolphin adapter with no connected Bluetooth HID controller only. Pairing, calibration with a Switch controller and physical Wii remain unverified.
