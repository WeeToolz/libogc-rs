# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 568. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::enable_usb_hid | Controller USB ownership rejects contention and permits reuse | PASS | [log L177](runtime_probe/probe.log#L177), [page-006](runtime_probe/pages/page-006.png) |
| ogc-controller::shutdown_usb_hid | Controller USB ownership rejects contention and permits reuse | PASS | [log L177](runtime_probe/probe.log#L177), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Ownership lifecycle without initialized USB devices or outstanding IOS requests; no USB transfer, descriptor cleanup, or physical hardware is covered.
