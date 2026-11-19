# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 472. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wiilaunch::from_raw_code | Wii launch error codes preserve named and ES values | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::raw_code | Wii launch error codes preserve named and ES values | PASS | [log L212](runtime_probe/probe.log#L212), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin error code conversions. Launches, initialization, ES calls and physical Wii remain unverified.
