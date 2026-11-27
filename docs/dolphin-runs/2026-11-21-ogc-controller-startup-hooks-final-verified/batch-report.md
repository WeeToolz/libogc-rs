# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 45. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::init | controller manager init produces a clean first scan | PASS | [log L78](input_probe/probe.log#L78), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | controller Bluetooth HID startup request accepted | PASS | [log L75](input_probe/probe.log#L75), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook receives stage progress | PASS | [log L169](input_probe/probe.log#L169), [page-005](input_probe/pages/page-005.png) |
| ogc-controller::set_input_trace_observer | input trace observer receives controller events | PASS | [log L171](input_probe/probe.log#L171), [page-005](input_probe/pages/page-005.png) |
| ogc-controller::set_usb_startup_hook | USB startup hook receives stage messages | PASS | [log L170](input_probe/probe.log#L170), [page-005](input_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin supplies scripted GameCube input and emulated Bluetooth startup. It does not verify callbacks from a physical Bluetooth controller or USB HID device.
