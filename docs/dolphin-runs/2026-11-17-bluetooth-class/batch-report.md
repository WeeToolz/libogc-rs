# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 278. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | Bluetooth address preserves HCI byte order | PASS | [log L119](runtime_probe/probe.log#L119), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::as_raw | Bluetooth address preserves HCI byte order | PASS | [log L119](runtime_probe/probe.log#L119), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::new | Bluetooth class extracts major and minor fields | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::major | Bluetooth class extracts major and minor fields | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::minor | Bluetooth class extracts major and minor fields | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::likely_game_controller | Bluetooth class recognizes peripheral devices | PASS | [log L121](runtime_probe/probe.log#L121), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Protocol value checks in Dolphin. Bluetooth transport and physical hardware are outside this batch.
