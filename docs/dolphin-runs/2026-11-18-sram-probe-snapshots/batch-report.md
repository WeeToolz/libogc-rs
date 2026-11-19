# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 461. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::snapshot_counter_bias_probe | System SRAM getter snapshots record calls | PASS | [log L295](runtime_probe/probe.log#L295), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system-sys::snapshot_display_offset_probe | System SRAM getter snapshots record calls | PASS | [log L295](runtime_probe/probe.log#L295), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system-sys::snapshot_eu_rgb60_probe | System SRAM getter snapshots record calls | PASS | [log L295](runtime_probe/probe.log#L295), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system-sys::snapshot_language_probe | System SRAM getter snapshots record calls | PASS | [log L295](runtime_probe/probe.log#L295), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system-sys::snapshot_progressive_scan_probe | System SRAM getter snapshots record calls | PASS | [log L295](runtime_probe/probe.log#L295), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system-sys::sram_checksum_words | System SRAM checksum wraps both sums | PASS | [log L296](runtime_probe/probe.log#L296), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin SRAM getter call flags and synthetic checksum words. Setter flags, persistence and physical Wii remain unverified.
