# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 545. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-si::set_sampling_rate | SI wrapper sets sampling rate one | PASS | [log L299](runtime_probe/probe.log#L299), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si::set_sampling_rate | SI sampling rate fixture restored | PASS | [log L303](runtime_probe/probe.log#L303), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si::refresh_sampling_rate | SI wrapper refresh restores selected sampling rate | PASS | [log L300](runtime_probe/probe.log#L300), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si::refresh_sampling_rate | SI sampling rate fixture restored | PASS | [log L303](runtime_probe/probe.log#L303), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si-sys::set_sampling_rate | SI raw clamps and sets sampling rate | PASS | [log L301](runtime_probe/probe.log#L301), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si-sys::set_sampling_rate | SI sampling rate fixture restored | PASS | [log L303](runtime_probe/probe.log#L303), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si-sys::refresh_sampling_rate | SI raw refresh restores selected sampling rate | PASS | [log L302](runtime_probe/probe.log#L302), [page-012](runtime_probe/pages/page-012.png) |
| ogc-si-sys::refresh_sampling_rate | SI sampling rate fixture restored | PASS | [log L303](runtime_probe/probe.log#L303), [page-012](runtime_probe/pages/page-012.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin current video mode, rate one and clamped rate eleven. Controller polling timing, other video modes, and physical Wii remain unverified.
