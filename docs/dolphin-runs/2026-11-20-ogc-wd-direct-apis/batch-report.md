# Dolphin evidence report

Probe `wd_probe` reported `pass`.
Passed checks 17. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wd::get_info | WD raw info query returns a valid record | PASS | [log L85](wd_probe/probe.log#L85), [page-003](wd_probe/pages/page-003.png) |
| ogc-wd::get_info_result | WD info result wrapper returns a valid record | PASS | [log L86](wd_probe/probe.log#L86), [page-003](wd_probe/pages/page-003.png) |
| ogc-wd::get_radio_level | WD radio level buckets match RSSI boundaries | PASS | [log L87](wd_probe/probe.log#L87), [page-003](wd_probe/pages/page-003.png) |
| ogc-wd::ncd_lock_wireless_driver | WD driver lock succeeds | PASS | [log L74](wd_probe/probe.log#L74), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::ncd_unlock_wireless_driver | WD driver unlock succeeds | PASS | [log L75](wd_probe/probe.log#L75), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::scan | WD raw scan succeeds with a valid result count | PASS | [log L77](wd_probe/probe.log#L77), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::scan_once | WD raw scan once succeeds | PASS | [log L80](wd_probe/probe.log#L80), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::scan_once_detailed | WD detailed scan once succeeds | PASS | [log L83](wd_probe/probe.log#L83), [page-003](wd_probe/pages/page-003.png) |
| ogc-wd::scan_once_detailed_result | WD detailed scan result wrapper succeeds | PASS | [log L84](wd_probe/probe.log#L84), [page-003](wd_probe/pages/page-003.png) |
| ogc-wd::scan_once_result | WD scan once result wrapper succeeds | PASS | [log L81](wd_probe/probe.log#L81), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::scan_result | WD scan result wrapper succeeds | PASS | [log L78](wd_probe/probe.log#L78), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::try_wd_deinit | WD fallible deinit succeeds | PASS | [log L79](wd_probe/probe.log#L79), [page-002](wd_probe/pages/page-002.png) |
| ogc-wd::try_wd_init | WD fallible init succeeds | PASS | [log L76](wd_probe/probe.log#L76), [page-002](wd_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
