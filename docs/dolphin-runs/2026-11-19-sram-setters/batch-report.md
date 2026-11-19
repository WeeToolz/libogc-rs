# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 519. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::set_counter_bias | SRAM counter bias setter updates field | PASS | [log L352](runtime_probe/probe.log#L352), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_counter_bias | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_display_offset_h | SRAM display offset setter updates field | PASS | [log L353](runtime_probe/probe.log#L353), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_display_offset_h | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_language | SRAM language setter updates field | PASS | [log L354](runtime_probe/probe.log#L354), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_language | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_eu_rgb60 | SRAM RGB60 setter updates bit | PASS | [log L355](runtime_probe/probe.log#L355), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_eu_rgb60 | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_progressive_scan | SRAM progressive setter updates bit | PASS | [log L356](runtime_probe/probe.log#L356), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_progressive_scan | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_sound_mode | SRAM sound setter updates bit | PASS | [log L357](runtime_probe/probe.log#L357), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_sound_mode | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_video_mode | SRAM video setter updates bits | PASS | [log L358](runtime_probe/probe.log#L358), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_video_mode | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_wireless_id | SRAM wireless setter updates selected channel | PASS | [log L359](runtime_probe/probe.log#L359), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_wireless_id | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_gbs_mode | SRAM GBS setter updates field | PASS | [log L360](runtime_probe/probe.log#L360), [page-014](runtime_probe/pages/page-014.png) |
| ogc-system::set_gbs_mode | SRAM setter fixture restores both images | PASS | [log L361](runtime_probe/probe.log#L361), [page-014](runtime_probe/pages/page-014.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin private profile only. Checks cover SRAM image changes and restoration, not persistence across restart or physical Wii behavior.
