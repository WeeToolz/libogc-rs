# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 692. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::set_option | Owned network set option rejects unsupported options locally | PASS | [log L906](runtime_probe/probe.log#L906), [page-037](runtime_probe/pages/page-037.png) |
| ogc-network-sys::bool_option | Owned network boolean option rejects unsupported options locally | PASS | [log L907](runtime_probe/probe.log#L907), [page-037](runtime_probe/pages/page-037.png) |
| ogc-network-sys::local | Owned network local error preserves its errno domain | PASS | [log L908](runtime_probe/probe.log#L908), [page-037](runtime_probe/pages/page-037.png) |
| ogc-network-sys::retryable | Owned network retry classification uses IOS socket errors | PASS | [log L910](runtime_probe/probe.log#L910), [page-038](runtime_probe/pages/page-038.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies the owned-socket local option guards and pure error mapping. It does not prove all IOS socket option behavior or network behavior on hardware.
