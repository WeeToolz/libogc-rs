# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 275. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network::select | Select reports queued stream data | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::select | Select clears an empty stream | PASS | [log L501](runtime_probe/probe.log#L501), [page-020](runtime_probe/pages/page-020.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin TCP loopback read readiness with zero timeout. Write and exception sets, invalid handles, and physical Wii behavior require separate checks.
