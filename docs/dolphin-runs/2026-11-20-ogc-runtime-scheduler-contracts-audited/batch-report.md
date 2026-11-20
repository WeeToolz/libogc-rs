# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 258. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-runtime::get | runtime thread ID returns its scheduler handle | PASS | [log L79](lwp_probe/probe.log#L79), [page-002](lwp_probe/pages/page-002.png) |
| ogc-runtime::now | runtime monotonic clock returns a tick value | PASS | [log L80](lwp_probe/probe.log#L80), [page-002](lwp_probe/pages/page-002.png) |
| ogc-runtime::ticks_per_second | runtime clock reports the LWP timebase rate | PASS | [log L82](lwp_probe/probe.log#L82), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::register | runtime timer rejects the null wake token | PASS | [log L83](lwp_probe/probe.log#L83), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::cancel | runtime timer cancellation accepts an absent token | PASS | [log L84](lwp_probe/probe.log#L84), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::enqueue | runtime deferred executor accepts a callback | PASS | [log L85](lwp_probe/probe.log#L85), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::wake | runtime deferred executor rejects a null wake token | PASS | [log L86](lwp_probe/probe.log#L86), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::wake_from_interrupt | runtime interrupt wake rejects a null token | PASS | [log L87](lwp_probe/probe.log#L87), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
