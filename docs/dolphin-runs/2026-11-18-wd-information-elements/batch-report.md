# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 396. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wd::get_number_of_ies | WD counts complete nonempty elements | PASS | [log L170](runtime_probe/probe.log#L170), [page-006](runtime_probe/pages/page-006.png) |
| ogc-wd::number_of_ies | WD counts complete nonempty elements | PASS | [log L170](runtime_probe/probe.log#L170), [page-006](runtime_probe/pages/page-006.png) |
| ogc-wd::get_ie_length | WD element lengths and missing IDs | PASS | [log L171](runtime_probe/probe.log#L171), [page-006](runtime_probe/pages/page-006.png) |
| ogc-wd::ie_length | WD element lengths and missing IDs | PASS | [log L171](runtime_probe/probe.log#L171), [page-006](runtime_probe/pages/page-006.png) |
| ogc-wd::get_ie | WD copies payload and clears remaining buffer | PASS | [log L172](runtime_probe/probe.log#L172), [page-006](runtime_probe/pages/page-006.png) |
| ogc-wd::copy_ie | WD safe copy checks capacity | PASS | [log L173](runtime_probe/probe.log#L173), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin synthetic BSS descriptor and malformed lengths. Wireless scans and physical Wii remain unverified.
