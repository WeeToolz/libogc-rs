# System protection call snapshot

The probe sent an invalid channel to `sys_protect_range`. That path records
the arguments and returns before it accesses the protection registers. The
snapshot reported the same channel, address, size, and control value twice.
Dolphin passed all 614 runtime checks. The inventory fell from 369 to 367
unverified entries.

This check does not verify valid memory protection settings. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
