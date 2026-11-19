# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 489. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::register_exi_callback | EXI callback registry installs and restores callback | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::register_exi_callback | EXI callback registry installs and restores callback | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::interrupts_ready | EXI callback registry installs and restores callback | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin EXI channel 1 callback replacement after startup. Interrupt dispatch, concurrent changes and physical Wii remain unverified.
