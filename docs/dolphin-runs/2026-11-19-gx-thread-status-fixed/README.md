# GX thread and status checks in Dolphin

Dolphin Metal passed 366 video probe checks. The new checks compare the GX
thread setter with the current LWP thread and its previous value. They also
check an idle overflow counter and compare the five status outputs with a
stable command processor register. The [batch report](batch-report.md)
lists the five APIs and the saved results.

The test covers one thread and an idle FIFO. A nonzero overflow, thread
handoff under contention, active status transitions, and physical Wii
behavior remain open. The runner requested XFB copies to RAM. A temporary
launcher changed the recorded OpenGL backend argument to Metal.

An initial build failed because the probe referenced a crate that it did
not depend on. The passing build uses the existing LWP wrapper.
