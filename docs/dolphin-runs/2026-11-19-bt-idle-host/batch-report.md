# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 36. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | Bluetooth idle host starts disabled | PASS | [log L86](general_probe/probe.log#L86), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::diagnostics | Bluetooth idle host starts disabled | PASS | [log L86](general_probe/probe.log#L86), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::diagnostics | Bluetooth idle host shutdown completes without adapter | PASS | [log L91](general_probe/probe.log#L91), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::search | Bluetooth idle host rejects search and report | PASS | [log L87](general_probe/probe.log#L87), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::stop_search | Bluetooth idle host has no queued events | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::send_report | Bluetooth idle host rejects search and report | PASS | [log L87](general_probe/probe.log#L87), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::disconnect | Bluetooth idle host has no queued events | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::disconnect | Bluetooth idle host rejects disconnect request | PASS | [log L88](general_probe/probe.log#L88), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::request_disconnect | Bluetooth idle host rejects disconnect request | PASS | [log L88](general_probe/probe.log#L88), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::switch_calibration | Bluetooth idle host rejects disconnect request | PASS | [log L88](general_probe/probe.log#L88), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::peek_event | Bluetooth idle host has no queued events | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::next_event | Bluetooth idle host has no queued events | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::poll | Bluetooth idle host poll leaves it disabled | PASS | [log L90](general_probe/probe.log#L90), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::poll | Bluetooth closed host remains complete after poll | PASS | [log L92](general_probe/probe.log#L92), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::shutdown | Bluetooth idle host shutdown completes without adapter | PASS | [log L91](general_probe/probe.log#L91), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::shutdown_complete | Bluetooth idle host starts disabled | PASS | [log L86](general_probe/probe.log#L86), [page-003](general_probe/pages/page-003.png) |
| ogc-bt-sync::shutdown_complete | Bluetooth idle host shutdown completes without adapter | PASS | [log L91](general_probe/probe.log#L91), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Idle host with no adapter or connected controllers; pairing, transfer, event delivery and physical hardware are not covered.
