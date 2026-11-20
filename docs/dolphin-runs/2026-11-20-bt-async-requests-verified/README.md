# Bluetooth asynchronous requests in Dolphin

Dolphin passed 578 runtime probe checks. The batch records direct evidence for
14 asynchronous transfer, command, and close APIs. The probe checks transfer
result copying, command status and result mapping, callback delivery, and
synchronous close result mapping with descriptor `-1`.

The [batch report](batch-report.md) links each API to its checks. The probe
does not test successful adapter transfers, valid Bluetooth frames, pairing,
or physical Wii hardware.
