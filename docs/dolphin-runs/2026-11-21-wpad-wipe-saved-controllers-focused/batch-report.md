# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 90. Failed checks 3.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::wipe_saved_controllers | WPAD wipe seeds saved controller records | FAIL | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe clears saved controller and guest records | FAIL | [log L102](wpad_probe/probe.log#L102), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe restores original saved records | FAIL | [log L103](wpad_probe/probe.log#L103), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::wipe_saved_controllers — WPAD wipe seeds saved controller records (FAIL)
- wpad-sys::wipe_saved_controllers — WPAD wipe clears saved controller and guest records (FAIL)
- wpad-sys::wipe_saved_controllers — WPAD wipe restores original saved records (FAIL)
