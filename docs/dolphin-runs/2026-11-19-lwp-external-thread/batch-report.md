# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 232. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-driver::create_thread_at_top | LWP stack-top worker starts suspended | PASS | [log L103](lwp_probe/probe.log#L103), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::initialize_external_reent | LWP external reentrancy storage initializes | PASS | [log L102](lwp_probe/probe.log#L102), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::set_thread_context_mirror | LWP suspended thread context mirror is writable | PASS | [log L104](lwp_probe/probe.log#L104), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-driver::set_thread_reent_storage | LWP external reentrancy reaches worker | PASS | [log L106](lwp_probe/probe.log#L106), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::thread_exit | LWP explicit thread exit returns value | PASS | [log L107](lwp_probe/probe.log#L107), [page-004](lwp_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One suspended worker using caller-owned stack, context and newlib state in Dolphin; concurrent mutation, physical Wii timing and all newlib fields are not covered.
