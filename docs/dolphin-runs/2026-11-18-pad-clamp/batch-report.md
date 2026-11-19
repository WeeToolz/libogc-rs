# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 482. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input::clamp | PAD clamp applies bounds and preserves error samples | PASS | [log L175](runtime_probe/probe.log#L175), [page-006](runtime_probe/pages/page-006.png) |
| ogc-input-sys::pad_clamp | PAD clamp applies bounds and preserves error samples | PASS | [log L175](runtime_probe/probe.log#L175), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin synthetic PAD samples on default clamp settings. Diagonal edges, alternate clamp settings and physical Wii remain unverified.
