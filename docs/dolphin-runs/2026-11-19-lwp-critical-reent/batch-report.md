# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 222. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::critical_enter | LWP nested critical sections restore interrupt state | PASS | [log L100](lwp_probe/probe.log#L100), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::critical_exit | LWP nested critical sections restore interrupt state | PASS | [log L100](lwp_probe/probe.log#L100), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::current_reent | LWP newlib storage is stable and thread local | PASS | [log L101](lwp_probe/probe.log#L101), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::__getreent | LWP newlib storage is stable and thread local | PASS | [log L101](lwp_probe/probe.log#L101), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin main and one child thread; exception-context reentrancy and physical Wii interrupt timing are not covered.
