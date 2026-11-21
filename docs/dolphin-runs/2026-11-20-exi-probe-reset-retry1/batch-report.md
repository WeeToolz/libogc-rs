# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 688. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::probe_reset | EXI probe reset leaves stable channel probe results | PASS | [log L428](runtime_probe/probe.log#L428), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin checks that the reset probe leaves valid, stable channel results. It does not prove detection timing or IDs for physical EXI devices.
