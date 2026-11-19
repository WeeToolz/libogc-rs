# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 339. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_accept | Raw TCP wrapper relistens and accepts | PASS | [log L566](runtime_probe/probe.log#L566), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_accept | Raw TCP accepted peer matches second client | PASS | [log L567](runtime_probe/probe.log#L567), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_accept | Raw TCP second accepted socket closes | PASS | [log L568](runtime_probe/probe.log#L568), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_listen | Raw TCP wrapper relistens and accepts | PASS | [log L566](runtime_probe/probe.log#L566), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_listen | Raw TCP accepted peer matches second client | PASS | [log L567](runtime_probe/probe.log#L567), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_fcntl | Raw TCP nonblocking flags set and clear | PASS | [log L553](runtime_probe/probe.log#L553), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_getsockopt | Raw TCP reuse option is enabled | PASS | [log L550](runtime_probe/probe.log#L550), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_getsockopt | Raw TCP reuse option can be cleared | PASS | [log L552](runtime_probe/probe.log#L552), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_setsockopt | Raw TCP reuse option is enabled | PASS | [log L550](runtime_probe/probe.log#L550), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_setsockopt | Raw TCP reuse option can be cleared | PASS | [log L552](runtime_probe/probe.log#L552), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_poll | Raw TCP listener becomes ready | PASS | [log L557](runtime_probe/probe.log#L557), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_poll | Raw TCP accepted socket receives bytes | PASS | [log L563](runtime_probe/probe.log#L563), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_shutdown | Raw TCP write shutdown delivers EOF | PASS | [log L564](runtime_probe/probe.log#L564), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_select | Raw select clears empty UDP readiness | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_select | Raw select reports queued UDP readiness | PASS | [log L508](runtime_probe/probe.log#L508), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_select | lwIP readiness corresponds to queued datagram | PASS | [log L509](runtime_probe/probe.log#L509), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_select | Raw select clears empty UDP readiness | PASS | [log L505](runtime_probe/probe.log#L505), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_select | Raw select reports queued UDP readiness | PASS | [log L508](runtime_probe/probe.log#L508), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_select | lwIP readiness corresponds to queued datagram | PASS | [log L509](runtime_probe/probe.log#L509), [page-021](runtime_probe/pages/page-021.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 loopback: TCP listener, SO_REUSEADDR, shutdown and one UDP read descriptor. Other socket options, multi-descriptor selection, error paths and physical Wii remain unverified.
