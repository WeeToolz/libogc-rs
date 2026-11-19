# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 284. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | L2CAP channel readiness requires a remote channel | PASS | [log L126](runtime_probe/probe.log#L126), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::ready | L2CAP channel readiness requires a remote channel | PASS | [log L126](runtime_probe/probe.log#L126), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::new | L2CAP HID state tracks both configured channels | PASS | [log L127](runtime_probe/probe.log#L127), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::ready | L2CAP HID state tracks both configured channels | PASS | [log L127](runtime_probe/probe.log#L127), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::observe_signal | L2CAP HID state tracks both configured channels | PASS | [log L127](runtime_probe/probe.log#L127), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- L2CAP signaling state checks in Dolphin with fixed packet payloads. Bluetooth transport and physical devices require separate evidence.
