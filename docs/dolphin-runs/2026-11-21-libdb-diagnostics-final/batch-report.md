# Dolphin evidence report

Probe `libdb_diagnostics_probe` reported `pass`.
Passed checks 6. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| libdb-sys::diagnostic_failure | Wii network invalid port maps to a generic diagnostic | PASS | [log L75](libdb_diagnostics_probe/probe.log#L75), [page-002](libdb_diagnostics_probe/pages/page-002.png) |
| libdb-sys::failure_from_socket | Wii network socket failure preserves operation, code, and cleanup | PASS | [log L74](libdb_diagnostics_probe/probe.log#L74), [page-002](libdb_diagnostics_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
