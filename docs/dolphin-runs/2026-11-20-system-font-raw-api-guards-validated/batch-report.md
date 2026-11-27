# Dolphin evidence report

Probe `font_probe` reported `probe_fail`.
Passed checks 13. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_init_font | Raw system font init rejects null storage | PASS | [log L74](font_probe/probe.log#L74), [page-002](font_probe/pages/page-002.png) |
| ogc-system-sys::sys_get_font_texture | Raw system font texture clears outputs without font data | PASS | [log L75](font_probe/probe.log#L75), [page-002](font_probe/pages/page-002.png) |
| ogc-system-sys::sys_get_font_texel | Raw system font texel rejects null image | PASS | [log L76](font_probe/probe.log#L76), [page-002](font_probe/pages/page-002.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — font init succeeded twice
