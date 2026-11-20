# Tuxedo condition wake in Dolphin

Dolphin passed 238 LWP probe checks. Two workers wait on the same condition.
Signal wakes one worker, then broadcast wakes the remaining worker. Each
worker checks that it owns the mutex after the wait and returns a result
that the main thread joins. The [batch report](batch-report.md) links the
three APIs to their checks.

The test covers one signal followed by one broadcast with two waiters.
Other queue ordering, multiple conditions, wake races, and physical Wii
timing remain open.
