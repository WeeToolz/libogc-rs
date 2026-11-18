# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 471. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-sysbase::open_read | ISO9660 convenience open and seek work | PASS | [log L482](runtime_probe/probe.log#L482), [page-020](runtime_probe/pages/page-020.png), [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |
| ogc-sysbase::seek | ISO9660 convenience open and seek work | PASS | [log L482](runtime_probe/probe.log#L482), [page-020](runtime_probe/pages/page-020.png), [log L497](runtime_probe/probe.log#L497), [page-020](runtime_probe/pages/page-020.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin ISO9660 fixture, read and absolute seek. Other devices, seek modes, errors and physical Wii remain unverified.
