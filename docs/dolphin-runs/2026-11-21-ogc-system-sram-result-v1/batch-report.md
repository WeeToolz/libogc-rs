# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 760. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::last_config_write_result | System last config write result follows SRAM setters | PASS | [log L591](runtime_probe/probe.log#L591), [page-023](runtime_probe/pages/page-023.png) |
| ogc-system::sync_sram | System SRAM sync reports success | PASS | [log L592](runtime_probe/probe.log#L592), [page-023](runtime_probe/pages/page-023.png) |
| ogc-system::sync_sram | SRAM setter fixture restores both images | PASS | [log L602](runtime_probe/probe.log#L602), [page-023](runtime_probe/pages/page-023.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
