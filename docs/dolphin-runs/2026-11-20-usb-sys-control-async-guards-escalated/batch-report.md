# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 672. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb-sys::read_ctrl_msg_async | USB sys async control read rejects invalid handle | PASS | [log L336](runtime_probe/probe.log#L336), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::v5_failure_status | USB sys V5 failure status reports a valid stage and error | PASS | [log L338](runtime_probe/probe.log#L338), [page-013](runtime_probe/pages/page-013.png) |
| ogc-usb-sys::write_ctrl_msg_async | USB sys async control write rejects invalid handle | PASS | [log L337](runtime_probe/probe.log#L337), [page-013](runtime_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin validates the invalid-handle paths and the status result shape. It does not test an attached USB device or successful transfers.
