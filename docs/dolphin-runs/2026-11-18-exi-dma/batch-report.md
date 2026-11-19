# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 491. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::dma | EXI DMA writes complete and preserve source | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_dma | EXI DMA writes complete and preserve source | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::dma | EXI DMA writes complete and preserve source | PASS | [log L263](runtime_probe/probe.log#L263), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin aligned EXI DMA writes on channel 1 device 1. Device response, DMA reads, asynchronous callbacks and physical Wii remain unverified.
