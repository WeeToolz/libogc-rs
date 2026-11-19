# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 285. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::diagnostics | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::next_event | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::peek_event | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::next_outgoing | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::search | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::switch_calibration | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::adapter_opening | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::start | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::fail_timeout | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::fail_transport | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::reset_preserving_keys | Bluetooth host startup and failure lifecycle | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Host startup and failure state in Dolphin. Adapter command completion, persistent key retention, and physical Bluetooth require separate checks.
