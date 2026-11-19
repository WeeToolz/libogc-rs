# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 304. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system::snapshot_framebuffer | System framebuffer snapshot matches preferred mode allocation | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |
| ogc-system::last_framebuffer_bytes | System framebuffer snapshot matches preferred mode allocation | PASS | [log L136](runtime_probe/probe.log#L136), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin preferred-mode allocation at startup. Allocation failure, other video modes, and physical hardware need separate evidence.
