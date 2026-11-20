# STM lifecycle and LED APIs

This Dolphin run checks four public `ogc-stm` APIs: event-handler replacement,
initialization without an event hook, and two LED mode calls. Dolphin passed all
eight probe checks. The inventory fell from 411 to 407 unverified entries.

A separate probe attempt hung during `stm::close()` after opening the STM
devices. This batch does not verify close, shutdown, or reboot APIs. Those
entries remain unverified.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
