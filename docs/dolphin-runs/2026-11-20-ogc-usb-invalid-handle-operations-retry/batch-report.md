# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 612. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::try_suspend_device | USB suspend rejects invalid handle | PASS | [log L271](runtime_probe/probe.log#L271), [page-010](runtime_probe/pages/page-010.png) |
| ogc-usb::try_resume_device | USB resume rejects invalid handle | PASS | [log L273](runtime_probe/probe.log#L273), [page-011](runtime_probe/pages/page-011.png) |
| ogc-usb::try_set_configuration | USB set configuration rejects invalid handle | PASS | [log L274](runtime_probe/probe.log#L274), [page-011](runtime_probe/pages/page-011.png) |
| ogc-usb::try_set_alternative_interface | USB set alternate interface rejects invalid handle | PASS | [log L275](runtime_probe/probe.log#L275), [page-011](runtime_probe/pages/page-011.png) |
| ogc-usb::try_clear_halt | USB clear halt rejects invalid handle | PASS | [log L276](runtime_probe/probe.log#L276), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
