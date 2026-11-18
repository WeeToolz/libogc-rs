# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 361. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::initialize | Owned UDP service initializes | PASS | [log L557](runtime_probe/probe.log#L557), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::initialize | Owned UDP binds distinct loopback sockets | PASS | [log L558](runtime_probe/probe.log#L558), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::snapshot | Owned UDP binds distinct loopback sockets | PASS | [log L558](runtime_probe/probe.log#L558), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::snapshot | Owned UDP releases leases and closes sockets | PASS | [log L563](runtime_probe/probe.log#L563), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::bind | Owned UDP binds distinct loopback sockets | PASS | [log L558](runtime_probe/probe.log#L558), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::address | Owned UDP binds distinct loopback sockets | PASS | [log L558](runtime_probe/probe.log#L558), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::address | Owned UDP addressed transfer preserves payload and source | PASS | [log L561](runtime_probe/probe.log#L561), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::send | Owned UDP addressed transfer preserves payload and source | PASS | [log L561](runtime_probe/probe.log#L561), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::send | Owned UDP connected transfer preserves payload | PASS | [log L562](runtime_probe/probe.log#L562), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::receive | Owned UDP nonblocking empty receive preserves bytes | PASS | [log L560](runtime_probe/probe.log#L560), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::receive | Owned UDP addressed transfer preserves payload and source | PASS | [log L561](runtime_probe/probe.log#L561), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::receive | Owned UDP connected transfer preserves payload | PASS | [log L562](runtime_probe/probe.log#L562), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::udp_connect | Owned UDP connected transfer preserves payload | PASS | [log L562](runtime_probe/probe.log#L562), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::set_nonblocking | Owned UDP nonblocking empty receive preserves bytes | PASS | [log L560](runtime_probe/probe.log#L560), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::set_timeout | Owned UDP timeout state validates values | PASS | [log L559](runtime_probe/probe.log#L559), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::timeout | Owned UDP timeout state validates values | PASS | [log L559](runtime_probe/probe.log#L559), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::release | Owned UDP releases leases and closes sockets | PASS | [log L563](runtime_probe/probe.log#L563), [page-023](runtime_probe/pages/page-023.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 loopback UDP, single-threaded success path, empty nonblocking receive, timeout state and cleanup. Concurrent sockets, failure injection, external network and physical Wii remain unverified.
