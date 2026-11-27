# Dolphin evidence report

Probe `network_probe` reported `probe_fail`.
Passed checks 80. Failed checks 4.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network::as_sockaddr_mut_ptr | socket address pointers address their original values | PASS | [log L75](network_probe/probe.log#L75), [page-002](network_probe/pages/page-002.png) |
| ogc-network::socket_address_len | socket address ABI lengths are sixteen bytes | PASS | [log L76](network_probe/probe.log#L76), [page-002](network_probe/pages/page-002.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — network SO_RCVTIMEO roundtrip
- Other failed check — network IP_TTL roundtrip
- Other failed check — network TCP_NODELAY roundtrip
- Other failed check — network TCP_KEEPALIVE roundtrip
