# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 96. Failed checks 3.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD guest search enters inquiry state | PASS | [log L174](wpad_probe/probe.log#L174), [page-006](wpad_probe/pages/page-006.png) |
| wpad-sys::stop_search | WPAD stop_search cleans up after guest inquiry failure | FAIL | [log L183](wpad_probe/probe.log#L183), [page-007](wpad_probe/pages/page-007.png) |
| wpad-sys::stop_search | WPAD stop_search clears an active general inquiry | FAIL | [log L192](wpad_probe/probe.log#L192), [page-007](wpad_probe/pages/page-007.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID search enters general inquiry state | PASS | [log L184](wpad_probe/probe.log#L184), [page-007](wpad_probe/pages/page-007.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID inquiry completes without command errors | FAIL | [log L190](wpad_probe/probe.log#L190), [page-007](wpad_probe/pages/page-007.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::stop_search — WPAD stop_search cleans up after guest inquiry failure (FAIL)
- wpad-sys::stop_search — WPAD stop_search clears an active general inquiry (FAIL)
- wpad-sys::search_bluetooth_hid — WPAD HID inquiry completes without command errors (FAIL)
