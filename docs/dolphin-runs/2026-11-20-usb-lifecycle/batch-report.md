# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 708. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::initialize | USB service initializes with the raw API | PASS | [log L315](runtime_probe/probe.log#L315), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::try_initialize | USB fallible initialization reports success | PASS | [log L317](runtime_probe/probe.log#L317), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::deinitialize | USB service deinitializes after raw initialization | PASS | [log L316](runtime_probe/probe.log#L316), [page-012](runtime_probe/pages/page-012.png) |
| ogc-usb::try_deinitialize | USB fallible deinitialization reports success | PASS | [log L318](runtime_probe/probe.log#L318), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin tests two start and cleanup cycles with its IOS USB implementation. It does not prove behavior with physical USB hosts or attached devices.
