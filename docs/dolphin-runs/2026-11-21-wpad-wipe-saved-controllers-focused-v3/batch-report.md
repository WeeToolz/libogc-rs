# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 93. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::wipe_saved_controllers | WPAD wipe seeds saved controller records | PASS | [log L107](wpad_probe/probe.log#L107), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe clears saved controller and guest records | PASS | [log L108](wpad_probe/probe.log#L108), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::wipe_saved_controllers | WPAD wipe restores original saved records | PASS | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
