# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 421. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si-sys::decode_type | SI raw decodes known device and errors | PASS | [log L199](runtime_probe/probe.log#L199), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::is_chan_busy | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_status | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_response_raw | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_response | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::debug_last_response | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_command | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_type | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::get_type_async | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si-sys::probe | SI raw rejects invalid channels without output writes | PASS | [log L200](runtime_probe/probe.log#L200), [page-007](runtime_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin device type decoding and invalid-channel behavior. Valid-channel device communication, async completion and physical Wii remain unverified.
