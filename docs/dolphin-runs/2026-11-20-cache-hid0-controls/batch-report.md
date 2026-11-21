# Dolphin evidence report

Probe `bus_probe` reported `pass`.
Passed checks 45. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-cache::dc_disable | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_disable | dc_disable clears only DCE | PASS | [log L87](bus_probe/probe.log#L87), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_enable | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_enable | dc_enable sets only DCE | PASS | [log L88](bus_probe/probe.log#L88), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_freeze | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_freeze | dc_freeze sets only DLOCK | PASS | [log L89](bus_probe/probe.log#L89), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_unfreeze | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::dc_unfreeze | dc_unfreeze clears only DLOCK | PASS | [log L90](bus_probe/probe.log#L90), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_disable | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_disable | ic_disable clears only ICE | PASS | [log L91](bus_probe/probe.log#L91), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_enable | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_enable | ic_enable sets only ICE | PASS | [log L92](bus_probe/probe.log#L92), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_freeze | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_freeze | ic_freeze sets only ILOCK | PASS | [log L93](bus_probe/probe.log#L93), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_unfreeze | HID0 cache baseline and restoration | PASS | [log L86](bus_probe/probe.log#L86), [page-003](bus_probe/pages/page-003.png) |
| ogc-cache::ic_unfreeze | ic_unfreeze clears only ILOCK | PASS | [log L94](bus_probe/probe.log#L94), [page-003](bus_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- This run checks HID0 bit transitions and restoration in Dolphin. It does not measure cache contents, cache timing, or physical Wii behavior.
