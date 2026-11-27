# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 687. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::probe_reset | EXI probe reset preserves the detected channel states | FAIL | [log L428](runtime_probe/probe.log#L428), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-exi::probe_reset — EXI probe reset preserves the detected channel states (FAIL)

## Coverage limits

- Dolphin checks that the reset probe keeps channel discovery stable. It does not prove detection timing or IDs for physical EXI devices.
