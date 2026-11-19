# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 490. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::imm | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_imm | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::imm_ex | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_imm_ex | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_sync | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::imm | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::imm_ex | EXI immediate writes complete and preserve source | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin EXI immediate writes on channel 1 device 1. Device response, reads, asynchronous callbacks, DMA and physical Wii remain unverified.
