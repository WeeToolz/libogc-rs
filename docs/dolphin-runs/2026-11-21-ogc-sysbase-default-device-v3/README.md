# Sysbase default device in Dolphin

This batch checks `setDefaultDevice` with a registered test device. It verifies
that a relative path selects the registered device and that indexes `-1` and
`35` leave that selection unchanged. Dolphin passed 758 checks with no failures.
The inventory moved from 181 to 180 unverified APIs.

The test keeps its device in static storage until the DOL stops because the
default device table holds a raw pointer and the API has no reset operation.
This run checks the registered-device path in Dolphin. It does not prove
physical Wii behavior.

See the [batch report](batch-report.md), [probe log](runtime_probe/probe.log),
[suite metadata](suite.json), and [completeness audit](completeness-audit.log).
