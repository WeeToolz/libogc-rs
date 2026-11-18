# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 354. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_get_mac_address | Network MAC matches independent IOS response | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_get_mac_address | Network MAC rejects null output | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_get_mac_address | Network MAC matches independent IOS response | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_get_mac_address | Network MAC rejects null output | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin private NAND and IOS network manager response. Physical Wii, alternate interface states and IOS failure paths are unverified.
