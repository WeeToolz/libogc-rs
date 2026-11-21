# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 647. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::sys_allocate_framebuffer | System framebuffer rejects null mode | PASS | [log L259](runtime_probe/probe.log#L259), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
