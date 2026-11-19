# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 216. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::deferred_reserve_interrupt_slot | LWP interrupt reservation cancels once | PASS | [log L229](lwp_probe/probe.log#L229), [page-009](lwp_probe/pages/page-009.png) |
| ogc-lwp-sys::deferred_reserve_interrupt_slot | LWP interrupt deferred submission completes | PASS | [log L230](lwp_probe/probe.log#L230), [page-009](lwp_probe/pages/page-009.png) |
| ogc-lwp-sys::deferred_cancel_interrupt_slot | LWP interrupt reservation cancels once | PASS | [log L229](lwp_probe/probe.log#L229), [page-009](lwp_probe/pages/page-009.png) |
| ogc-lwp-sys::deferred_cancel_interrupt_slot | LWP interrupt deferred submission completes | PASS | [log L230](lwp_probe/probe.log#L230), [page-009](lwp_probe/pages/page-009.png) |
| ogc-lwp-sys::deferred_call_from_interrupt | LWP interrupt deferred submission completes | PASS | [log L230](lwp_probe/probe.log#L230), [page-009](lwp_probe/pages/page-009.png) |
| ogc-lwp-sys::deferred_call_reserved_from_interrupt | LWP interrupt deferred submission completes | PASS | [log L230](lwp_probe/probe.log#L230), [page-009](lwp_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin decrementer interrupt and deferred worker; queue saturation, concurrent hardware interrupts and physical Wii timing are not covered.
