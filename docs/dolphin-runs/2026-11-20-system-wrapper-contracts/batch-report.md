# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 679. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::check_argv | System argv wrapper preserves the checked descriptor | PASS | [log L297](runtime_probe/probe.log#L297), [page-012](runtime_probe/pages/page-012.png) |
| ogc-system::stdio_report_uses_stdout | System stdio wrapper matches the raw report mode | PASS | [log L298](runtime_probe/probe.log#L298), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The probe verifies wrapper state and argument descriptor stability in Dolphin. It does not test host-only behavior or stdio output to physical hardware.
