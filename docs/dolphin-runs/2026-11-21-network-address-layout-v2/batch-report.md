# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 764. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network::as_sockaddr_mut_ptr | socket address pointers address their original values | PASS | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::socket_address_len | socket address ABI lengths are sixteen bytes | PASS | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
