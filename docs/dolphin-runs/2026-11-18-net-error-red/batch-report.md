# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 340. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::_net_convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion maps IOS errors | FAIL | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion offsets unknown values | FAIL | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion maps IOS errors | FAIL | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion offsets unknown values | FAIL | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion maps IOS errors | FAIL | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion offsets unknown values | FAIL | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-network-sys::_net_convert_error — Network error conversion maps IOS errors (FAIL)
- ogc-network-sys::_net_convert_error — Network error conversion offsets unknown values (FAIL)
- ogc-network-sys::_net_convert_error — Network error conversion maps IOS errors (FAIL)
- ogc-network-sys::_net_convert_error — Network error conversion offsets unknown values (FAIL)
- ogc-network::convert_error — Network error conversion maps IOS errors (FAIL)
- ogc-network::convert_error — Network error conversion offsets unknown values (FAIL)

## Coverage limits

- Fixed representative newlib errno mappings and unknown values on Dolphin PowerPC; physical Wii and all error values remain unverified.
