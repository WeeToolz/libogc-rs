# PAD low-level accessors in Dolphin

Dolphin passed 28 input probe checks. The probe compares nine low-level PAD
button and axis accessors with raw channel-zero samples during scripted
button and main-stick transitions. The [batch report](batch-report.md)
links the APIs to the checks.

Other channels, nonzero triggers and substick input, physical controllers,
and transition races remain open.
