# Tuxedo mutexes and condition timeouts in Dolphin

Dolphin passed 235 LWP probe checks. The new checks cover mutex ownership,
recursive locking and release, and condition timeouts in ticks, nanoseconds,
microseconds, and milliseconds. The timeout checks measure each wait and
check that the mutex is owned after the calls. The
[batch report](batch-report.md) lists the 12 API entries and their evidence.

The checks use one thread and one-millisecond timeouts. Cross-thread mutex
contention, signaled condition waits, timeout races, and physical Wii
scheduling remain open.
