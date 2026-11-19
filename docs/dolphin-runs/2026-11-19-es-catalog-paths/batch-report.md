# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 42. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::get_data_dir | ES system menu data path matches NAND | PASS | [log L85](general_probe/probe.log#L85), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_stored_tmd_contents | ES rejected stored content list preserves output | PASS | [log L86](general_probe/probe.log#L86), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Uses the imported Dolphin NAND system menu; stored content listing is rejected by Dolphin and successful listing remains open.
