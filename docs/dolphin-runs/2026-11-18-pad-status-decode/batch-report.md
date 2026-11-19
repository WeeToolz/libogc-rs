# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 483. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_debug_make_status | PAD diagnostic decoder handles neutral sample and invalid channels | PASS | [log L176](runtime_probe/probe.log#L176), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin default PAD mode and neutral synthetic sample. Calibrated origins, other controller modes and physical Wii remain unverified.
