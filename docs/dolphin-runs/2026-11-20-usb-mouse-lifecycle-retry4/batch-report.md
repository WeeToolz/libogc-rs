# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 710. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::init | USB mouse initializes its worker | PASS | [log L321](runtime_probe/probe.log#L321), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb::deinit | USB mouse deinitialization joins its worker and releases state | PASS | [log L322](runtime_probe/probe.log#L322), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
