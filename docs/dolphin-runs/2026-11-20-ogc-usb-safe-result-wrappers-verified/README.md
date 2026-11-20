# USB wrapper errors for invalid handles

Seven public `ogc-usb` queries returned `Err` for handle `-1`. The probe
checks device information, USB and HID descriptors, report descriptor size
and data, and ASCII strings. Dolphin 2609a passed all 607 runtime checks.
The inventory fell from 381 to 374 unverified entries.

Successful queries on an open USB device still need verification. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
