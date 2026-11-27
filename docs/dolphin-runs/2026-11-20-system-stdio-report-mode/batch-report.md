# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 736. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_stdio_report | Raw stdio report selects and clears stdout mode | FAIL | [log L267](runtime_probe/probe.log#L267), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-system-sys::sys_stdio_report — Raw stdio report selects and clears stdout mode (FAIL)

## Coverage limits

- Checks the observable report mode state in Dolphin. UART output and physical hardware routing remain unverified.
