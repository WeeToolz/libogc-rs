# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 618. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_start_pmc | PMC start stores control values | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_dump_pmc | PMC dump marks counters read | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_stop_pmc | PMC stop marks counters stopped | PASS | [log L257](runtime_probe/probe.log#L257), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::sys_reset_pmc | PMC reset clears started state | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| ogc-system-sys::snapshot_pmc | PMC reset clears started state | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
