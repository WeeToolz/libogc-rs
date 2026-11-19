# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 457. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | Bluetooth opcode packs group and masked command | PASS | [log L206](runtime_probe/probe.log#L206), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::raw | Bluetooth opcode packs group and masked command | PASS | [log L206](runtime_probe/probe.log#L206), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::request_disconnect | Bluetooth disconnected slots reject disconnect requests | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::disconnect | Bluetooth disconnected slots reject disconnect requests | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::switch_calibration | Bluetooth disconnected slots reject disconnect requests | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin opcode construction and disconnected host slots. Live adapter, active links, calibration and physical Wii remain unverified.
