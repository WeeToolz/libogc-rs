# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 218. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::set_thread_priority | LWP priority change selects higher worker | PASS | [log L94](lwp_probe/probe.log#L94), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-sys::lwp_set_thread_priority_abi | LWP ABI priority change selects higher worker | PASS | [log L95](lwp_probe/probe.log#L95), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Two ready threads below the main thread on Dolphin; priority inheritance, equal-priority rotation and physical Wii timing are not covered.
