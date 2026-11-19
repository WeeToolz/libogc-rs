# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 427. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::allocate_workspace | LWP workspace retains separate allocations | PASS | [log L127](runtime_probe/probe.log#L127), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-sys::release_workspace | LWP workspace allocates after release | PASS | [log L128](runtime_probe/probe.log#L128), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-sys::mutex_try_lock | LWP raw try lock reports busy | PASS | [log L129](runtime_probe/probe.log#L129), [page-004](runtime_probe/pages/page-004.png) |
| ogc-lwp-sys::sem_get_value | LWP raw semaphore reports its count | PASS | [log L131](runtime_probe/probe.log#L131), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::init_queue | LWP raw closed queue rejects sleep | PASS | [log L132](runtime_probe/probe.log#L132), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::close_queue | LWP raw closed queue rejects sleep | PASS | [log L132](runtime_probe/probe.log#L132), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::thread_sleep | LWP raw closed queue rejects sleep | PASS | [log L132](runtime_probe/probe.log#L132), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_init_queue_abi | LWP ABI closed queue rejects sleep | PASS | [log L133](runtime_probe/probe.log#L133), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_close_queue_abi | LWP ABI closed queue rejects sleep | PASS | [log L133](runtime_probe/probe.log#L133), [page-005](runtime_probe/pages/page-005.png) |
| ogc-lwp-sys::lwp_thread_sleep_abi | LWP ABI closed queue rejects sleep | PASS | [log L133](runtime_probe/probe.log#L133), [page-005](runtime_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin single-threaded workspace and idle synchronization resources. Waiters, contention, allocator exhaustion and physical Wii remain unverified.
