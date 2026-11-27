# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 81. Failed checks 18.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::search | WPAD guest search enters inquiry state | PASS | [log L119](wpad_probe/probe.log#L119), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search cleans up after guest inquiry failure | FAIL | [log L125](wpad_probe/probe.log#L125), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::stop_search | WPAD stop_search clears an active general inquiry | FAIL | [log L133](wpad_probe/probe.log#L133), [page-005](wpad_probe/pages/page-005.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID search enters general inquiry state | PASS | [log L126](wpad_probe/probe.log#L126), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::search_bluetooth_hid | WPAD HID inquiry completes without command errors | FAIL | [log L132](wpad_probe/probe.log#L132), [page-005](wpad_probe/pages/page-005.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::stop_search — WPAD stop_search cleans up after guest inquiry failure (FAIL)
- wpad-sys::stop_search — WPAD stop_search clears an active general inquiry (FAIL)
- wpad-sys::search_bluetooth_hid — WPAD HID inquiry completes without command errors (FAIL)
- Other failed check — WPAD setter checks confirm an empty channel
- Other failed check — WPAD empty channel data format update returns success
- Other failed check — WPAD empty channel ignores an invalid data format
- Other failed check — WPAD empty channel MotionPlus configuration succeeds
- Other failed check — WPAD all-channel MotionPlus configuration succeeds without controllers
- Other failed check — WPAD disconnect checks confirm an empty channel
- Other failed check — WPAD empty channel disconnect succeeds
- Other failed check — WPAD empty channel speaker control succeeds
- Other failed check — WPAD empty channel rumble control succeeds
- Other failed check — WPAD stream probe confirms empty channel
- Other failed check — WPAD empty channel speaker stream succeeds
- Other failed check — WPAD empty channel read reports no controller
- Other failed check — WPAD empty channel flush reports no controller
- Other failed check — empty channel probe preserves output on no controller
- Other failed check — empty channel event read reports no controller
