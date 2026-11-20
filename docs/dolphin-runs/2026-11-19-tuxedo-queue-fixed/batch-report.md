# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 246. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tuxedo-sys::k_thr_queue_unblock_one_by_mask | Tuxedo queue mask wakes one and cancels other | PASS | [log L314](lwp_probe/probe.log#L314), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thr_queue_unblock_all_by_mask | Tuxedo queue mask wakes all matching tokens | PASS | [log L317](lwp_probe/probe.log#L317), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thr_queue_unblock_all_by_value | Tuxedo queue value wakes all matching tokens | PASS | [log L320](lwp_probe/probe.log#L320), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thr_queue_cancel | Tuxedo queue cancellation wakes both with zero | PASS | [log L324](lwp_probe/probe.log#L324), [page-013](lwp_probe/pages/page-013.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Two Dolphin workers with selected masks and tokens. Queue priority ordering, repeated cancellation races, and physical Wii scheduling remain open.
