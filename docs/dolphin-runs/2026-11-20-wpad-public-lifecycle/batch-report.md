# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 43. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::get_status | WPAD starts disabled | PASS | [log L84](wpad_probe/probe.log#L84), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::orientation | WPAD invalid channel leaves orientation output unchanged | PASS | [log L80](wpad_probe/probe.log#L80), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::pad_status | WPAD invalid channel pad status leaves lifecycle state unchanged | PASS | [log L96](wpad_probe/probe.log#L96), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::probe | empty channel probe preserves output on no controller | PASS | [log L110](wpad_probe/probe.log#L110), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::read_pending | WPAD invalid channel rejects pending-event read | PASS | [log L95](wpad_probe/probe.log#L95), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::scan_pads | WPAD Bluetooth poll hook observes scan boundary | PASS | [log L87](wpad_probe/probe.log#L87), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::shutdown | WPAD shutdown succeeds | PASS | [log L114](wpad_probe/probe.log#L114), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::shutdown | WPAD reports disabled after shutdown | PASS | [log L117](wpad_probe/probe.log#L117), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The probe checks invalid channels and the no-controller path in Dolphin. It does not test valid connected Wii Remote traffic, pairing, or physical Wii behavior.
