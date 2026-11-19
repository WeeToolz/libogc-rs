# Dolphin evidence report

Probe `runtime_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_gethostbyname | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497) |
| ogc-network-sys::net_gethostbyname | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498) |
| ogc-network-sys::net_gethostbyname | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499) |
| ogc-network-sys::net_gethostbyname | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497) |
| ogc-network-sys::net_gethostbyname | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498) |
| ogc-network-sys::net_gethostbyname | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499) |
| ogc-network::get_host_by_name | Network resolver rejects null and empty names | PASS | [log L497](runtime_probe/probe.log#L497) |
| ogc-network::get_host_by_name | Network resolver returns numeric loopback address | PASS | [log L498](runtime_probe/probe.log#L498) |
| ogc-network::get_host_by_name | Network resolver wrapper returns numeric loopback | PASS | [log L499](runtime_probe/probe.log#L499) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Numeric IPv4 loopback and invalid empty input in Dolphin. External DNS, aliases, multiple addresses, malformed IOS replies, concurrency and physical Wii remain unverified.
