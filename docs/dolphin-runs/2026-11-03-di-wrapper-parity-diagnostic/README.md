# DI wrapper comparison diagnostic run

This rerun added DI error values to the probe log. It passed 118 checks and
failed the same repeated-value comparison. The Rust call returned success
with output `0x01023a00`. The C wrapper call also returned success with output
`0x01000000`. The final probe checks each call for success and a changed
output sentinel. It does not assume that consecutive drive error queries
return identical values.

- Dolphin: `Dolphin 2609a`
- DOL SHA-256: `6c72c7067684f6ece685cd90a84b3583a39f5420334f7bf27b561cc9c09835ed`
- Suite report: [`suite.json`](suite.json)
- Probe log: [`dvd_probe/probe.log`](dvd_probe/probe.log)
- Follow-up: [final passing run](../2026-11-03-di-wrapper-parity-final/README.md)
