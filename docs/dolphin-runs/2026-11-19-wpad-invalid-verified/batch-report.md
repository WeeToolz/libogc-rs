# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 22. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::accel | WPAD invalid channel leaves acceleration output unchanged | PASS | [log L78](wpad_probe/probe.log#L78), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::data | WPAD invalid channel has no data pointer | PASS | [log L74](wpad_probe/probe.log#L74), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::expansion | WPAD invalid channel leaves expansion output unchanged | PASS | [log L83](wpad_probe/probe.log#L83), [page-003](wpad_probe/pages/page-003.png) |
| wpad-sys::g_force | WPAD invalid channel leaves g-force output unchanged | PASS | [log L81](wpad_probe/probe.log#L81), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::ir | WPAD invalid channel leaves infrared output unchanged | PASS | [log L79](wpad_probe/probe.log#L79), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::battery_level | WPAD invalid channel battery level is zero | PASS | [log L75](wpad_probe/probe.log#L75), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::buttons_down | WPAD invalid channel button edges are zero | PASS | [log L76](wpad_probe/probe.log#L76), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::buttons_held | WPAD invalid channel held buttons are zero | PASS | [log L77](wpad_probe/probe.log#L77), [page-002](wpad_probe/pages/page-002.png) |
| wpad-sys::buttons_up | WPAD invalid channel button edges are zero | PASS | [log L76](wpad_probe/probe.log#L76), [page-002](wpad_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Only invalid channel WPAD_MAX_WIIMOTES was tested. Valid channel output, connected controller behavior and physical Wii remain unverified.
