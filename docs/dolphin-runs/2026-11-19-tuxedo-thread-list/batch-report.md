# Dolphin evidence report

Probe `lwp_probe` reported `probe_fail`.
Passed checks 241. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tuxedo-sys::empty | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_context_snapshot | Tuxedo suspended thread context has aligned stack | FAIL | [log L307](lwp_probe/probe.log#L307), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_find_physical | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_first | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_get_self | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_next | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_set_prio | Tuxedo suspended thread priority resume and exit | PASS | [log L308](lwp_probe/probe.log#L308), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_suspend | Tuxedo suspended thread priority resume and exit | PASS | [log L308](lwp_probe/probe.log#L308), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_sleep_ticks | Tuxedo sleep units delay and yield returns | PASS | [log L309](lwp_probe/probe.log#L309), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_sleep_us | Tuxedo sleep units delay and yield returns | PASS | [log L309](lwp_probe/probe.log#L309), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_sleep_ms | Tuxedo sleep units delay and yield returns | PASS | [log L309](lwp_probe/probe.log#L309), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_yield | Tuxedo sleep units delay and yield returns | PASS | [log L309](lwp_probe/probe.log#L309), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::thread_prepare | Tuxedo thread list finds suspended worker and current thread | PASS | [log L306](lwp_probe/probe.log#L306), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::thread_resume | Tuxedo suspended thread priority resume and exit | PASS | [log L308](lwp_probe/probe.log#L308), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_thread_exit | Tuxedo suspended thread priority resume and exit | PASS | [log L308](lwp_probe/probe.log#L308), [page-012](lwp_probe/pages/page-012.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- tuxedo-sys::k_thread_context_snapshot — Tuxedo suspended thread context has aligned stack (FAIL)

## Coverage limits

- One suspended worker and main thread in Dolphin. Multi-thread list ordering, priority contention, register preservation, and physical Wii timing remain open.
