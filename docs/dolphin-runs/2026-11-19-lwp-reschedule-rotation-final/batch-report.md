# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 220. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::reschedule | LWP reschedule rotates equal priority workers | PASS | [log L97](lwp_probe/probe.log#L97), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-sys::lwp_reschedule_abi | LWP ABI reschedule rotates equal priority workers | PASS | [log L99](lwp_probe/probe.log#L99), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Two ready threads at equal priority in Dolphin; other priorities, interruption and physical Wii timing are not covered.
