# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 495. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::raw | IPC descriptor identifies open file | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::seek | IPC seek selects file offsets | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::read | IPC reads settings bytes | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::read_exact | IPC exact read detects short file | PASS | [log L252](runtime_probe/probe.log#L252), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IOS reads the copied NAND setting.txt. Other files, write operations, async callbacks, and physical Wii remain unverified.
