# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 371. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::init | LWP message queue handle matches initialized queue | PASS | [log L104](runtime_probe/probe.log#L104), [page-003](runtime_probe/pages/page-003.png) |
| ogc-lwp::init | LWP message queue returns urgent before FIFO and reports empty | PASS | [log L107](runtime_probe/probe.log#L107), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::raw | LWP message queue handle matches initialized queue | PASS | [log L104](runtime_probe/probe.log#L104), [page-003](runtime_probe/pages/page-003.png) |
| ogc-lwp::send | LWP message queue rejects send when full | PASS | [log L105](runtime_probe/probe.log#L105), [page-003](runtime_probe/pages/page-003.png) |
| ogc-lwp::send | LWP closed message queue rejects send | PASS | [log L108](runtime_probe/probe.log#L108), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::jam | LWP message queue rejects send when full | PASS | [log L105](runtime_probe/probe.log#L105), [page-003](runtime_probe/pages/page-003.png) |
| ogc-lwp::jam | LWP message queue returns urgent before FIFO and reports empty | PASS | [log L107](runtime_probe/probe.log#L107), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::receive | LWP message queue returns urgent before FIFO and reports empty | PASS | [log L107](runtime_probe/probe.log#L107), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded nonblocking queue operations only. Blocking wakeups, concurrency, exhaustion and physical Wii remain unverified.
