# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 487. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi-sys::lock | Raw EXI lock delivers queued callback once | PASS | [log L259](runtime_probe/probe.log#L259), [page-010](runtime_probe/pages/page-010.png) |
| ogc-exi-sys::unlock | Raw EXI lock delivers queued callback once | PASS | [log L259](runtime_probe/probe.log#L259), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin one EXI channel with one queued callback and duplicate request. Multiple waiting devices, transfers and physical Wii remain unverified.
