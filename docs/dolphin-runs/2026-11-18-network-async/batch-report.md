# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 352. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-network-sys::net_init_async | Raw network asynchronous initialization completes | PASS | [log L551](runtime_probe/probe.log#L551), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_init_async | Network wrapper asynchronous initialization completes | PASS | [log L553](runtime_probe/probe.log#L553), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network-sys::net_init_async | Raw network asynchronous initialization completes | PASS | [log L551](runtime_probe/probe.log#L551), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network-sys::net_init_async | Network wrapper asynchronous initialization completes | PASS | [log L553](runtime_probe/probe.log#L553), [page-023](runtime_probe/pages/page-023.png) |
| ogc-network::init_async | Raw network asynchronous initialization completes | PASS | [log L551](runtime_probe/probe.log#L551), [page-022](runtime_probe/pages/page-022.png) |
| ogc-network::init_async | Network wrapper asynchronous initialization completes | PASS | [log L553](runtime_probe/probe.log#L553), [page-023](runtime_probe/pages/page-023.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin success path and callback completion with a private NAND. Callback failure, abort, simultaneous requests and physical Wii remain unverified.
