# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 600. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::open_device | USB open rejects a null output descriptor | PASS | [log L253](runtime_probe/probe.log#L253), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::close_device | USB close rejects a null descriptor pointer | PASS | [log L254](runtime_probe/probe.log#L254), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::close_device_async | USB async close rejects a null descriptor pointer | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::free_descriptors | USB descriptor free accepts a null descriptor | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_ascii_string | USB ASCII string query rejects a null buffer | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_device_description | USB device description rejects a null output | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_descriptors | USB descriptor query rejects a null output | PASS | [log L257](runtime_probe/probe.log#L257), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_device_list | USB device list rejects a null array without changing count | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_generic_descriptor | USB generic descriptor rejects a null buffer | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_hid_descriptor | USB HID descriptor rejects a null output | PASS | [log L259](runtime_probe/probe.log#L259), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb-sys::get_report_descriptor | USB report descriptor rejects a null buffer | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
