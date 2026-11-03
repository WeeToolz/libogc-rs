# Initial DI wrapper comparison

This run preserves the first comparison attempt. The probe passed 118 checks
and failed one check: it required two consecutive `DI_GetError` calls to
return the same output. Both calls returned success, but their output values
differed. The result does not show a C wrapper mismatch. A follow-up
diagnostic run captured both values, and the final check verifies that both
calls succeed and update their outputs without requiring identical values.

- Dolphin: `Dolphin 2609a`
- DOL SHA-256: `1fa7b1a7912c25e3231dbc8d6e564ec60850eccbd635982ec5cf4735d21a0c09`
- Suite report: [`suite.json`](suite.json)
- Probe log: [`dvd_probe/probe.log`](dvd_probe/probe.log)
- Follow-up: [final passing run](../2026-11-03-di-wrapper-parity-final/README.md)
