# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 305. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::get_font_encoding | System hardware getters match Dolphin default values | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::reset_button_down | System hardware getters match Dolphin default values | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::get_hollywood_revision | System hardware getters match Dolphin default values | PASS | [log L144](runtime_probe/probe.log#L144), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Default Dolphin configuration only. Pressed reset input, other font regions, and physical hardware need separate evidence.
