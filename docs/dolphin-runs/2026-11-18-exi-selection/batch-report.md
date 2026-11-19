# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 488. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::select | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_select | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::deselect | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_deselect | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::select_sd | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi::try_select_sd | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::select | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::deselect | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::select_sd | EXI selection tracks state and rejects duplicates | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin EXI channel 1 device 1 selection without transfer. Device presence, other channels, concurrent access and physical Wii remain unverified.
