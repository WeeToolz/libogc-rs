# Dolphin evidence report

Probe `callback_probe` reported `pass`.
Passed checks 11. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::set_reset_callback | reset callback replacement preserves previous pointer | PASS | [log L75](callback_probe/probe.log#L75), [page-002](callback_probe/pages/page-002.png) |
| ogc-system::snapshot_callbacks | reset callback replacement preserves previous pointer | PASS | [log L75](callback_probe/probe.log#L75), [page-002](callback_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Registration and snapshot state in Dolphin. Physical reset button dispatch and power callback behavior need separate evidence.
