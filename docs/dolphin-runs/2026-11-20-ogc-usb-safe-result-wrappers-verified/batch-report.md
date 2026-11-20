# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 607. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::device_description | USB device description maps invalid handle to error | PASS | [log L264](runtime_probe/probe.log#L264), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::descriptors | USB descriptor query maps invalid handle to error | PASS | [log L265](runtime_probe/probe.log#L265), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::generic_descriptor | USB generic descriptor maps invalid handle to error | PASS | [log L266](runtime_probe/probe.log#L266), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::hid_descriptor | USB HID descriptor maps invalid handle to error | PASS | [log L267](runtime_probe/probe.log#L267), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::report_descriptor_size | USB report descriptor size maps invalid handle to error | PASS | [log L268](runtime_probe/probe.log#L268), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::report_descriptor | USB report descriptor maps invalid handle to error | PASS | [log L269](runtime_probe/probe.log#L269), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::ascii_string | USB ASCII string maps invalid handle to error | PASS | [log L270](runtime_probe/probe.log#L270), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
