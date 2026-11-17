# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 280. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-bt-sync::new | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::new | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::len | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::len | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::is_empty | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::is_empty | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::is_full | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::is_full | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::active_generation | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::active_generation | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_connected | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_connected | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_report | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_report | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_disconnected | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::push_disconnected | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::pop | Bluetooth queue preserves connection report and disconnect order | PASS | [log L122](runtime_probe/probe.log#L122), [page-004](runtime_probe/pages/page-004.png) |
| ogc-bt-sync::pop | Bluetooth queue applies backpressure without losing state | PASS | [log L123](runtime_probe/probe.log#L123), [page-004](runtime_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Queue state and event data in Dolphin. Bluetooth adapter and physical controller behavior require separate evidence.
