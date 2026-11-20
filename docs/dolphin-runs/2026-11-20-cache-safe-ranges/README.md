# Cache safe ranges

This Dolphin run checks six `ogc-cache` APIs:

- Data-cache invalidate, flush without sync, and store without sync on an
  aligned buffer.
- Locked-cache load and store with a zero byte count.
- Locked-cache wait at the maximum queue depth.

Dolphin passed 36 checks. The audit reduced the unverified count from 480 to
474. The run does not check cache mode changes, nonzero locked-cache transfers,
or physical hardware timing.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
