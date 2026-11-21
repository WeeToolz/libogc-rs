# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 634. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwbt-sys::write_stored_link_key | Bluetooth key write rejects null address | PASS | [log L236](runtime_probe/probe.log#L236), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::delete_stored_link_key | Bluetooth key delete rejects null address | PASS | [log L237](runtime_probe/probe.log#L237), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::link_key_request_reply | Bluetooth key reply rejects null address | PASS | [log L238](runtime_probe/probe.log#L238), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::link_key_request_negative_reply | Bluetooth negative key reply rejects null address | PASS | [log L239](runtime_probe/probe.log#L239), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::read_stored_link_key | Bluetooth key read rejects null output | PASS | [log L240](runtime_probe/probe.log#L240), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
