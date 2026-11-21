# Bluetooth link-key pointer guards

Five raw `lwbt-sys` key APIs returned `ERR_ARG` for null addresses or an
invalid output buffer. Dolphin passed all 634 runtime checks. The inventory
fell from 351 to 346 unverified entries.

Valid stored-key access remains untested. See the [batch report](batch-report.md),
[suite result](suite.json), and [completeness audit](completeness-audit.json).
