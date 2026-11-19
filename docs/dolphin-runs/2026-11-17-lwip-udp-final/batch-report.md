# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 325. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::send_to | lwIP send_to and receive_from preserve datagram and source | PASS | [log L514](runtime_probe/probe.log#L514), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send_to | lwIP send_to and receive_from preserve datagram and source | PASS | [log L514](runtime_probe/probe.log#L514), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::receive_from | lwIP send_to and receive_from preserve datagram and source | PASS | [log L514](runtime_probe/probe.log#L514), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::receive_from | lwIP send_to and receive_from preserve datagram and source | PASS | [log L514](runtime_probe/probe.log#L514), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::recv | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::recv | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::write | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::write | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::read | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::read | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 UDP loopback only; larger, truncated, zero-length and error cases, TCP stream transfers, other IOS versions and physical Wii remain unverified.
