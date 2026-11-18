# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 401. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::create | IPC heap rejects nonpositive size | PASS | [log L175](runtime_probe/probe.log#L175), [page-006](runtime_probe/pages/page-006.png) |
| ogc-ipc::create | IPC created heap has valid ID | PASS | [log L178](runtime_probe/probe.log#L178), [page-007](runtime_probe/pages/page-007.png) |
| ogc-ipc::raw | IPC created heap has valid ID | PASS | [log L178](runtime_probe/probe.log#L178), [page-007](runtime_probe/pages/page-007.png) |
| ogc-ipc::default_heap | IPC default heap remains stable | PASS | [log L176](runtime_probe/probe.log#L176), [page-006](runtime_probe/pages/page-006.png) |
| ogc-ipc::alloc | IPC allocations are aligned and distinct | PASS | [log L179](runtime_probe/probe.log#L179), [page-007](runtime_probe/pages/page-007.png) |
| ogc-ipc::free | IPC freed space supports a larger allocation | PASS | [log L180](runtime_probe/probe.log#L180), [page-007](runtime_probe/pages/page-007.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded heap creation and allocation. Exhaustion, concurrent access and physical Wii remain unverified.
