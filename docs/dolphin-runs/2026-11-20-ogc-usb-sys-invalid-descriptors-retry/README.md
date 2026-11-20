# USB system invalid-pointer guards

This Dolphin run checks 11 public `ogc-usb-sys` APIs with rejected null
pointers and buffers. The checks cover open and close outputs, descriptor
queries, device listing, and string reads. Dolphin passed all 600 runtime probe
checks. The inventory fell from 392 to 381 unverified entries.

The checks do not open a USB device or prove successful USB traffic. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
