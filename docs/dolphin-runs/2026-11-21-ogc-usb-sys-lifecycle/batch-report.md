# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 745. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::initialize | USB sys initialize starts both host services | PASS | [log L338](runtime_probe/probe.log#L338), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::initialize_vendor | USB sys vendor initialize starts the vendor host | PASS | [log L340](runtime_probe/probe.log#L340), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::deinitialize | USB sys deinitialize stops both host services | PASS | [log L339](runtime_probe/probe.log#L339), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::deinitialize_vendor | USB sys vendor deinitialize stops the vendor host | PASS | [log L341](runtime_probe/probe.log#L341), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
