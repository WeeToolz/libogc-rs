# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 80. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_data_format | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_data_format | WPAD empty channel ignores an invalid data format | PASS | [log L108](wpad_probe/probe.log#L108), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_data_format | WPAD disabled adapter rejects data format | PASS | [log L156](wpad_probe/probe.log#L156), [page-006](wpad_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin has no connected Remote. It tests the C empty-channel no-op and disabled-state result. Data-format reports after connection remain unverified.
