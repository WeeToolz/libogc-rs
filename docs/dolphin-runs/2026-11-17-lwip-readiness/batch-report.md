# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 331. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::ioctl | lwIP ioctl toggles socket nonblocking mode | FAIL | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl rejects FIONREAD and preserves output | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl toggles socket nonblocking mode | FAIL | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::ioctl | lwIP ioctl rejects FIONREAD and preserves output | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll clears empty read readiness | PASS | [log L495](runtime_probe/probe.log#L495), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll reports queued read readiness | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP readiness corresponds to queued datagram | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll clears empty read readiness | PASS | [log L495](runtime_probe/probe.log#L495), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP poll reports queued read readiness | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::poll | lwIP readiness corresponds to queued datagram | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select clears empty read readiness | PASS | [log L496](runtime_probe/probe.log#L496), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select reports queued read readiness | PASS | [log L498](runtime_probe/probe.log#L498), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP readiness corresponds to queued datagram | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select clears empty read readiness | PASS | [log L496](runtime_probe/probe.log#L496), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP select reports queued read readiness | PASS | [log L498](runtime_probe/probe.log#L498), [page-020](runtime_probe/pages/page-020.png) |
| lwip-sys::select | lwIP readiness corresponds to queued datagram | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- lwip-sys::ioctl — lwIP ioctl toggles socket nonblocking mode (FAIL)
- lwip-sys::ioctl — lwIP ioctl toggles socket nonblocking mode (FAIL)

## Coverage limits

- Dolphin IPv4 UDP loopback, one readable descriptor and FIONBIO/FIONREAD only. Multiple descriptors, timeout edge cases, other commands and physical Wii remain unverified.
