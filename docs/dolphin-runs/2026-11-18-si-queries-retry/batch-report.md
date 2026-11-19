# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 407. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si::decode_type | SI decodes controller and error types | PASS | [log L185](runtime_probe/probe.log#L185), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::is_chan_busy | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_status | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_response_raw | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_response | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_command | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_type | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::get_type_async | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |
| ogc-si::probe | SI rejects channels outside zero through three | PASS | [log L186](runtime_probe/probe.log#L186), [page-007](runtime_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin controller type decoding and invalid-channel behavior. Connected controller data, asynchronous completion and physical Wii remain unverified.
