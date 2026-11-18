# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 383. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::empty | Controller empty slot keeps source | PASS | [log L109](runtime_probe/probe.log#L109), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::new | Controller manager starts with empty slots | PASS | [log L110](runtime_probe/probe.log#L110), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::bluetooth_slots | Controller manager starts with empty slots | PASS | [log L110](runtime_probe/probe.log#L110), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::bluetooth_slots | Controller left slot needs a right partner | PASS | [log L112](runtime_probe/probe.log#L112), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::bluetooth_slots | Controller right slot retains address and handle | PASS | [log L113](runtime_probe/probe.log#L113), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::usb_slots | Controller manager starts with empty slots | PASS | [log L110](runtime_probe/probe.log#L110), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::joycon_pair | Controller pairs left and right by role | PASS | [log L114](runtime_probe/probe.log#L114), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::joycon_pair | Controller split keeps both devices | PASS | [log L115](runtime_probe/probe.log#L115), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::bind_usb_hid_button | Controller USB profile checks identity and usage | PASS | [log L111](runtime_probe/probe.log#L111), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_connected | Controller left slot needs a right partner | PASS | [log L112](runtime_probe/probe.log#L112), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_connected_with_handle | Controller right slot retains address and handle | PASS | [log L113](runtime_probe/probe.log#L113), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::pair_joycons | Controller left slot needs a right partner | PASS | [log L112](runtime_probe/probe.log#L112), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::pair_joycons | Controller pairs left and right by role | PASS | [log L114](runtime_probe/probe.log#L114), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::clear_joycon_pair | Controller split keeps both devices | PASS | [log L115](runtime_probe/probe.log#L115), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::clear_bluetooth_slot | Controller disconnect clears pair and one device | PASS | [log L116](runtime_probe/probe.log#L116), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_report | Controller guide report sets HOME | PASS | [log L117](runtime_probe/probe.log#L117), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_report_checked | Controller guide release clears HOME | PASS | [log L118](runtime_probe/probe.log#L118), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_report_checked | Controller short guide report invalidates sample | PASS | [log L119](runtime_probe/probe.log#L119), [page-004](runtime_probe/pages/page-004.png) |
| ogc-controller::set_bluetooth_calibration_checked | Controller rejects missing or incompatible calibration | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Synthetic controller slots and reports in Dolphin. USB profile validation only. Radio pairing, physical devices, calibrated samples and USB reconnects remain unverified.
