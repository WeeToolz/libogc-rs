# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 762. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-irq::dispatch_pending | IRQ dispatch reports no handled pending interrupt | PASS | [log L472](runtime_probe/probe.log#L472), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
