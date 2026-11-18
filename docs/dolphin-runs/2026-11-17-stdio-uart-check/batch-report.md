# Dolphin evidence report

Probe `callback_probe` reported `pass`.
Passed checks 13. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::stdio_report | stdio report installs UART stream writers | PASS | [log L87](callback_probe/probe.log#L87), [page-003](callback_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin stdio redirection. Physical UART availability needs separate evidence.

## Previous run

[Previous suite result](../2026-11-17-stdio-red/suite.json)

| Check | Previous | Current |
| --- | --- | --- |
| stdio report installs UART stream writers | 0 pass, 1 fail | 1 pass, 0 fail |
