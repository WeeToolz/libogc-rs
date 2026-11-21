# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 40. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input::scan_pads | PAD low level scans observe scripted controller | PASS | [log L151](input_probe/probe.log#L151), [page-003](input_probe/pages/page-003.png), [log L158](input_probe/probe.log#L158), [page-003](input_probe/pages/page-003.png) |
| ogc-input::scan_pads_with_status | PAD low level scans observe scripted controller | PASS | [log L151](input_probe/probe.log#L151), [page-003](input_probe/pages/page-003.png), [log L158](input_probe/probe.log#L158), [page-003](input_probe/pages/page-003.png) |
| ogc-input::control_motor | PAD invalid motor request produces trace without state change | PASS | [log L148](input_probe/probe.log#L148), [page-003](input_probe/pages/page-003.png), [log L157](input_probe/probe.log#L157), [page-003](input_probe/pages/page-003.png) |
| ogc-input::set_spec | PAD set spec preserves the selected specification | PASS | [log L156](input_probe/probe.log#L156), [page-003](input_probe/pages/page-003.png) |
| ogc-input::set_sampling_callback | PAD sampling callback replaces delivers and removes | PASS | [log L159](input_probe/probe.log#L159), [page-003](input_probe/pages/page-003.png), [log L162](input_probe/probe.log#L162), [page-003](input_probe/pages/page-003.png) |
| ogc-input::set_debug_hook | PAD repeated initialization reports debug stages | PASS | [log L142](input_probe/probe.log#L142), [page-003](input_probe/pages/page-003.png), [log L155](input_probe/probe.log#L155), [page-003](input_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
