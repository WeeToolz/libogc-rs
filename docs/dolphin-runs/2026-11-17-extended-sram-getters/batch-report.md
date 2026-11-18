# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 301. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::get_wireless_id | Wireless ID getter reads each extended SRAM channel | PASS | [log L199](runtime_probe/probe.log#L199), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system::get_wireless_id | Extended SRAM fixture lock and restoration | PASS | [log L201](runtime_probe/probe.log#L201), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system::get_gbs_mode | GBS getter reads extended SRAM | PASS | [log L200](runtime_probe/probe.log#L200), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system::get_gbs_mode | Extended SRAM fixture lock and restoration | PASS | [log L201](runtime_probe/probe.log#L201), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin extended SRAM buffer reads and invalid channel handling. Persistent writes and physical hardware require separate checks.
