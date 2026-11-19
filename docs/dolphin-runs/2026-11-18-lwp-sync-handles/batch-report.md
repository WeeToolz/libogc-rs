# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 389. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::init | LWP queue handle retains output | PASS | [log L109](runtime_probe/probe.log#L109), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::raw | LWP queue handle retains output | PASS | [log L109](runtime_probe/probe.log#L109), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::into_raw | LWP queue handle retains output | PASS | [log L109](runtime_probe/probe.log#L109), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::init | LWP condition handle retains output | PASS | [log L110](runtime_probe/probe.log#L110), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::raw | LWP condition handle retains output | PASS | [log L110](runtime_probe/probe.log#L110), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::signal | LWP condition signal and broadcast succeed | PASS | [log L111](runtime_probe/probe.log#L111), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::broadcast | LWP condition signal and broadcast succeed | PASS | [log L111](runtime_probe/probe.log#L111), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::destroy | LWP condition destroy succeeds | PASS | [log L112](runtime_probe/probe.log#L112), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::init | LWP semaphore handle retains output | PASS | [log L113](runtime_probe/probe.log#L113), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::raw | LWP semaphore handle retains output | PASS | [log L113](runtime_probe/probe.log#L113), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp::destroy | LWP semaphore destroy succeeds | PASS | [log L114](runtime_probe/probe.log#L114), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded initialization and idle synchronization objects. Waiters, contention, failure paths and physical Wii remain unverified.
