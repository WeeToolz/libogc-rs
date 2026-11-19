# Dolphin evidence report

Probe `hid_descriptor_probe` reported `pass`.
Passed checks 16. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::open_device | USB open rejects null output | PASS | [log L74](hid_descriptor_probe/probe.log#L74), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::close_device | USB close rejects null output | PASS | [log L75](hid_descriptor_probe/probe.log#L75), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_device_description | USB device description rejects null output | PASS | [log L76](hid_descriptor_probe/probe.log#L76), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_descriptors | USB descriptors reject null output | PASS | [log L77](hid_descriptor_probe/probe.log#L77), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_generic_descriptor | USB generic descriptor rejects invalid buffers | PASS | [log L78](hid_descriptor_probe/probe.log#L78), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_hid_descriptor | USB HID descriptor rejects invalid buffers | PASS | [log L79](hid_descriptor_probe/probe.log#L79), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_report_descriptor | USB report descriptor rejects invalid buffers | PASS | [log L80](hid_descriptor_probe/probe.log#L80), [page-002](hid_descriptor_probe/pages/page-002.png) |
| ogc-usb::get_configuration | USB configuration rejects null output | PASS | [log L81](hid_descriptor_probe/probe.log#L81), [page-002](hid_descriptor_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Invalid argument rejection only; no USB device, successful transfer, IOS or physical Wii behavior is covered.
