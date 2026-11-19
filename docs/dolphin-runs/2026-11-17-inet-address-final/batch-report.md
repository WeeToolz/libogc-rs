# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 310. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::inet_addr | inet_addr dotted decimal | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr abbreviated hexadecimal | PASS | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr rejects overflow | PASS | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::inet_addr | inet_addr rejects invalid octal | PASS | [log L77](runtime_probe/probe.log#L77), [page-002](runtime_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin PowerPC parser cases only; other address forms and physical Wii remain unverified.
