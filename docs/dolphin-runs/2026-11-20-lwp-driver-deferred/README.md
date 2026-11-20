# LWP driver deferred callbacks in Dolphin

Dolphin passed 250 LWP probe checks. The new checks exercise repeated driver
initialization, deferred callback submission, reservation cancellation, and
reserved and unreserved submissions from a timer interrupt. They check
callback execution and rejection of empty callbacks. The
[batch report](batch-report.md) lists the seven APIs.

Queue saturation, cancellation races, and physical Wii timing remain open.
