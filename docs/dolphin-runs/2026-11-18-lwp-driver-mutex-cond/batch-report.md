# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 419. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::init_mutex | LWP driver mutex rejects repeated nonrecursive lock | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::mutex_try_lock | LWP driver mutex rejects repeated nonrecursive lock | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::mutex_try_lock | LWP driver mutex can be locked after release | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::close_mutex | LWP driver closed mutex rejects lock | PASS | [log L124](runtime_probe/probe.log#L124), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::init_cond | LWP driver empty condition broadcasts | PASS | [log L125](runtime_probe/probe.log#L125), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::cond_broadcast | LWP driver empty condition broadcasts | PASS | [log L125](runtime_probe/probe.log#L125), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::cond_broadcast | LWP driver closed condition rejects broadcast | PASS | [log L126](runtime_probe/probe.log#L126), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::close_cond | LWP driver closed condition rejects broadcast | PASS | [log L126](runtime_probe/probe.log#L126), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded nonrecursive mutex and idle condition. Contention, recursive locking, waiters and physical Wii remain unverified.
