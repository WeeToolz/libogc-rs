# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 783. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format bridge rejects null call | PASS | [log L447](runtime_probe/probe.log#L447), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format bridge returns ES title ID through output vector | FAIL | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format bridge closes ES descriptor | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-ipc::__ogc_ipc_ioctlv_format_bridge — IPC format bridge returns ES title ID through output vector (FAIL)
