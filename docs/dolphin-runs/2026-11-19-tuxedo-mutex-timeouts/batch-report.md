# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 235. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tuxedo-sys::k_mutex_lock | Tuxedo mutex lock try unlock and wrapper ownership | PASS | [log L295](lwp_probe/probe.log#L295), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_mutex_try_lock | Tuxedo mutex lock try unlock and wrapper ownership | PASS | [log L295](lwp_probe/probe.log#L295), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_mutex_unlock | Tuxedo mutex lock try unlock and wrapper ownership | PASS | [log L295](lwp_probe/probe.log#L295), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_mutex_is_locked_by_current_thread | Tuxedo mutex lock try unlock and wrapper ownership | PASS | [log L295](lwp_probe/probe.log#L295), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::mutex_try_lock | Tuxedo mutex lock try unlock and wrapper ownership | PASS | [log L295](lwp_probe/probe.log#L295), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_rmutex_lock | Tuxedo recursive mutex preserves owner until final unlock | PASS | [log L296](lwp_probe/probe.log#L296), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_rmutex_try_lock | Tuxedo recursive mutex preserves owner until final unlock | PASS | [log L296](lwp_probe/probe.log#L296), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_rmutex_unlock | Tuxedo recursive mutex preserves owner until final unlock | PASS | [log L296](lwp_probe/probe.log#L296), [page-011](lwp_probe/pages/page-011.png) |
| tuxedo-sys::k_cond_var_wait_timeout_ticks | Tuxedo condition timeouts reacquire mutex | PASS | [log L298](lwp_probe/probe.log#L298), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait_ns | Tuxedo condition timeouts reacquire mutex | PASS | [log L298](lwp_probe/probe.log#L298), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait_us | Tuxedo condition timeouts reacquire mutex | PASS | [log L298](lwp_probe/probe.log#L298), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait_ms | Tuxedo condition timeouts reacquire mutex | PASS | [log L298](lwp_probe/probe.log#L298), [page-012](lwp_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Single-thread ownership and one-millisecond condition timeouts in Dolphin. Cross-thread mutex contention, signaled condition waits, timeout races, and physical Wii scheduling remain untested.
