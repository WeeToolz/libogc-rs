# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 333. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::ioctl | lwIP ioctl toggles socket nonblocking mode | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl rejects FIONREAD and preserves output | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl toggles socket nonblocking mode | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl rejects FIONREAD and preserves output | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll clears empty read readiness | PASS | [log L500](runtime_probe/probe.log#L500), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll reports queued read readiness | PASS | [log L502](runtime_probe/probe.log#L502), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP readiness corresponds to queued datagram | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::poll | lwIP poll clears empty read readiness | PASS | [log L500](runtime_probe/probe.log#L500), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll reports queued read readiness | PASS | [log L502](runtime_probe/probe.log#L502), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP readiness corresponds to queued datagram | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::select | lwIP select clears empty read readiness | PASS | [log L501](runtime_probe/probe.log#L501), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select reports queued read readiness | PASS | [log L503](runtime_probe/probe.log#L503), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP readiness corresponds to queued datagram | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::select | lwIP select clears empty read readiness | PASS | [log L501](runtime_probe/probe.log#L501), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select reports queued read readiness | PASS | [log L503](runtime_probe/probe.log#L503), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP readiness corresponds to queued datagram | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 UDP loopback, one readable descriptor and FIONBIO/FIONREAD only. Multiple descriptors, timeout edge cases, other commands and physical Wii remain unverified.
