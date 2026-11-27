# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 61. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::g_force | WPAD empty channel g-force is zero | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
