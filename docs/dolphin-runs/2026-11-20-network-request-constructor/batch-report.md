# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 693. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::new | Owned network request constructor initializes ABI fields | PASS | [log L905](runtime_probe/probe.log#L905), [page-037](runtime_probe/pages/page-037.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies the 32-bit request constructor fields and pointer preservation. It does not test request transport behavior.
