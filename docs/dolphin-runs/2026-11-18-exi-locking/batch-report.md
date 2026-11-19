# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 486. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::lock | EXI lock rejects contention and permits reacquisition | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_lock | EXI lock rejects contention and permits reacquisition | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::unlock | EXI lock rejects contention and permits reacquisition | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_unlock | EXI lock rejects contention and permits reacquisition | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded EXI channel 1 lock and error paths. Concurrent callbacks, transfers, hardware devices and physical Wii remain unverified.
