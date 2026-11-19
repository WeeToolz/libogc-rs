# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 227. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::wake_thread_with_token | LWP external direct wake returns token | PASS | [log L109](lwp_probe/probe.log#L109), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::wake_external_by_value | LWP external value wake returns signal | PASS | [log L116](lwp_probe/probe.log#L116), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::wake_external_by_mask | LWP external mask wake returns matched bits | PASS | [log L123](lwp_probe/probe.log#L123), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::broadcast_external | LWP external broadcast wakes waiter | PASS | [log L131](lwp_probe/probe.log#L131), [page-005](lwp_probe/pages/page-005.png) |
| ogc-lwp-driver::cancel_external_wait | LWP external cancellation returns zero | PASS | [log L138](lwp_probe/probe.log#L138), [page-005](lwp_probe/pages/page-005.png) |
| ogc-lwp-driver::block_current_external | LWP external direct wake returns token | PASS | [log L109](lwp_probe/probe.log#L109), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::block_current_external | LWP external value wake returns signal | PASS | [log L116](lwp_probe/probe.log#L116), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::block_current_external | LWP external mask wake returns matched bits | PASS | [log L123](lwp_probe/probe.log#L123), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::block_current_external | LWP external broadcast wakes waiter | PASS | [log L131](lwp_probe/probe.log#L131), [page-005](lwp_probe/pages/page-005.png) |
| ogc-lwp-driver::block_current_external | LWP external cancellation returns zero | PASS | [log L138](lwp_probe/probe.log#L138), [page-005](lwp_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Single blocked waiter in Dolphin; multi-waiter selection, pending-wake races and physical Wii timing are not covered.
