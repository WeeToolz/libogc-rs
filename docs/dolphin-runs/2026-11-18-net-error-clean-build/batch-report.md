# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 342. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::_net_convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion maps IOS errors | PASS | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion offsets unknown values | PASS | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion maps IOS errors | PASS | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network-sys::_net_convert_error | Network error conversion offsets unknown values | PASS | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion preserves nonnegative results | PASS | [log L74](runtime_probe/probe.log#L74), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion maps IOS errors | PASS | [log L75](runtime_probe/probe.log#L75), [page-002](runtime_probe/pages/page-002.png) |
| ogc-network::convert_error | Network error conversion offsets unknown values | PASS | [log L76](runtime_probe/probe.log#L76), [page-002](runtime_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Fixed representative newlib errno mappings and unknown values on Dolphin PowerPC; physical Wii and all error values remain unverified.
