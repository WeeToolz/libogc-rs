# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 278. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::note_ipc_wake | LWP sys IPC wake note increments counter | PASS | [log L366](lwp_probe/probe.log#L366), [page-014](lwp_probe/pages/page-014.png) |
| ogc-lwp-sys::note_ipc_wake | reply interrupt woke LWP queue | PASS | [log L372](lwp_probe/probe.log#L372), [page-015](lwp_probe/pages/page-015.png) |
| ogc-lwp-sys::initialize | LWP sys repeated initialization preserves scheduler state | PASS | [log L367](lwp_probe/probe.log#L367), [page-014](lwp_probe/pages/page-014.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
