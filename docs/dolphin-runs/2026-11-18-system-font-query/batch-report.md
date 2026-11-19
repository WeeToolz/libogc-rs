# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 466. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_get_font_encoding | System font encoding and snapshots report query | PASS | [log L220](runtime_probe/probe.log#L220), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_font | System font encoding and snapshots report query | PASS | [log L220](runtime_probe/probe.log#L220), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system::snapshot_font | System font encoding and snapshots report query | PASS | [log L220](runtime_probe/probe.log#L220), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin current video mode encoding and font query snapshot. Font initialization, glyph textures, other video modes and physical Wii remain unverified.
