# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 350. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_gethostbyname | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver requires an active session | PASS | [log L550](runtime_probe/probe.log#L550), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_gethostbyname | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostbyname | Network resolver requires an active session | PASS | [log L550](runtime_probe/probe.log#L550), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network::get_host_by_name | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::get_host_by_name | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::get_host_by_name | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::get_host_by_name | Network resolver requires an active session | PASS | [log L550](runtime_probe/probe.log#L550), [page-022](runtime_probe/pages/page-022.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Numeric IPv4 loopback and invalid empty input in Dolphin. External DNS, aliases, multiple addresses, malformed IOS replies, concurrency and physical Wii remain unverified.
- Resolver session behavior checked before and after normal UDP socket use; concurrent deinitialization and multiple resolver callers are not tested.
