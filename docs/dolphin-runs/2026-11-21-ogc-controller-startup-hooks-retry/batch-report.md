# Dolphin evidence report

Probe `input_probe` reported `probe_fail`.
Passed checks 43. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::init | controller manager init produces a clean first scan | FAIL | [log L75](input_probe/probe.log#L75), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::enable_bluetooth_hid | controller Bluetooth HID startup request accepted | PASS | [log L76](input_probe/probe.log#L76), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::set_bluetooth_startup_hook | Bluetooth startup hook receives stage progress | PASS | [log L169](input_probe/probe.log#L169), [page-006](input_probe/pages/page-006.png) |
| ogc-controller::set_input_trace_observer | input trace observer receives controller events | PASS | [log L171](input_probe/probe.log#L171), [page-006](input_probe/pages/page-006.png) |
| ogc-controller::set_usb_startup_hook | USB startup hook receives stage messages | PASS | [log L170](input_probe/probe.log#L170), [page-006](input_probe/pages/page-006.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-controller::init — controller manager init produces a clean first scan (FAIL)
- Other failed check — input collection completed without source, capture, or cleanup errors

## Coverage limits

- Dolphin supplies scripted GameCube input and emulated Bluetooth startup. It does not verify callbacks from a physical Bluetooth controller or USB HID device.
