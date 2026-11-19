# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 524. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ios-sys::get_version | IOS version matches low memory | PASS | [log L373](runtime_probe/probe.log#L373), [page-015](runtime_probe/pages/page-015.png) |
| ogc-ios-sys::get_revision | IOS revision matches low memory | PASS | [log L374](runtime_probe/probe.log#L374), [page-015](runtime_probe/pages/page-015.png) |
| ogc-ios-sys::get_revision_major | IOS revision major matches low memory | PASS | [log L375](runtime_probe/probe.log#L375), [page-015](runtime_probe/pages/page-015.png) |
| ogc-ios-sys::get_revision_minor | IOS revision minor matches low memory | PASS | [log L376](runtime_probe/probe.log#L376), [page-015](runtime_probe/pages/page-015.png) |
| ogc-ios-sys::get_preferred_version | IOS preferred version selects installed IOS 58 | PASS | [log L377](runtime_probe/probe.log#L377), [page-015](runtime_probe/pages/page-015.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin current IOS and a NAND fixture with installed IOS 58 only. Invalid version words, alternate title catalogs, and physical Wii remain unverified.
