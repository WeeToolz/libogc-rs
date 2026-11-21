# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 645. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::suspend_device | USB raw suspend rejects invalid handle | PASS | [log L305](runtime_probe/probe.log#L305), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::resume_device | USB raw resume rejects invalid handle | PASS | [log L306](runtime_probe/probe.log#L306), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::set_configuration | USB raw configuration rejects invalid handle | PASS | [log L307](runtime_probe/probe.log#L307), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::set_alternative_interface | USB raw alternate interface rejects invalid handle | PASS | [log L308](runtime_probe/probe.log#L308), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::clear_halt | USB raw clear halt rejects invalid handle | PASS | [log L309](runtime_probe/probe.log#L309), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::configuration | USB configuration query rejects invalid handle | PASS | [log L310](runtime_probe/probe.log#L310), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
