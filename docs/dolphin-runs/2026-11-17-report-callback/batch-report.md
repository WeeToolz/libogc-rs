# Dolphin evidence report

Probe `callback_probe` reported `probe_fail`.
Passed checks 11. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::diet_print_set_func | report callback receives formatted bytes | PASS | [log L81](callback_probe/probe.log#L81), [page-002](callback_probe/pages/page-002.png) |
| ogc-system::try_report | report callback receives formatted bytes | PASS | [log L81](callback_probe/probe.log#L81), [page-002](callback_probe/pages/page-002.png) |
| ogc-system::report | report callback receives formatted bytes | PASS | [log L81](callback_probe/probe.log#L81), [page-002](callback_probe/pages/page-002.png) |
| ogc-system::last_report_succeeded | report callback receives formatted bytes | PASS | [log L81](callback_probe/probe.log#L81), [page-002](callback_probe/pages/page-002.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — power callback dispatch

## Coverage limits

- Formatted callback delivery in Dolphin. Default UART output, callback concurrency, and physical hardware require separate checks.
