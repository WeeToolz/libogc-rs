# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 37. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::next_cert | ES next certificate follows all supported ABI layouts | PASS | [log L83](general_probe/probe.log#L83), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Structurally valid synthetic certificate layouts only; signatures are not authenticated and no IOS or physical hardware operations are covered.
