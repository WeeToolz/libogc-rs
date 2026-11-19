# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 35. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::control_speaker | WPAD invalid channel rejects speaker control | PASS | [log L88](wpad_probe/probe.log#L88), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::disconnect | WPAD invalid channel rejects disconnect | PASS | [log L89](wpad_probe/probe.log#L89), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::dropped_events | WPAD invalid channel rejects dropped-event query | PASS | [log L90](wpad_probe/probe.log#L90), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::flush | WPAD invalid channel rejects event flush | PASS | [log L91](wpad_probe/probe.log#L91), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::is_speaker_enabled | WPAD invalid channel rejects speaker query | PASS | [log L92](wpad_probe/probe.log#L92), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::read_event | WPAD invalid channel rejects event read and preserves output | PASS | [log L93](wpad_probe/probe.log#L93), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::rumble | WPAD invalid channel rejects rumble | PASS | [log L94](wpad_probe/probe.log#L94), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::send_stream_data | WPAD invalid channel rejects speaker stream | PASS | [log L95](wpad_probe/probe.log#L95), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_data_format | WPAD invalid channel rejects data format | PASS | [log L96](wpad_probe/probe.log#L96), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_event_bufs | WPAD invalid channel rejects event buffers | PASS | [log L97](wpad_probe/probe.log#L97), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_idle_thresholds | WPAD invalid channel rejects idle thresholds | PASS | [log L98](wpad_probe/probe.log#L98), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_motion_plus | WPAD invalid channel rejects MotionPlus configuration | PASS | [log L99](wpad_probe/probe.log#L99), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::set_v_res | WPAD invalid channel rejects IR resolution | PASS | [log L100](wpad_probe/probe.log#L100), [page-003](wpad_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Only invalid channel WPAD_MAX_WIIMOTES was tested. Valid channel operations, connected controller behavior and physical Wii remain unverified.
