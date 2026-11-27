# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 78. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_data_format | WPAD setter checks confirm an empty channel | PASS | [log L101](wpad_probe/probe.log#L101), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_data_format | WPAD empty channel ignores an invalid data format | FAIL | [log L108](wpad_probe/probe.log#L108), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::set_data_format | WPAD disabled adapter rejects data format | FAIL | [log L156](wpad_probe/probe.log#L156), [page-007](wpad_probe/pages/page-007.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::set_data_format — WPAD empty channel ignores an invalid data format (FAIL)
- wpad-sys::set_data_format — WPAD disabled adapter rejects data format (FAIL)

## Coverage limits

- Dolphin has no connected Remote. It tests the C empty-channel no-op and disabled-state result. Data-format reports after connection remain unverified.
