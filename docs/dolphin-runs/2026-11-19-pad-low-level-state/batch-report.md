# Dolphin evidence report

Probe `input_probe` reported `probe_fail`.
Passed checks 30. Failed checks 4.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_init | PAD repeated initialization reports debug stages | PASS | [log L142](input_probe/probe.log#L142), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::set_debug_hook | PAD repeated initialization reports debug stages | PASS | [log L142](input_probe/probe.log#L142), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::set_trace_hook | PAD invalid motor request produces trace without state change | PASS | [log L143](input_probe/probe.log#L143), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_control_motor | PAD invalid motor request produces trace without state change | PASS | [log L143](input_probe/probe.log#L143), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_state | PAD sync matches trace and idle state | FAIL | [log L145](input_probe/probe.log#L145), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_counters | PAD trace counters and drain preserve event sequence | PASS | [log L144](input_probe/probe.log#L144), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_drain | PAD trace counters and drain preserve event sequence | PASS | [log L144](input_probe/probe.log#L144), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_sync | PAD sync matches trace and idle state | FAIL | [log L145](input_probe/probe.log#L145), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_scan_pads | PAD low level scans observe scripted controller | FAIL | [log L146](input_probe/probe.log#L146), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_scan_pads_with_status | PAD low level scans observe scripted controller | FAIL | [log L146](input_probe/probe.log#L146), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_read | PAD read reports connected channel and rejects null | FAIL | [log L147](input_probe/probe.log#L147), [page-003](input_probe/pages/page-003.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-input-sys::pad_debug_state — PAD sync matches trace and idle state (FAIL)
- ogc-input-sys::pad_sync — PAD sync matches trace and idle state (FAIL)
- ogc-input-sys::pad_scan_pads — PAD low level scans observe scripted controller (FAIL)
- ogc-input-sys::pad_scan_pads_with_status — PAD low level scans observe scripted controller (FAIL)
- ogc-input-sys::pad_read — PAD read reports connected channel and rejects null (FAIL)
- Other failed check — input collection completed without source, capture, or cleanup errors

## Coverage limits

- One scripted Dolphin controller after initialization, idle synchronization, and invalid motor channel. Real controller motors, reset races, and physical Wii behavior remain open.
