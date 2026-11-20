# USB sys transfer errors

This Dolphin run checks 14 direct `ogc-usb-sys` calls with file descriptor `-1`:
interrupt, bulk, control, and isochronous reads and writes, with synchronous and
asynchronous calls where the API provides them.

All calls returned the expected unavailable-device error. The probe also checked
that rejected calls did not change the aligned data buffers, packet sizes, or
callback count. Dolphin passed 49 checks. The inventory audit reduced the
unverified count from 528 to 512. It also accepted two earlier USB evidence
rows during the refreshed audit.

This run checks the unavailable-device error paths. It does not check successful
USB transfers or physical Wii behavior.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
