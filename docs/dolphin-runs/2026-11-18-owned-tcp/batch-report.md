# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 367. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::connect | Owned TCP connect and accept match peer addresses | PASS | [log L566](runtime_probe/probe.log#L566), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::connect | Owned TCP connection transfers bytes | PASS | [log L567](runtime_probe/probe.log#L567), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::connect | Owned TCP closes all sockets | PASS | [log L570](runtime_probe/probe.log#L570), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::accept | Owned TCP connect and accept match peer addresses | PASS | [log L566](runtime_probe/probe.log#L566), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::accept | Owned TCP connection transfers bytes | PASS | [log L567](runtime_probe/probe.log#L567), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::accept | Owned TCP closes all sockets | PASS | [log L570](runtime_probe/probe.log#L570), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::shutdown | Owned TCP shutdown delivers EOF | PASS | [log L568](runtime_probe/probe.log#L568), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::shutdown | Owned TCP rejects invalid shutdown mode | PASS | [log L569](runtime_probe/probe.log#L569), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::lookup | Owned resolver returns loopback and rejects invalid names | PASS | [log L565](runtime_probe/probe.log#L565), [page-023](runtime_probe/pages/page-023.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin private profile, numeric IPv4 loopback and one TCP connection. DNS names, concurrent connections, transport failure injection and physical Wii remain unverified.
