# Dolphin evidence report

Probe `lwp_probe` reported `probe_fail`.
Passed checks 218. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-lwp-sys::reschedule | LWP reschedule rotates equal priority workers | FAIL | [log L97](lwp_probe/probe.log#L97), [page-003](lwp_probe/pages/page-003.png) |
| ogc-lwp-sys::lwp_reschedule_abi | LWP ABI reschedule rotates equal priority workers | FAIL | [log L99](lwp_probe/probe.log#L99), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-lwp-sys::reschedule — LWP reschedule rotates equal priority workers (FAIL)
- ogc-lwp-sys::lwp_reschedule_abi — LWP ABI reschedule rotates equal priority workers (FAIL)

## Coverage limits

- Two ready threads at equal priority in Dolphin; other priorities, interruption and physical Wii timing are not covered.
