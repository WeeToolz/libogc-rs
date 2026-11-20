# Bluetooth protocol guards in Dolphin

Dolphin passed 569 runtime probe checks. Five Bluetooth host APIs now have
direct evidence. The probe checks that a disabled host rejects search and
report work, that a timer tick leaves it disabled, and that a malformed ACL
frame increments the packet and malformed counters.

The [batch report](batch-report.md) links each API to its check. The test does
not cover adapter I/O, connected links, valid ACL traffic, or physical Wii
hardware.
