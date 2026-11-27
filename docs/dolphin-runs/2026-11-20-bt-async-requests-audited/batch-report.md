# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 578. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::close | Bluetooth synchronous close maps the IPC descriptor result | PASS | [log L219](runtime_probe/probe.log#L219), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::close_async | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::copy_to | Bluetooth async transfers publish consistent request results | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::is_complete | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::raw_result | Bluetooth async transfers publish consistent request results | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::raw_result | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::result | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::status | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_acl_async | Bluetooth async transfers publish consistent request results | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_event_async | Bluetooth async transfers publish consistent request results | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::write_acl_async | Bluetooth async transfers publish consistent request results | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async | Bluetooth async callbacks report invalid descriptor completion | PASS | [log L218](runtime_probe/probe.log#L218), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async | Bluetooth raw async command rejects a missing completion callback | PASS | [log L220](runtime_probe/probe.log#L220), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async_owned | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async_with | Bluetooth owned command and close results match request status | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async_with | Bluetooth async callbacks report invalid descriptor completion | PASS | [log L218](runtime_probe/probe.log#L218), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The test uses descriptor -1 and checks IPC completion state, request result mapping, callback delivery, and synchronous close mapping. It does not test successful adapter transfers, valid Bluetooth frames, pairing, or physical Wii hardware.
