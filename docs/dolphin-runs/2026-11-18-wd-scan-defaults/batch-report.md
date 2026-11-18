# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 440. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wd::set_default_scan_parameters | WD defaults overwrite every scan parameter | PASS | [log L206](runtime_probe/probe.log#L206), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wd::default_scan_parameters | WD default scan constructor returns expected fields | PASS | [log L207](runtime_probe/probe.log#L207), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin scan parameter initialization. Wireless scanning, driver state and physical Wii remain unverified.
