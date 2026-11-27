# Dolphin evidence report

Probe `debugger_probe` reported `crash`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| libdb-sys::diagnostic_failure | Wii network invalid port maps to a generic diagnostic | Missing |  |
| libdb-sys::failure_from_socket | Wii network socket failure preserves operation, code, and cleanup | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- libdb-sys::diagnostic_failure — Wii network invalid port maps to a generic diagnostic (Missing)
- libdb-sys::failure_from_socket — Wii network socket failure preserves operation, code, and cleanup (Missing)
