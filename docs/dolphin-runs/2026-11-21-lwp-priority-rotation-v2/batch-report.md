# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 276. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp::reschedule | LWP wrapper reschedule rotates equal priority workers | PASS | [log L130](lwp_probe/probe.log#L130), [page-005](lwp_probe/pages/page-005.png) |
| ogc-lwp::set_priority | LWP wrapper priority change selects higher worker | PASS | [log L123](lwp_probe/probe.log#L123), [page-004](lwp_probe/pages/page-004.png) |
| ogc-lwp-driver::reschedule | LWP driver reschedule rotates equal priority workers | PASS | [log L132](lwp_probe/probe.log#L132), [page-005](lwp_probe/pages/page-005.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
