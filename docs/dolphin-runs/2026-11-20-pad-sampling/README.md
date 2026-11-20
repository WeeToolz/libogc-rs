# PAD sampling callback in Dolphin

Dolphin passed 35 input probe checks. The sampling check registers two
callbacks in sequence, observes each callback, verifies the returned prior
callback, removes the callback, and checks that delivery stops. It restores
the original registration afterward. The [batch report](batch-report.md)
links the API to the check.

The test uses Dolphin SI polling with one scripted controller. Interrupt
contention and physical Wii behavior remain open.
