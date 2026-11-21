# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 704. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::get_device_list | USB zero-capacity device list returns an empty result | PASS | [log L327](runtime_probe/probe.log#L327), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::device_removal_notify_async | USB removal notification rejects an invalid descriptor | PASS | [log L328](runtime_probe/probe.log#L328), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::get_ascii_string | USB ASCII string rejects an invalid descriptor without changing output | PASS | [log L329](runtime_probe/probe.log#L329), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::get_event | USB mouse event queue starts empty | PASS | [log L330](runtime_probe/probe.log#L330), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::flush_events | USB mouse event flush preserves an empty queue | PASS | [log L331](runtime_probe/probe.log#L331), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::is_connected | USB mouse reports disconnected before initialization | PASS | [log L332](runtime_probe/probe.log#L332), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies bounded empty-state and invalid-descriptor paths before USB mouse initialization. It does not test enumeration or callbacks with an attached USB device.
