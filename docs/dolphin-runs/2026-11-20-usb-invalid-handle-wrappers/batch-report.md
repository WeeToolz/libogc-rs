# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 698. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::close_device_async | USB async handle close rejects an invalid descriptor | PASS | [log L329](runtime_probe/probe.log#L329), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::close_device_handle | USB handle close rejects an invalid descriptor | PASS | [log L328](runtime_probe/probe.log#L328), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::try_close_device_async | USB fallible async close maps an invalid descriptor | PASS | [log L330](runtime_probe/probe.log#L330), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::open_device_handle | USB open handle rejects an invalid device id | PASS | [log L327](runtime_probe/probe.log#L327), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::get_report_descriptor_size | USB report descriptor size rejects an invalid handle | PASS | [log L331](runtime_probe/probe.log#L331), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies invalid descriptor and device-id rejection through the owned API wrappers. It does not test successful operations with an attached USB device.
