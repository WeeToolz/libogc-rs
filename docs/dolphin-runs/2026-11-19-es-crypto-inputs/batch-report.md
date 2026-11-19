# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 57. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::get_device_cert | ES device certificate has ECC format and bounded output | PASS | [log L99](general_probe/probe.log#L99), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::encrypt | ES encrypt rejects invalid buffers without mutation | PASS | [log L100](general_probe/probe.log#L100), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::decrypt | ES decrypt rejects invalid buffers without mutation | PASS | [log L101](general_probe/probe.log#L101), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Certificate identity is specific to the imported Dolphin NAND. Encryption and decryption checks cover rejected input only; successful IOS crypto remains untested.
