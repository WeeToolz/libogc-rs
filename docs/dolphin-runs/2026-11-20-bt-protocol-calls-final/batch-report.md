# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 569. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::search | Bluetooth disabled protocol calls reject work and count malformed ACL | PASS | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::stop_search | Bluetooth disabled protocol calls reject work and count malformed ACL | PASS | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::send_report | Bluetooth disabled protocol calls reject work and count malformed ACL | PASS | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::tick | Bluetooth disabled protocol calls reject work and count malformed ACL | PASS | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |
| ogc-bt-sync::receive_acl | Bluetooth disabled protocol calls reject work and count malformed ACL | PASS | [log L215](runtime_probe/probe.log#L215), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- This test uses a disabled protocol host and one malformed ACL frame. It does not test adapter I/O, connected links, valid ACL traffic, or physical hardware.
