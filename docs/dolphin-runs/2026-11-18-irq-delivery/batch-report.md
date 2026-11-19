# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 477. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-irq::irq_disable | IRQ nested disable and restore preserve MSR delivery state | PASS | [log L248](runtime_probe/probe.log#L248), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq::irq_restore | IRQ nested disable and restore preserve MSR delivery state | PASS | [log L248](runtime_probe/probe.log#L248), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq-sys::k_irq_disable_all | IRQ nested disable and restore preserve MSR delivery state | PASS | [log L248](runtime_probe/probe.log#L248), [page-009](runtime_probe/pages/page-009.png) |
| ogc-irq-sys::k_irq_restore_all | IRQ nested disable and restore preserve MSR delivery state | PASS | [log L248](runtime_probe/probe.log#L248), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin nested interrupt delivery disable and restoration on the current thread. Interrupt arrival, concurrency and physical Wii remain unverified.
