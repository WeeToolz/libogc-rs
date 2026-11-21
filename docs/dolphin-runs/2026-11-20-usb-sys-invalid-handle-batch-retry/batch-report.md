# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 669. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::clear_halt | USB sys clear halt rejects invalid handle | PASS | [log L334](runtime_probe/probe.log#L334), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::get_configuration | USB sys configuration query preserves output on invalid handle | PASS | [log L331](runtime_probe/probe.log#L331), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::get_report_descriptor_size | USB sys report descriptor size rejects invalid handle | PASS | [log L335](runtime_probe/probe.log#L335), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::resume_device | USB sys resume rejects invalid handle | PASS | [log L330](runtime_probe/probe.log#L330), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::set_alternative_interface | USB sys alternate interface rejects invalid handle | PASS | [log L333](runtime_probe/probe.log#L333), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::set_configuration | USB sys configuration set rejects invalid handle | PASS | [log L332](runtime_probe/probe.log#L332), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::suspend_device | USB sys suspend rejects invalid handle | PASS | [log L329](runtime_probe/probe.log#L329), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
