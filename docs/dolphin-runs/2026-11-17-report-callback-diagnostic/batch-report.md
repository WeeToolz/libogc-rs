# Dolphin evidence report

Probe `callback_probe` reported `probe_fail`.
Passed checks 11. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::diet_print_set_func | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-004](callback_probe/pages/page-004.png) |
| ogc-system::try_report | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-004](callback_probe/pages/page-004.png) |
| ogc-system::report | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-004](callback_probe/pages/page-004.png) |
| ogc-system::last_report_succeeded | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-004](callback_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — power callback dispatch

## Coverage limits

- Formatted callback delivery in Dolphin. Default UART output, callback concurrency, and physical hardware require separate checks.

## Previous run

[Previous suite result](../2026-11-17-report-callback-repeat/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| Check counts unchanged | | |
