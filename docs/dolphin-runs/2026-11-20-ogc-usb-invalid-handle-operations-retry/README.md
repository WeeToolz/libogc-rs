# USB operations with invalid handles

Five public `ogc-usb` operations returned errors for handle `-1`. The probe
checks suspend, resume, configuration, alternate interface, and clear halt.
Dolphin passed all 612 runtime checks. The inventory fell from 374 to 369
unverified entries.

Successful operations on an open USB device remain untested. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
