# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 47. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::get_device_id | ES device ID matches imported NAND and rejects null | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_title_id | ES title ID unavailable preserves output and rejects null | PASS | [log L90](general_probe/probe.log#L90), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::check_has_korean_key | ES Korean key query returns IOS error | PASS | [log L91](general_probe/probe.log#L91), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Device ID expectation belongs to the imported NAND fixture. Title ID and Korean-key queries exercise Dolphin error paths only.
