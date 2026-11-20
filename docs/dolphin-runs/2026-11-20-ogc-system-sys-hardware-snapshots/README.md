# System hardware snapshots and hooks

This Dolphin run checks seven `ogc-system-sys` APIs: reset snapshots, Hollywood
revision and reset-button queries, the HBC reload-stub query, the runtime FP
stage, stdio report mode, and reset-debug-hook replacement. Dolphin passed all
589 runtime probe checks. The inventory fell from 399 to 392 unverified
entries.

The probe does not trigger a system reset or launch a title. It does not prove
physical Wii hardware behavior. See the [batch report](batch-report.md),
[suite result](suite.json), and [completeness audit](completeness-audit.json).
