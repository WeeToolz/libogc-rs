# Sysbase default device evidence recheck

This recheck confirms the two `setDefaultDevice` checks against the final
source and documentation snapshot. Dolphin passed 758 checks with no failures.
It adds no inventory entries; the batch was recorded in
[`v3`](../2026-11-21-ogc-sysbase-default-device-v3/README.md).

The test keeps its registered device in static storage until the DOL stops.
It checks Dolphin behavior only, not physical Wii behavior.

See the [batch report](batch-report.md), [probe log](runtime_probe/probe.log),
and [suite metadata](suite.json).
