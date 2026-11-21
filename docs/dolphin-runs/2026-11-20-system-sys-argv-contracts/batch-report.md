# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 677. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::current_system_argv | System sys argv accessor returns a stable descriptor | PASS | [log L294](runtime_probe/probe.log#L294), [page-011](runtime_probe/pages/page-011.png) |
| ogc-system-sys::_check_argv | System sys argv check preserves a valid stable descriptor | PASS | [log L295](runtime_probe/probe.log#L295), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin tests the runtime-owned Wii argument descriptor after startup. It does not test host compatibility storage or every argument encoding.
