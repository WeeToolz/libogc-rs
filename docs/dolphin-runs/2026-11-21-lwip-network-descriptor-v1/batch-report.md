# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 781. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::set_network_top_fd | lwIP network descriptor gates IOS socket creation | PASS | [log L1107](runtime_probe/probe.log#L1107), [page-041](runtime_probe/pages/page-041.png) |
| lwip-sys::set_network_top_fd | lwIP descriptor probe restores network lifecycle | PASS | [log L1108](runtime_probe/probe.log#L1108), [page-041](runtime_probe/pages/page-041.png) |
| lwip-sys::set_network_top_fd | lwIP network descriptor gates IOS socket creation | PASS | [log L1107](runtime_probe/probe.log#L1107), [page-041](runtime_probe/pages/page-041.png) |
| lwip-sys::set_network_top_fd | lwIP descriptor probe restores network lifecycle | PASS | [log L1108](runtime_probe/probe.log#L1108), [page-041](runtime_probe/pages/page-041.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
