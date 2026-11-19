# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 52. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::read_content | ES content read rejects invalid inputs and preserves output | PASS | [log L94](general_probe/probe.log#L94), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::seek_content | ES content seek rejects negative descriptor | PASS | [log L95](general_probe/probe.log#L95), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::close_content | ES content close rejects negative descriptor | PASS | [log L96](general_probe/probe.log#L96), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Checks input rejection with ES initialized. Successful content handles, reads, seeks, and close operations remain open.
