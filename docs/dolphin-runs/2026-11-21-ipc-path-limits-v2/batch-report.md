# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 771. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::validate_path | IPC path validation accepts 63 bytes and rejects 64 bytes | PASS | [log L445](runtime_probe/probe.log#L445), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::validate_path | IPC async open rejects oversized path before submission | PASS | [log L446](runtime_probe/probe.log#L446), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
