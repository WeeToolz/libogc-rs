# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 335. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_init | Raw network initializes and reports status | PASS | [log L489](runtime_probe/probe.log#L489), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_init | Raw UDP sockets open | PASS | [log L490](runtime_probe/probe.log#L490), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_get_status | Raw network initializes and reports status | PASS | [log L489](runtime_probe/probe.log#L489), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_deinit | Raw network deinitializes | PASS | [log L537](runtime_probe/probe.log#L537), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_socket | Raw UDP sockets open | PASS | [log L490](runtime_probe/probe.log#L490), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_socket | Raw UDP sockets bind loopback | PASS | [log L491](runtime_probe/probe.log#L491), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_bind | Raw UDP sockets bind loopback | PASS | [log L491](runtime_probe/probe.log#L491), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_bind | Raw UDP names report distinct loopback ports | PASS | [log L492](runtime_probe/probe.log#L492), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_getsockname | Raw UDP names report distinct loopback ports | PASS | [log L492](runtime_probe/probe.log#L492), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_sendto | Raw UDP invalid descriptor rejects sendto | PASS | [log L493](runtime_probe/probe.log#L493), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_sendto | Raw UDP recvfrom preserves bytes and sender | PASS | [log L520](runtime_probe/probe.log#L520), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_recvfrom | Raw UDP invalid descriptor preserves recvfrom outputs | PASS | [log L494](runtime_probe/probe.log#L494), [page-020](runtime_probe/pages/page-020.png) |
| ogc-network-sys::net_recvfrom | Raw UDP recvfrom preserves bytes and sender | PASS | [log L520](runtime_probe/probe.log#L520), [page-021](runtime_probe/pages/page-021.png) |
| ogc-network-sys::net_connect | Raw UDP connect succeeds | PASS | [log L530](runtime_probe/probe.log#L530), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_connect | Raw UDP send and recv preserve connected datagram | PASS | [log L531](runtime_probe/probe.log#L531), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_send | Raw UDP send and recv preserve connected datagram | PASS | [log L531](runtime_probe/probe.log#L531), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_recv | Raw UDP send and recv preserve connected datagram | PASS | [log L531](runtime_probe/probe.log#L531), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_write | Raw UDP write and read preserve connected datagram | PASS | [log L532](runtime_probe/probe.log#L532), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_read | Raw UDP write and read preserve connected datagram | PASS | [log L532](runtime_probe/probe.log#L532), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_close | Raw UDP sender closes | PASS | [log L535](runtime_probe/probe.log#L535), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_close | Raw UDP receiver closes | PASS | [log L536](runtime_probe/probe.log#L536), [page-022](runtime_probe/pages/page-022.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPv4 loopback UDP with small datagrams and selected failure paths. Other transports, edge cases, IOS versions and physical Wii remain unverified.
