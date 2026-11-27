# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 90. Failed checks 3.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::wipe_saved_controllers | WPAD wipe seeds saved controller records | FAIL | [log L107](wpad_probe/probe.log#L107), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe clears saved controller and guest records | FAIL | [log L108](wpad_probe/probe.log#L108), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe restores original saved records | FAIL | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::wipe_saved_controllers — WPAD wipe seeds saved controller records (FAIL)
- wpad-sys::wipe_saved_controllers — WPAD wipe clears saved controller and guest records (FAIL)
- wpad-sys::wipe_saved_controllers — WPAD wipe restores original saved records (FAIL)
