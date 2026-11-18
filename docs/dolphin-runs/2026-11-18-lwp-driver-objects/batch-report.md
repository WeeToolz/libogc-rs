# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 414. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::init_semaphore | LWP driver semaphore starts at two | PASS | [log L115](runtime_probe/probe.log#L115), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::semaphore_get_value | LWP driver semaphore starts at two | PASS | [log L115](runtime_probe/probe.log#L115), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::semaphore_get_value | LWP driver semaphore wait decrements | PASS | [log L116](runtime_probe/probe.log#L116), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::semaphore_get_value | LWP driver semaphore post increments | PASS | [log L117](runtime_probe/probe.log#L117), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::semaphore_wait | LWP driver semaphore wait decrements | PASS | [log L116](runtime_probe/probe.log#L116), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::semaphore_post | LWP driver semaphore post increments | PASS | [log L117](runtime_probe/probe.log#L117), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::close_semaphore | LWP driver closed semaphore rejects access | PASS | [log L118](runtime_probe/probe.log#L118), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::init_message_queue | LWP driver queue rejects overflow | PASS | [log L119](runtime_probe/probe.log#L119), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::message_send | LWP driver queue rejects overflow | PASS | [log L119](runtime_probe/probe.log#L119), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::message_send | LWP driver closed queue rejects send | PASS | [log L121](runtime_probe/probe.log#L121), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::message_jam | LWP driver queue returns urgent then FIFO | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::message_receive | LWP driver queue returns urgent then FIFO | PASS | [log L120](runtime_probe/probe.log#L120), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-driver::close_message_queue | LWP driver closed queue rejects send | PASS | [log L121](runtime_probe/probe.log#L121), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded nonblocking message queue and semaphore operations. Waiters, contention, exhaustion and physical Wii remain unverified.
