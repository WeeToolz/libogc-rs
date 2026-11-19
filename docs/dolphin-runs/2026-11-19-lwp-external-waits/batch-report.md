# Dolphin evidence report

Probe `lwp_probe` reported `probe_fail`.
Passed checks 222. Failed checks 5.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::wake_thread_with_token | LWP external direct wake returns token | FAIL | [log L102](lwp_probe/probe.log#L102), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::wake_external_by_value | LWP external value wake returns signal | FAIL | [log L103](lwp_probe/probe.log#L103), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::wake_external_by_mask | LWP external mask wake returns matched bits | FAIL | [log L104](lwp_probe/probe.log#L104), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::broadcast_external | LWP external broadcast wakes waiter | FAIL | [log L106](lwp_probe/probe.log#L106), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::cancel_external_wait | LWP external cancellation returns zero | FAIL | [log L107](lwp_probe/probe.log#L107), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::block_current_external | LWP external direct wake returns token | FAIL | [log L102](lwp_probe/probe.log#L102), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::block_current_external | LWP external value wake returns signal | FAIL | [log L103](lwp_probe/probe.log#L103), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::block_current_external | LWP external mask wake returns matched bits | FAIL | [log L104](lwp_probe/probe.log#L104), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::block_current_external | LWP external broadcast wakes waiter | FAIL | [log L106](lwp_probe/probe.log#L106), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::block_current_external | LWP external cancellation returns zero | FAIL | [log L107](lwp_probe/probe.log#L107), [page-004](lwp_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-lwp-driver::wake_thread_with_token — LWP external direct wake returns token (FAIL)
- ogc-lwp-driver::wake_external_by_value — LWP external value wake returns signal (FAIL)
- ogc-lwp-driver::wake_external_by_mask — LWP external mask wake returns matched bits (FAIL)
- ogc-lwp-driver::broadcast_external — LWP external broadcast wakes waiter (FAIL)
- ogc-lwp-driver::cancel_external_wait — LWP external cancellation returns zero (FAIL)
- ogc-lwp-driver::block_current_external — LWP external direct wake returns token (FAIL)
- ogc-lwp-driver::block_current_external — LWP external value wake returns signal (FAIL)
- ogc-lwp-driver::block_current_external — LWP external mask wake returns matched bits (FAIL)
- ogc-lwp-driver::block_current_external — LWP external broadcast wakes waiter (FAIL)
- ogc-lwp-driver::block_current_external — LWP external cancellation returns zero (FAIL)

## Coverage limits

- Single blocked waiter in Dolphin; multi-waiter selection, pending-wake races and physical Wii timing are not covered.
