# Wireless driver APIs

This Dolphin run directly checks 13 public `ogc-wd` APIs. It checks the driver
lock and unlock, driver init and deinit, six scan calls, two driver information
calls, and the RSSI bucket boundaries. Dolphin passed all 17 probe checks. The
inventory fell from 437 to 424 unverified entries.

The scan checks use Dolphin's emulated Wii wireless driver. They do not prove
behavior with a physical Wii wireless device or a real network.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
