# USB raw invalid-handle operations

Six raw `ogc-usb` operations returned errors for handle `-1`. The probe
checks suspend, resume, configuration, alternate interface, clear halt, and
configuration query. Dolphin passed all 645 runtime checks. The inventory
fell from 341 to 335 unverified entries.

Successful calls on an open USB device remain untested. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
