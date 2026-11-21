# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 94. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_status_callback | WPAD status callback receives valid shutdown channel notifications | PASS | [log L172](wpad_probe/probe.log#L172), [page-006](wpad_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
