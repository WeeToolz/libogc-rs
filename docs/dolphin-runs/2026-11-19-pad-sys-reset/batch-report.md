# Dolphin evidence report

Probe `pad_state_probe` reported `pass`.
Passed checks 67. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_reset | sys-reset-channel-0 | PASS | [log L114](pad_state_probe/probe.log#L114), [page-005](pad_state_probe/pages/page-005.png) |
| ogc-input-sys::pad_recalibrate | sys-recalibrate-channel-1 | PASS | [log L132](pad_state_probe/probe.log#L132), [page-006](pad_state_probe/pages/page-006.png) |
| ogc-input-sys::pad_set_spec | PAD post-init specification remains unchanged | PASS | [log L137](pad_state_probe/probe.log#L137), [page-007](pad_state_probe/pages/page-007.png) |
| ogc-input-sys::_pad_disable_recalibration | PAD recalibration flag changes and restores low memory | PASS | [log L138](pad_state_probe/probe.log#L138), [page-007](pad_state_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Deterministic SI error fixtures for one reset and one recalibration, post-init specification, and restored low-memory flag in Dolphin. Real controller calibration, transfer races, and physical Wii behavior remain open.
