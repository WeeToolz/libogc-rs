# Dolphin evidence report

Probe `callback_probe` reported `pass`.
Passed checks 12. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::diet_print_set_func | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-003](callback_probe/pages/page-003.png) |
| ogc-system::try_report | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-003](callback_probe/pages/page-003.png) |
| ogc-system::report | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-003](callback_probe/pages/page-003.png) |
| ogc-system::last_report_succeeded | report callback receives formatted bytes | PASS | [log L86](callback_probe/probe.log#L86), [page-003](callback_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Formatted callback delivery in Dolphin. Default UART output, callback concurrency, and physical hardware require separate checks.

## Previous run

[Previous suite result](../2026-11-17-report-callback-diagnostic/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| power callback dispatch | 0 pass, 1 fail | 1 pass, 0 fail |
