# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 73. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::get_consumption_count | ES consumption count returns zero and rejects null | PASS | [log L95](general_probe/probe.log#L95), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_consumption_count | ES consumption queries reject closed service | PASS | [log L97](general_probe/probe.log#L97), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_consumption | ES consumption validates output and preserves empty results | PASS | [log L96](general_probe/probe.log#L96), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_consumption | ES consumption queries reject closed service | PASS | [log L97](general_probe/probe.log#L97), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin returns zero limits for the selected ticket ID. Populated consumption records, real-ticket authorization, and physical hardware remain untested.
