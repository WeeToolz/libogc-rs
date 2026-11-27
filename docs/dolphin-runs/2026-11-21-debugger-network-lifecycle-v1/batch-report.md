# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 778. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| libdb-sys::bind | debugger network binds and closes listener | PASS | [log L1107](runtime_probe/probe.log#L1107), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::bind_with_control | debugger network accepts local client | PASS | [log L1108](runtime_probe/probe.log#L1108), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::poll | debugger network accepts local client | PASS | [log L1108](runtime_probe/probe.log#L1108), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::state | debugger network accepts local client | PASS | [log L1108](runtime_probe/probe.log#L1108), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::state | debugger network arm and disarm change state | PASS | [log L1109](runtime_probe/probe.log#L1109), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::state | debugger network closes connected transport | PASS | [log L1110](runtime_probe/probe.log#L1110), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::arm | debugger network arm and disarm change state | PASS | [log L1109](runtime_probe/probe.log#L1109), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::disarm | debugger network arm and disarm change state | PASS | [log L1109](runtime_probe/probe.log#L1109), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::close | debugger network closes connected transport | PASS | [log L1110](runtime_probe/probe.log#L1110), [page-041](runtime_probe/pages/page-041.png) |
| libdb-sys::close | debugger network probe deinitializes | PASS | [log L1111](runtime_probe/probe.log#L1111), [page-041](runtime_probe/pages/page-041.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
