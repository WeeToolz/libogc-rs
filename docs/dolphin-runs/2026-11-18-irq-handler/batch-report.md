# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 475. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-irq::request | IRQ handler registry stores and removes handler | PASS | [log L246](runtime_probe/probe.log#L246), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq::get_handler | IRQ handler registry stores and removes handler | PASS | [log L246](runtime_probe/probe.log#L246), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq::free | IRQ handler registry stores and removes handler | PASS | [log L246](runtime_probe/probe.log#L246), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin registration and removal on an unused debug interrupt. Dispatch, interrupt masks, concurrent replacement and physical Wii remain unverified.
