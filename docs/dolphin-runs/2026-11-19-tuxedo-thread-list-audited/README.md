# Tuxedo thread lifecycle in Dolphin

Dolphin passed 242 LWP probe checks. The new checks cover a suspended
worker's list membership, physical-ID lookup, initial stack snapshot,
priority change, nested suspension, resume, exit, and timed sleeps.
The [batch report](batch-report.md) lists the 15 API entries.

The test uses the main thread and one worker. Multi-thread list ordering,
priority contention, register preservation, and physical Wii timing remain
open. The final screenshot is black; the
[previous passing capture](../2026-11-19-tuxedo-thread-list-final/lwp_probe/pages/page-012.png)
shows the same checks. The first run expected a 32-byte aligned stack
pointer and failed. The corrected check accounts for the initial eight-byte
stack frame below the aligned stack top.
