# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 34. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_init | PAD repeated initialization reports debug stages | PASS | [log L142](input_probe/probe.log#L142), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::set_debug_hook | PAD repeated initialization reports debug stages | PASS | [log L142](input_probe/probe.log#L142), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::set_trace_hook | PAD invalid motor request produces trace without state change | PASS | [log L148](input_probe/probe.log#L148), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_control_motor | PAD invalid motor request produces trace without state change | PASS | [log L148](input_probe/probe.log#L148), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_state | PAD sync matches trace and idle state | PASS | [log L150](input_probe/probe.log#L150), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_counters | PAD trace counters and drain preserve event sequence | PASS | [log L149](input_probe/probe.log#L149), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_debug_drain | PAD trace counters and drain preserve event sequence | PASS | [log L149](input_probe/probe.log#L149), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_sync | PAD sync matches trace and idle state | PASS | [log L150](input_probe/probe.log#L150), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_scan_pads | PAD low level scans observe scripted controller | PASS | [log L151](input_probe/probe.log#L151), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_scan_pads_with_status | PAD low level scans observe scripted controller | PASS | [log L151](input_probe/probe.log#L151), [page-003](input_probe/pages/page-003.png) |
| ogc-input-sys::pad_read | PAD read reports connected channel and rejects null | PASS | [log L152](input_probe/probe.log#L152), [page-003](input_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One scripted Dolphin controller after initialization, idle synchronization, and invalid motor channel. Real controller motors, reset races, and physical Wii behavior remain open.
