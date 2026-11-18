# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 468. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::snapshot_hardware_probe | Raw system hardware snapshot records getter calls | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_time | Raw system clock advances monotonically | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin hardware getter call flags and consecutive clock reads. Button transitions, clock rollover and physical Wii remain unverified.
