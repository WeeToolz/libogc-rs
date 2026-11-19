# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 537. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si::enable_polling_interrupt | SI wrapper polling interrupt changes and reports previous state | PASS | [log L287](runtime_probe/probe.log#L287), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::register_polling_handler | SI wrapper polling handler registration and removal | PASS | [log L289](runtime_probe/probe.log#L289), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::unregister_polling_handler | SI wrapper polling handler registration and removal | PASS | [log L289](runtime_probe/probe.log#L289), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::transfer | SI wrapper transfer reports no controller and clears busy | PASS | [log L291](runtime_probe/probe.log#L291), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::busy | SI wrapper transfer reports no controller and clears busy | PASS | [log L291](runtime_probe/probe.log#L291), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si::sync | SI wrapper transfer reports no controller and clears busy | PASS | [log L291](runtime_probe/probe.log#L291), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::enable_polling_interrupt | SI raw polling interrupt changes and reports previous state | PASS | [log L288](runtime_probe/probe.log#L288), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::register_polling_handler | SI raw polling handler registration and removal | PASS | [log L290](runtime_probe/probe.log#L290), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::unregister_polling_handler | SI raw polling handler registration and removal | PASS | [log L290](runtime_probe/probe.log#L290), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::transfer | SI raw transfer reports no controller and clears busy | PASS | [log L292](runtime_probe/probe.log#L292), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::busy | SI raw transfer reports no controller and clears busy | PASS | [log L292](runtime_probe/probe.log#L292), [page-011](runtime_probe/pages/page-011.png) |
| ogc-si-sys::sync | SI raw transfer reports no controller and clears busy | PASS | [log L292](runtime_probe/probe.log#L292), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin no-controller channel and masked polling interrupt registry checks only. Handler delivery, controller responses, concurrent transfers, and physical Wii remain unverified.
