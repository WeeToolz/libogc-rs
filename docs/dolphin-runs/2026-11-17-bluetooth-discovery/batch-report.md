# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 282. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::from_name | Bluetooth discovery classifies controller names | PASS | [log L124](runtime_probe/probe.log#L124), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::is_switch | Bluetooth discovery classifies controller names | PASS | [log L124](runtime_probe/probe.log#L124), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::new | Bluetooth HCI endpoints validate direction and distinct inputs | PASS | [log L125](runtime_probe/probe.log#L125), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Name matching and endpoint address validation in Dolphin. Bluetooth discovery, USB descriptors, and physical devices need separate checks.
