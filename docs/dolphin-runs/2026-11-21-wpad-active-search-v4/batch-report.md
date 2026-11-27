# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 96. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD guest search enters inquiry state | FAIL | [log L120](wpad_probe/probe.log#L120), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search clears guest inquiry state | PASS | [log L121](wpad_probe/probe.log#L121), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search clears general inquiry state | PASS | [log L124](wpad_probe/probe.log#L124), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID search enters inquiry state | FAIL | [log L123](wpad_probe/probe.log#L123), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::search — WPAD guest search enters inquiry state (FAIL)
- wpad-sys::search_bluetooth_hid — WPAD HID search enters inquiry state (FAIL)
