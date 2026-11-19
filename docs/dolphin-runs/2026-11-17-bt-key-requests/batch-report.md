# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 286. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::load_link_key | Bluetooth link key requests preserve keys and reject unknown peers | PASS | [log L130](runtime_probe/probe.log#L130), [page-005](runtime_probe/pages/page-005.png) |
| ogc-bt-sync::reset_preserving_keys | Bluetooth link key requests preserve keys and reject unknown peers | PASS | [log L130](runtime_probe/probe.log#L130), [page-005](runtime_probe/pages/page-005.png) |
| ogc-bt-sync::receive_event | Bluetooth link key requests preserve keys and reject unknown peers | PASS | [log L130](runtime_probe/probe.log#L130), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Controlled HCI events and host state in Dolphin. Persistent storage and physical Bluetooth transport require separate evidence.
