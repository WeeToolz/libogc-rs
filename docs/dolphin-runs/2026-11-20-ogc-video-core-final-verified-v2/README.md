# Video console and query APIs

This Dolphin run checks four public `ogc-video` APIs. It checks cursor reset
after console clear, rejection of invalid RGBA image sizes, the current scan
line query, and the next-field query. Dolphin passed all 582 runtime checks and
all four selected API checks. The inventory fell from 441 to 437 unverified
entries.

The RGBA test checks invalid image sizes. It does not check successful drawing.
This run also does not prove behavior on physical video hardware.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
