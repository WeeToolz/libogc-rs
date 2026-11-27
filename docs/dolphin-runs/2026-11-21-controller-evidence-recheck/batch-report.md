# Dolphin evidence report

Probe `input_probe` reported `probe_fail`.
Passed checks 46. Failed checks 3.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::init | controller manager init produces a clean first scan | PASS | [log L77](input_probe/probe.log#L77), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::init | controller discovered | PASS | [log L123](input_probe/probe.log#L123), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::init | A transition observed | PASS | [log L124](input_probe/probe.log#L124), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | controller Bluetooth HID startup request accepted | PASS | [log L75](input_probe/probe.log#L75), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | direct Bluetooth HID startup reaches Ready before the WPAD alias runs | PASS | [log L162](input_probe/probe.log#L162), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook reports initialization before Ready | PASS | [log L163](input_probe/probe.log#L163), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook stays silent during restart after removal | Missing |  |
| ogc-controller::set_input_trace_observer | input trace observer receives the scripted GameCube A press with identity and time | PASS | [log L165](input_probe/probe.log#L165), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_input_trace_observer | USB and trace observers stop receiving callbacks after removal | PASS | [log L166](input_probe/probe.log#L166), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_usb_startup_hook | USB startup hook reports initialization and enumeration in order | PASS | [log L164](input_probe/probe.log#L164), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_usb_startup_hook | USB and trace observers stop receiving callbacks after removal | PASS | [log L166](input_probe/probe.log#L166), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::enable_wpad | WPAD alias restarts the stopped Bluetooth service | Missing |  |
| ogc-controller::shutdown_bluetooth | Bluetooth shutdown starts with an active adapter | PASS | [log L167](input_probe/probe.log#L167), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth shutdown request accepted | PASS | [log L169](input_probe/probe.log#L169), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth close is pending after submission | FAIL | [log L170](input_probe/probe.log#L170), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_shutdown_complete | Bluetooth close is pending after submission | FAIL | [log L170](input_probe/probe.log#L170), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_shutdown_complete | Bluetooth shutdown completed | PASS | [log L172](input_probe/probe.log#L172), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_diagnostics | Bluetooth diagnostics retain the closed adapter state | PASS | [log L173](input_probe/probe.log#L173), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_diagnostics | Bluetooth transport has no terminal failure | PASS | [log L174](input_probe/probe.log#L174), [page-004](input_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-controller::set_bluetooth_startup_hook — Bluetooth startup hook stays silent during restart after removal (Missing)
- ogc-controller::enable_wpad — WPAD alias restarts the stopped Bluetooth service (Missing)
- ogc-controller::shutdown_bluetooth — Bluetooth close is pending after submission (FAIL)
- ogc-controller::bluetooth_shutdown_complete — Bluetooth close is pending after submission (FAIL)
- Other failed check — controller manager shutdown completed
- Other failed check — input collection completed without source, capture, or cleanup errors

## Coverage limits

- Dolphin supplies scripted GameCube input and emulated Bluetooth transport. The trace check covers decoded GameCube events. USB enumeration can be empty. Physical input, real USB reads, physical Bluetooth pairing and the event and diagnostics hooks require separate evidence.
