# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 324. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::send_to | lwIP send_to and receive_from preserve datagram and source | FAIL | [log L517](runtime_probe/probe.log#L517), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send_to | lwIP send_to and receive_from preserve datagram and source | FAIL | [log L517](runtime_probe/probe.log#L517), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::receive_from | lwIP send_to and receive_from preserve datagram and source | FAIL | [log L517](runtime_probe/probe.log#L517), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::receive_from | lwIP send_to and receive_from preserve datagram and source | FAIL | [log L517](runtime_probe/probe.log#L517), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::send | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::recv | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::recv | lwIP send and recv preserve connected datagram | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::write | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::write | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::read | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::read | lwIP write and read preserve connected datagram | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- lwip-sys::send_to — lwIP send_to and receive_from preserve datagram and source (FAIL)
- lwip-sys::send_to — lwIP send_to and receive_from preserve datagram and source (FAIL)
- lwip-sys::receive_from — lwIP send_to and receive_from preserve datagram and source (FAIL)
- lwip-sys::receive_from — lwIP send_to and receive_from preserve datagram and source (FAIL)

## Coverage limits

- Dolphin IPv4 UDP loopback only; larger, truncated, zero-length and error cases, TCP stream transfers, other IOS versions and physical Wii remain unverified.
