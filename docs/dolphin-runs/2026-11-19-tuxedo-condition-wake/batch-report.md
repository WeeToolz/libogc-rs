# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 238. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tuxedo-sys::k_cond_var_signal | Tuxedo condition signal wakes one mutex owner | PASS | [log L301](lwp_probe/probe.log#L301), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_broadcast | Tuxedo condition broadcast wakes remaining mutex owner | PASS | [log L302](lwp_probe/probe.log#L302), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait | Tuxedo condition signal wakes one mutex owner | PASS | [log L301](lwp_probe/probe.log#L301), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait | Tuxedo condition broadcast wakes remaining mutex owner | PASS | [log L302](lwp_probe/probe.log#L302), [page-012](lwp_probe/pages/page-012.png) |
| tuxedo-sys::k_cond_var_wait | Tuxedo condition workers join after wake | PASS | [log L303](lwp_probe/probe.log#L303), [page-012](lwp_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Two waiting workers and one signal followed by broadcast in Dolphin. Other queue ordering, multiple conditions, wake races, and physical Wii timing remain open.
