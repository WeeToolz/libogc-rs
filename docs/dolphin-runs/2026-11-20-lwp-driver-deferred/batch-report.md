# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 250. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::initialize | LWP driver repeat initialization preserves current thread | PASS | [log L283](lwp_probe/probe.log#L283), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::deferred_enqueue | LWP driver deferred worker runs and rejects empty callback | PASS | [log L285](lwp_probe/probe.log#L285), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::deferred_reserve_interrupt_slot | LWP driver reservation cancels once | PASS | [log L284](lwp_probe/probe.log#L284), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::deferred_cancel_interrupt_slot | LWP driver reservation cancels once | PASS | [log L284](lwp_probe/probe.log#L284), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::timer_start | LWP driver timer submits interrupt callbacks | PASS | [log L286](lwp_probe/probe.log#L286), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::deferred_enqueue_from_interrupt | LWP driver timer submits interrupt callbacks | PASS | [log L286](lwp_probe/probe.log#L286), [page-011](lwp_probe/pages/page-011.png) |
| ogc-lwp-driver::deferred_enqueue_reserved_from_interrupt | LWP driver timer submits interrupt callbacks | PASS | [log L286](lwp_probe/probe.log#L286), [page-011](lwp_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One deferred callback and one timer interrupt with reserved and unreserved submissions in Dolphin. Queue saturation, cancellation races, and physical Wii timing remain open.
