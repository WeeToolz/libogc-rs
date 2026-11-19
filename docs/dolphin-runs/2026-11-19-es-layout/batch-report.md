# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 26. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::certificate_size | ES certificate sizes match public layouts | PASS | [log L76](general_probe/probe.log#L76), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::is_valid_cert | ES certificate types accept only supported values | PASS | [log L77](general_probe/probe.log#L77), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::is_valid_signature | ES signature types accept only supported values | PASS | [log L75](general_probe/probe.log#L75), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::signature_size | ES signature sizes match public layouts | PASS | [log L74](general_probe/probe.log#L74), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::signed_cert_size | ES signed certificate sizes combine signature and certificate | PASS | [log L78](general_probe/probe.log#L78), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::signed_tik_size | ES signed ticket sizes include signature | PASS | [log L79](general_probe/probe.log#L79), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::signed_tmd_size | ES signed title metadata adds signature size | PASS | [log L81](general_probe/probe.log#L81), [page-002](general_probe/pages/page-002.png) |
| ogc-es-sys::tmd_size | ES title metadata sizes match public layouts | PASS | [log L80](general_probe/probe.log#L80), [page-002](general_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Only public size and type tables were tested. IOS ticket, title, signing and certificate operations remain unverified.
