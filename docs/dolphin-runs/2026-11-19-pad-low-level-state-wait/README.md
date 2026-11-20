# PAD low-level state in Dolphin

Dolphin passed 34 input probe checks. The new checks cover repeated
initialization and debug stages, trace hooks and event draining, invalid
motor input, idle synchronization, and scans and reads of the scripted
controller. The [batch report](batch-report.md) lists 11 API entries.

The [first run](../2026-11-19-pad-low-level-state/batch-report.md) failed
four checks after issuing back-to-back reads while SI was active. The passing
run waits for synchronization and a retrace between reads. Physical motor
behavior, reset races, and physical Wii operation remain open.
