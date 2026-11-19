# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 502. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::write | IPC write persists bytes | PASS | [log L580](runtime_probe/probe.log#L580), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::write | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::write_exact | IPC exact write persists bytes | PASS | [log L581](runtime_probe/probe.log#L581), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::write_exact | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::write_async | IPC async write completes and persists | PASS | [log L582](runtime_probe/probe.log#L582), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::write_async | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::seek_async | IPC async seek selects offset | PASS | [log L583](runtime_probe/probe.log#L583), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::seek_async | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::read_async | IPC async read returns bytes | PASS | [log L584](runtime_probe/probe.log#L584), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::read_async | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::close_async | IPC async close completes and persists | PASS | [log L585](runtime_probe/probe.log#L585), [page-024](runtime_probe/pages/page-024.png) |
| ogc-ipc::close_async | IPC test file cleanup | PASS | [log L586](runtime_probe/probe.log#L586), [page-024](runtime_probe/pages/page-024.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IOS uses a temporary file in a private NAND copy. Physical Wii, async error paths, concurrent requests, and other file types remain unverified.
