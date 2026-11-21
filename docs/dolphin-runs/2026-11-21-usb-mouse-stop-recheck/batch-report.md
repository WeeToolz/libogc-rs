# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 745. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::init | USB mouse initializes its worker | PASS | [log L344](runtime_probe/probe.log#L344), [page-014](runtime_probe/pages/page-014.png) |
| ogc-usb::deinit | USB mouse deinitialization joins its worker and releases state | PASS | [log L345](runtime_probe/probe.log#L345), [page-014](runtime_probe/pages/page-014.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies successful worker initialization and shutdown without a physical USB mouse. Injected host tests cover wake and join errors through the production stop helper. An attached device and a pending physical read remain open.
