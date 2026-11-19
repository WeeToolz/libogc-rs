# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 567. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::from_file_descriptor | Bluetooth transport keeps supplied descriptor and endpoints | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::fd | Bluetooth transport keeps supplied descriptor and endpoints | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::acl_out_endpoint | Bluetooth transport keeps supplied descriptor and endpoints | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::event_in_endpoint | Bluetooth transport keeps supplied descriptor and endpoints | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::acl_in_endpoint | Bluetooth transport keeps supplied descriptor and endpoints | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::parse_event | Bluetooth transport parses and rejects event frames | PASS | [log L208](runtime_probe/probe.log#L208), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::pin_code_request_reply | Bluetooth transport rejects oversized PIN before submission | PASS | [log L209](runtime_probe/probe.log#L209), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::open_with_endpoints | Bluetooth transport rejects invalid endpoints before open | PASS | [log L210](runtime_probe/probe.log#L210), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Synthetic descriptor and invalid-input checks only; no adapter open, IOS transfer, PIN submission, or physical controller behavior is covered.
