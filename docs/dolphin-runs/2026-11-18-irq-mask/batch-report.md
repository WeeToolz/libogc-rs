# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 478. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-irq::disable | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq::enable | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq::mask | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq::unmask | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq-sys::k_irq_disable | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq-sys::k_irq_enable | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-irq-sys::mask | IRQ mask operations update and restore debug bit | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin debug interrupt software mask with external delivery disabled during the check. Hardware delivery, concurrent changes and physical Wii remain unverified.
