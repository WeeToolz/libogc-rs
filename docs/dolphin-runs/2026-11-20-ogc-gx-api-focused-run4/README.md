# GX public APIs

This Dolphin run directly checks 13 public `ogc-gx` APIs. The focused probe
initializes the display and GX FIFO, then checks FIFO setup and restoration,
write-gather redirection, breakpoint status, display-list state, metric reads,
and invalid TPL inputs. Dolphin passed all 22 probe checks. The inventory fell
from 424 to 411 unverified entries.

The metric check follows the C implementation: `GX_ReadGPMetric` writes counter
zero and leaves counter one unsupported. This run does not cover the five EFB
failures in the broader video probe or prove physical Wii behavior.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
