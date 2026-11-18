# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 346. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_gethostip | Network DHCP address matches IOS host address | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_gethostip | Network DHCP address matches IOS host address | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_config | Network DHCP text address matches binary address | PASS | [log L495](runtime_probe/probe.log#L495), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network static configuration rejects without writes | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network DHCP address matches IOS host address | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network wrapper DHCP addresses match IOS | PASS | [log L496](runtime_probe/probe.log#L496), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network static configuration rejects without writes | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network DHCP address matches IOS host address | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::if_configex | Network wrapper DHCP addresses match IOS | PASS | [log L496](runtime_probe/probe.log#L496), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::if_configex | Network static configuration rejects without writes | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::if_configex | Network DHCP address matches IOS host address | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network::if_configex | Network wrapper DHCP addresses match IOS | PASS | [log L496](runtime_probe/probe.log#L496), [page-020](runtime_probe/pages/page-020.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin private profile using its DHCP configuration; no physical Wii or static configuration support is established. Gateway and lease changes are not tested.
