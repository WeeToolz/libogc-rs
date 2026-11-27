# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 573. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::close | Bluetooth close paths report invalid descriptor errors | FAIL | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::close_async | Bluetooth close paths report invalid descriptor errors | FAIL | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::command_complete | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::create_connection | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::link_key_request_negative_reply | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::link_key_request_reply | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::pin_code_request_negative_reply | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_acl | Bluetooth sync reads and writes propagate invalid descriptor errors | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_acl_packet | Bluetooth sync reads and writes propagate invalid descriptor errors | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_bd_addr | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_event | Bluetooth sync reads and writes propagate invalid descriptor errors | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_event_async | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::read_acl_async | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::write_acl_async | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::remote_name_request | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::reset | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::scan_inquiry | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command | Bluetooth sync reads and writes propagate invalid descriptor errors | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async_owned | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_hci_command_async_with | Bluetooth async transfers reject invalid descriptor submissions | FAIL | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_l2cap_configuration_request | Bluetooth L2CAP send helpers propagate invalid descriptor errors | PASS | [log L214](runtime_probe/probe.log#L214), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_l2cap_configuration_request_with_mtu | Bluetooth L2CAP send helpers propagate invalid descriptor errors | PASS | [log L214](runtime_probe/probe.log#L214), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_l2cap_configuration_response | Bluetooth L2CAP send helpers propagate invalid descriptor errors | PASS | [log L214](runtime_probe/probe.log#L214), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_l2cap_connection_request | Bluetooth L2CAP send helpers propagate invalid descriptor errors | PASS | [log L214](runtime_probe/probe.log#L214), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_l2cap_hid_data | Bluetooth L2CAP send helpers propagate invalid descriptor errors | PASS | [log L214](runtime_probe/probe.log#L214), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::set_authentication_enable | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::set_event_mask | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::set_scan_enable | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::start_inquiry | Bluetooth HCI helpers propagate invalid descriptor errors | PASS | [log L213](runtime_probe/probe.log#L213), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::write_acl | Bluetooth sync reads and writes propagate invalid descriptor errors | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-bt-sync::close — Bluetooth close paths report invalid descriptor errors (FAIL)
- ogc-bt-sync::close_async — Bluetooth close paths report invalid descriptor errors (FAIL)
- ogc-bt-sync::read_event_async — Bluetooth async transfers reject invalid descriptor submissions (FAIL)
- ogc-bt-sync::read_acl_async — Bluetooth async transfers reject invalid descriptor submissions (FAIL)
- ogc-bt-sync::write_acl_async — Bluetooth async transfers reject invalid descriptor submissions (FAIL)
- ogc-bt-sync::send_hci_command_async — Bluetooth async transfers reject invalid descriptor submissions (FAIL)
- ogc-bt-sync::send_hci_command_async_owned — Bluetooth async transfers reject invalid descriptor submissions (FAIL)
- ogc-bt-sync::send_hci_command_async_with — Bluetooth async transfers reject invalid descriptor submissions (FAIL)

## Coverage limits

- The probe uses descriptor -1 to check error and argument-validation paths. It does not test successful IOS submissions, adapter opening, callback completion, live Bluetooth traffic, or physical Wii hardware.
