# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 405. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-sysbase::add_device | Device registry finds the exact prefix | PASS | [log L181](runtime_probe/probe.log#L181), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::find_device | Device registry finds the exact prefix | PASS | [log L181](runtime_probe/probe.log#L181), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::FindDevice | Device registry finds the exact prefix | PASS | [log L181](runtime_probe/probe.log#L181), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::device_op_tab | Device registry returns the registered table | PASS | [log L182](runtime_probe/probe.log#L182), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::GetDeviceOpTab | Device registry returns the registered table | PASS | [log L182](runtime_probe/probe.log#L182), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::remove_device | Device registry removes the registered name | PASS | [log L183](runtime_probe/probe.log#L183), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::AddDevice | Device C registry removes by path | PASS | [log L184](runtime_probe/probe.log#L184), [page-007](runtime_probe/pages/page-007.png) |
| ogc-sysbase::RemoveDevice | Device C registry removes by path | PASS | [log L184](runtime_probe/probe.log#L184), [page-007](runtime_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin serialized registration of one synthetic device with no callbacks. Concurrent access, full table, file operations and physical Wii remain unverified.
