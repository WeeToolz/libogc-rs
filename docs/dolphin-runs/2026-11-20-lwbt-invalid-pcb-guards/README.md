# Bluetooth invalid PCB guards

Seven raw `lwbt-sys` connection and send APIs returned `ERR_ARG` for a null
PCB. Dolphin passed all 629 runtime probe checks. The inventory fell from
358 to 351 unverified entries.

Successful Bluetooth transport still needs device testing. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
