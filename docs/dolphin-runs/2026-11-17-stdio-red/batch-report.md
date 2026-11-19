# Dolphin evidence report

Probe `callback_probe` reported `probe_fail`.
Passed checks 12. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::stdio_report | stdio report installs UART stream writers | FAIL | [log L87](callback_probe/probe.log#L87), [page-004](callback_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-system::stdio_report — stdio report installs UART stream writers (FAIL)

## Coverage limits

- Dolphin stdio redirection. Physical UART availability needs separate evidence.
