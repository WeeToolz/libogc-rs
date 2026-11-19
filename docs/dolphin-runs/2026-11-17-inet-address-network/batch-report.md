# Dolphin evidence report

Probe `network_probe` reported `probe_fail`.
Passed checks 66. Failed checks 4.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::inet_addr | inet_addr dotted decimal | PASS | [log L74](network_probe/probe.log#L74), [page-002](network_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr abbreviated hexadecimal | PASS | [log L75](network_probe/probe.log#L75), [page-002](network_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr rejects overflow | PASS | [log L76](network_probe/probe.log#L76), [page-002](network_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr rejects invalid octal | PASS | [log L77](network_probe/probe.log#L77), [page-002](network_probe/pages/page-002.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — network SO_RCVTIMEO roundtrip
- Other failed check — network IP_TTL roundtrip
- Other failed check — network TCP_NODELAY roundtrip
- Other failed check — network TCP_KEEPALIVE roundtrip

## Coverage limits

- Dolphin PowerPC parser cases only; other address forms and physical Wii remain unverified.
