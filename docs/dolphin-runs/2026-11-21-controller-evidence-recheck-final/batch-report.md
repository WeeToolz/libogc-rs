# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 58. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::init | controller manager init produces a clean first scan | PASS | [log L77](input_probe/probe.log#L77), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::init | controller discovered | PASS | [log L123](input_probe/probe.log#L123), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::init | A transition observed | PASS | [log L124](input_probe/probe.log#L124), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | controller Bluetooth HID startup request accepted | PASS | [log L75](input_probe/probe.log#L75), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | direct Bluetooth HID startup reaches Ready before the WPAD alias runs | PASS | [log L164](input_probe/probe.log#L164), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook reports initialization before Ready | PASS | [log L165](input_probe/probe.log#L165), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook stays silent during restart after removal | PASS | [log L178](input_probe/probe.log#L178), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_input_trace_observer | input trace observer receives the scripted GameCube A press with identity and time | PASS | [log L167](input_probe/probe.log#L167), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_input_trace_observer | USB and trace observers stop receiving callbacks after removal | PASS | [log L168](input_probe/probe.log#L168), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_usb_startup_hook | USB startup hook reports initialization and enumeration in order | PASS | [log L166](input_probe/probe.log#L166), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::set_usb_startup_hook | USB and trace observers stop receiving callbacks after removal | PASS | [log L168](input_probe/probe.log#L168), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::enable_wpad | WPAD alias restarts the stopped Bluetooth service | PASS | [log L177](input_probe/probe.log#L177), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth shutdown starts with an active adapter | PASS | [log L169](input_probe/probe.log#L169), [page-004](input_probe/pages/page-004.png), [log L179](input_probe/probe.log#L179), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth shutdown request accepted | PASS | [log L171](input_probe/probe.log#L171), [page-004](input_probe/pages/page-004.png), [log L181](input_probe/probe.log#L181), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth diagnostics retain the closed adapter state | PASS | [log L175](input_probe/probe.log#L175), [page-004](input_probe/pages/page-004.png), [log L186](input_probe/probe.log#L186), [page-005](input_probe/pages/page-005.png) |
| ogc-controller::bluetooth_shutdown_complete | Bluetooth shutdown starts with an active adapter | PASS | [log L169](input_probe/probe.log#L169), [page-004](input_probe/pages/page-004.png), [log L179](input_probe/probe.log#L179), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_shutdown_complete | Bluetooth shutdown completed | PASS | [log L174](input_probe/probe.log#L174), [page-004](input_probe/pages/page-004.png), [log L185](input_probe/probe.log#L185), [page-005](input_probe/pages/page-005.png) |
| ogc-controller::bluetooth_diagnostics | Bluetooth diagnostics retain the closed adapter state | PASS | [log L175](input_probe/probe.log#L175), [page-004](input_probe/pages/page-004.png), [log L186](input_probe/probe.log#L186), [page-005](input_probe/pages/page-005.png) |
| ogc-controller::bluetooth_diagnostics | Bluetooth transport has no terminal failure | PASS | [log L176](input_probe/probe.log#L176), [page-004](input_probe/pages/page-004.png), [log L187](input_probe/probe.log#L187), [page-005](input_probe/pages/page-005.png) |
| ogc-input::init | PAD repeated initialization reports debug stages | PASS | [log L141](input_probe/probe.log#L141), [page-003](input_probe/pages/page-003.png), [log L154](input_probe/probe.log#L154), [page-003](input_probe/pages/page-003.png) |
| ogc-si::init | SI wrapper repeat init preserves the active GameCube channel | PASS | [log L139](input_probe/probe.log#L139), [page-003](input_probe/pages/page-003.png) |
| ogc-si-sys::init | SI driver repeat init preserves the active GameCube channel | PASS | [log L140](input_probe/probe.log#L140), [page-003](input_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin supplies scripted GameCube input and emulated Bluetooth transport. The trace check covers decoded GameCube events. USB enumeration can be empty. Physical input, real USB reads, physical Bluetooth pairing and the event and diagnostics hooks require separate evidence.
- The runtime starts SI before the input probe. The explicit SI assertions cover repeat initialization on an active channel. They do not exercise either entry point in isolation from a cold boot.
- Bluetooth shutdown requires a ready host before submission and a closed adapter afterward. The report records whether the first completion poll saw a pending close.
