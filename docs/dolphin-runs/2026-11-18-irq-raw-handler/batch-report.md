# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 476. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-irq-sys::k_irq_set | Raw IRQ registry stores and clears handler | PASS | [log L247](runtime_probe/probe.log#L247), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq-sys::k_irq_get | Raw IRQ registry stores and clears handler | PASS | [log L247](runtime_probe/probe.log#L247), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq::clear | Raw IRQ registry stores and clears handler | PASS | [log L247](runtime_probe/probe.log#L247), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin registration and removal on an unused debug interrupt. Dispatch, interrupt masks, concurrent replacement and physical Wii remain unverified.
