# PAD reset and recalibration in Dolphin

Dolphin passed 67 PAD state probe checks. The new cases exercise low-level
reset and recalibration with deterministic SI no-response fixtures. They
check the selected channel, consumed fixture, and clean completion. The probe
also checks that a post-initialization specification change has no effect
and that the recalibration flag is restored. The
[batch report](batch-report.md) lists the four APIs.

The SI responses are controlled fixtures. Real controller calibration,
transfer races, and physical Wii behavior remain open.
