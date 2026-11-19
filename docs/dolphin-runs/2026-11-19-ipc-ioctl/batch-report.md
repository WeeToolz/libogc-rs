# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 509. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::ioctl | IPC ioctl reads file stats | PASS | [log L254](runtime_probe/probe.log#L254), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::ioctl_result | IPC ioctl_result reads guarded file stats | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::ioctl_async | IPC ioctl_async returns file stats | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::ioctlv | IPC ioctlv returns MAC | PASS | [log L643](runtime_probe/probe.log#L643), [page-026](runtime_probe/pages/page-026.png) |
| ogc-ipc::ioctlv_result | IPC ioctlv_result returns MAC | PASS | [log L645](runtime_probe/probe.log#L645), [page-027](runtime_probe/pages/page-027.png) |
| ogc-ipc::ioctlv_raw | IPC ioctlv_raw returns MAC | PASS | [log L646](runtime_probe/probe.log#L646), [page-027](runtime_probe/pages/page-027.png) |
| ogc-ipc::ioctlv_async_raw | IPC ioctlv_async_raw returns MAC | PASS | [log L647](runtime_probe/probe.log#L647), [page-027](runtime_probe/pages/page-027.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IOS file-stat and network MAC requests only. Other commands, error paths, concurrent requests, and physical Wii remain unverified.
