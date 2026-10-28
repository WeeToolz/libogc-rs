# Input probe lifecycle

The input monitor collects up to 9,000 display frames. The Dolphin automation
run keeps its existing 600-frame observation window and assertions. Neither
mode remains in the sampling loop after its window ends.

Before the final summary, the probe requests Bluetooth shutdown and USB HID
shutdown. It continues real `ControllerManager::scan_at` calls so the async
Bluetooth shutdown and owned USB reads, writes, descriptors, and retirement
work can finish. It uses the system tick counter for a six-second deadline.
Every distinct controller scan or cleanup error is recorded. The final result
is incomplete if either transport does not report completion, a shutdown
request fails, any controller operation reports an error, or the deadline
expires.

Before the durable summary, the probe records a `collection_candidate` only
when a device was observed, all captured events and controller operations
succeeded, cleanup completed, and the logger remained healthy. A run with no
detected device records `collection_candidate=INCOMPLETE reason=no_device_observed`.
The summary records `hardware_verdict=PENDING`; its final PASS/FAIL follows
durable log synchronization. Finite host tests and DOL builds do not prove
hardware acceptance.
