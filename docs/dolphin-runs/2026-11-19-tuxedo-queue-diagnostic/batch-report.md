# Dolphin evidence report

Probe `lwp_probe` reported `probe_fail`.
Passed checks 242. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tuxedo-sys::k_thr_queue_unblock_one_by_mask | Tuxedo queue mask wakes one and cancels other | Missing |  |
| tuxedo-sys::k_thr_queue_unblock_all_by_mask | Tuxedo queue mask wakes all matching tokens | Missing |  |
| tuxedo-sys::k_thr_queue_unblock_all_by_value | Tuxedo queue value wakes all matching tokens | Missing |  |
| tuxedo-sys::k_thr_queue_cancel | Tuxedo queue cancellation wakes both with zero | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- tuxedo-sys::k_thr_queue_unblock_one_by_mask — Tuxedo queue mask wakes one and cancels other (Missing)
- tuxedo-sys::k_thr_queue_unblock_all_by_mask — Tuxedo queue mask wakes all matching tokens (Missing)
- tuxedo-sys::k_thr_queue_unblock_all_by_value — Tuxedo queue value wakes all matching tokens (Missing)
- tuxedo-sys::k_thr_queue_cancel — Tuxedo queue cancellation wakes both with zero (Missing)
- Other failed check — Tuxedo queue workers reached wait

## Coverage limits

- Two Dolphin workers with selected masks and tokens. Queue priority ordering, repeated cancellation races, and physical Wii scheduling remain open.
