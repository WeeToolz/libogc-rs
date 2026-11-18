# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 303. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_pre_main | System startup snapshots report completed initialization | PASS | [log L134](runtime_probe/probe.log#L134), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::runtime_fp_disabled_stage | System startup snapshots report completed initialization | PASS | [log L134](runtime_probe/probe.log#L134), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::validate | System startup validation preserves failed stage and incomplete state | PASS | [log L135](runtime_probe/probe.log#L135), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Successful Dolphin startup and pure validation branches. Startup subsystem failures and physical hardware require separate checks.
