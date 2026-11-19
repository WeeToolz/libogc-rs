# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 314. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwip-sys::get_socket_option | lwIP reuse option set and read | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::get_socket_option | lwIP reuse option clears | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::get_socket_option | lwIP zero-length option read preserves output | PASS | [log L520](runtime_probe/probe.log#L520), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::get_socket_option | lwIP reuse option set and read | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::get_socket_option | lwIP reuse option clears | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::get_socket_option | lwIP zero-length option read preserves output | PASS | [log L520](runtime_probe/probe.log#L520), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP reuse option set and read | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP reuse option clears | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP oversized option write rejected | PASS | [log L521](runtime_probe/probe.log#L521), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP reuse option set and read | PASS | [log L518](runtime_probe/probe.log#L518), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP reuse option clears | PASS | [log L519](runtime_probe/probe.log#L519), [page-021](runtime_probe/pages/page-021.png) |
| lwip-sys::set_socket_option | lwIP oversized option write rejected | PASS | [log L521](runtime_probe/probe.log#L521), [page-021](runtime_probe/pages/page-021.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 TCP socket with SO_REUSEADDR and local validation only. Other options, IOS versions, and physical Wii remain unverified.
- The separate network probe has four socket-option failures on unpatched Dolphin; see docs/dolphin-runs/2026-11-17-inet-address-network/batch-report.md and docs/dolphin-runs/2026-11-04-network-fixed/README.md.
