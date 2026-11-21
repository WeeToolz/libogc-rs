# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 40. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::enable_wpad | WPAD initialized | PASS | [log L75](input_probe/probe.log#L75), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::shutdown_bluetooth | Bluetooth shutdown request accepted | PASS | [log L165](input_probe/probe.log#L165), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_shutdown_complete | Bluetooth shutdown completed | PASS | [log L167](input_probe/probe.log#L167), [page-004](input_probe/pages/page-004.png) |
| ogc-controller::bluetooth_diagnostics | Bluetooth transport has no terminal failure | PASS | [log L168](input_probe/probe.log#L168), [page-004](input_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin tests controller-manager Bluetooth lifecycle behavior in the emulator. It does not verify behavior with a physical Wii Remote or USB controller.
