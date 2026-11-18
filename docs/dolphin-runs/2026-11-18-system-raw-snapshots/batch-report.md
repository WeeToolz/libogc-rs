# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 465. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-system-sys::snapshot_startup | Raw system snapshots report completed startup | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_pre_main | Raw system snapshots report completed startup | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_runtime_calls | Raw system snapshots report completed startup | PASS | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_framebuffer | Raw system framebuffer snapshot records allocation | PASS | [log L217](runtime_probe/probe.log#L217), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system-sys::snapshot_time_probe | Raw system time snapshot records call | PASS | [log L218](runtime_probe/probe.log#L218), [page-008](runtime_probe/pages/page-008.png) |
| ogc-system::snapshot | System aggregate snapshot includes startup and framebuffer | PASS | [log L219](runtime_probe/probe.log#L219), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin successful startup, framebuffer allocation and time call. Failure paths, other aggregate fields and physical Wii remain unverified.
