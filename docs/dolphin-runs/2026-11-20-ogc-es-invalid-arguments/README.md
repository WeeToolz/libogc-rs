# ES invalid argument guards

Four ES title launch and verification calls returned `ES_EINVAL` for null
ticket views, invalid ticket size, or null signed metadata. Dolphin passed all
622 runtime checks. The inventory fell from 362 to 358 unverified entries.

Valid title launch and signature verification remain untested. See the
[batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
