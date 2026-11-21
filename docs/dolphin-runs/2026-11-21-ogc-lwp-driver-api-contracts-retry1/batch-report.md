# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 272. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::note_ipc_wake | LWP IPC wake note increments counter | PASS | [log L359](lwp_probe/probe.log#L359), [page-014](lwp_probe/pages/page-014.png) |
| ogc-lwp-driver::set_thread_priority | LWP priority change selects higher worker | PASS | [log L120](lwp_probe/probe.log#L120), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::zeroed | LWP context constructor zeroes all register state | PASS | [log L94](lwp_probe/probe.log#L94), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The context constructor check validates the PowerPC register-state layout in Dolphin. Dolphin does not provide physical Wii timing evidence.
