# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 319. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::listen | Raw TCP listener binds and listens | PASS | [log L525](runtime_probe/probe.log#L525), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::listen | Raw TCP client connects | PASS | [log L528](runtime_probe/probe.log#L528), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::listen | Raw TCP accept reports loopback peer | PASS | [log L530](runtime_probe/probe.log#L530), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::listen | Raw TCP listener binds and listens | PASS | [log L525](runtime_probe/probe.log#L525), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::listen | Raw TCP client connects | PASS | [log L528](runtime_probe/probe.log#L528), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::listen | Raw TCP accept reports loopback peer | PASS | [log L530](runtime_probe/probe.log#L530), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | Raw TCP accept reports loopback peer | PASS | [log L530](runtime_probe/probe.log#L530), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | lwIP accepted peer matches client | PASS | [log L534](runtime_probe/probe.log#L534), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | Raw TCP accepted socket receives bytes | PASS | [log L535](runtime_probe/probe.log#L535), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | Raw TCP accept reports loopback peer | PASS | [log L530](runtime_probe/probe.log#L530), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | lwIP accepted peer matches client | PASS | [log L534](runtime_probe/probe.log#L534), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::accept | Raw TCP accepted socket receives bytes | PASS | [log L535](runtime_probe/probe.log#L535), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_peer_name | lwIP client peer matches listener | PASS | [log L532](runtime_probe/probe.log#L532), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_peer_name | lwIP accepted peer matches client | PASS | [log L534](runtime_probe/probe.log#L534), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_peer_name | lwIP client peer matches listener | PASS | [log L532](runtime_probe/probe.log#L532), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_peer_name | lwIP accepted peer matches client | PASS | [log L534](runtime_probe/probe.log#L534), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_socket_name | lwIP listener local address matches bind | PASS | [log L531](runtime_probe/probe.log#L531), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_socket_name | lwIP client local address matches accepted peer | PASS | [log L533](runtime_probe/probe.log#L533), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_socket_name | lwIP listener local address matches bind | PASS | [log L531](runtime_probe/probe.log#L531), [page-022](runtime_probe/pages/page-022.png) |
| lwip-sys::get_socket_name | lwIP client local address matches accepted peer | PASS | [log L533](runtime_probe/probe.log#L533), [page-022](runtime_probe/pages/page-022.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 loopback TCP, one listener and client. Other address families, error paths, IOS versions, and physical Wii behavior remain unverified.
