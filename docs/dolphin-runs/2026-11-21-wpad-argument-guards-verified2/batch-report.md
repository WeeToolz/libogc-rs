# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 70. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::set_event_bufs | WPAD valid channel rejects a null event buffer | PASS | [log L94](wpad_probe/probe.log#L94), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_event_bufs | WPAD valid channel rejects a zero event count without writing | PASS | [log L95](wpad_probe/probe.log#L95), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_event_bufs | WPAD empty channel installs and initializes an event buffer | PASS | [log L96](wpad_probe/probe.log#L96), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::send_stream_data | WPAD valid channel rejects a null stream buffer | PASS | [log L97](wpad_probe/probe.log#L97), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::send_stream_data | WPAD valid channel rejects a zero stream length | PASS | [log L98](wpad_probe/probe.log#L98), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
