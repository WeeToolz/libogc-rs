# Tuxedo queue wake and completed-thread reuse

Dolphin passed 246 LWP probe checks. Two workers block on a queue. The probe
checks one and all mask wakes, all value wakes, cancellation results, and
queue cleanup across four rounds. The [batch report](batch-report.md) maps
the four APIs to their checks.

The [diagnostic run](../2026-11-19-tuxedo-queue-diagnostic/batch-report.md)
failed before both reused workers reached the queue. Its log recorded LWP
handles for both previously joined Tuxedo objects. A completed-thread join
returned its saved value without joining the underlying LWP thread. The fix
joins a completed thread when its LWP handle is present. The fixed run records
zero old handles and reuses both objects in four rounds.

The checks use selected masks and tokens. Queue priority ordering,
cancellation races, and physical Wii scheduling remain open.
