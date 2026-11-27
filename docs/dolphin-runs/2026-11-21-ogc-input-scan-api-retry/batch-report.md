# Dolphin evidence report

Probe `pad_state_probe` reported `pass`.
Passed checks 67. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input::scan_pads | PAD low level scans observe scripted controller | Missing |  |
| ogc-input::scan_pads_with_status | PAD low level scans observe scripted controller | Missing |  |
| ogc-input::control_motor | PAD invalid motor request produces trace without state change | Missing |  |
| ogc-input::set_spec | PAD set spec preserves the selected specification | Missing |  |
| ogc-input::set_sampling_callback | PAD sampling callback replaces delivers and removes | Missing |  |
| ogc-input::set_debug_hook | PAD repeated initialization reports debug stages | Missing |  |

## Evidence gaps

- ogc-input::scan_pads — PAD low level scans observe scripted controller (Missing)
- ogc-input::scan_pads_with_status — PAD low level scans observe scripted controller (Missing)
- ogc-input::control_motor — PAD invalid motor request produces trace without state change (Missing)
- ogc-input::set_spec — PAD set spec preserves the selected specification (Missing)
- ogc-input::set_sampling_callback — PAD sampling callback replaces delivers and removes (Missing)
- ogc-input::set_debug_hook — PAD repeated initialization reports debug stages (Missing)
