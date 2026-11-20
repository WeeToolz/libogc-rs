# Bluetooth synchronous transport in Dolphin

Dolphin passed 573 runtime probe checks. The batch records direct evidence for
23 synchronous Bluetooth transport, HCI, and L2CAP methods. A descriptor with
value `-1` makes these calls return typed IPC errors. Oversized HCI parameters
and PIN values return a length error before IOS receives a request.

The [batch report](batch-report.md) links each method to its check. The probe
does not test valid adapter traffic, asynchronous completion, adapter opening,
or physical Wii hardware.
