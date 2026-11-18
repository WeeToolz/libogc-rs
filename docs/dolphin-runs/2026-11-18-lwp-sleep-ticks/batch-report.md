# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 474. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::sleep_ticks | LWP driver sleep waits requested ticks | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::sleep_ticks | LWP raw sleep waits requested ticks | PASS | [log L137](runtime_probe/probe.log#L137), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one-millisecond nonzero waits measured by the PowerPC timebase. Zero delay, long deadlines, competing threads and physical Wii remain unverified.
