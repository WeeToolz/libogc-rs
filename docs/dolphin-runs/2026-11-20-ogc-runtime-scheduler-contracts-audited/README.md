# Shared scheduler contracts

This Dolphin run checks eight `ogc-runtime` scheduler APIs through the LWP
driver adapters. It covers thread ID reads, the monotonic clock, timer token
validation, deferred callback submission, and wake token validation. Dolphin
passed all 258 LWP probe checks. The inventory fell from 407 to 399 unverified
entries.

This run does not prove physical Wii timing. See the [batch report](batch-report.md),
[suite result](suite.json), and [completeness audit](completeness-audit.json).
