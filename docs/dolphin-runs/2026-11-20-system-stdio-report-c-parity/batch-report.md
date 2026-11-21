# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 741. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_stdio_report | Raw stdio report keeps stdout routing selected after a false call | PASS | [log L314](runtime_probe/probe.log#L314), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin confirms routing mode state. It does not confirm UART output or physical hardware routing.
