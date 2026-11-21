# Bluetooth PCB lifecycle

The probe allocated a PCB, checked its initial fields, wrote and read its
callback argument, checked callback setters, freed the PCB, and confirmed
that a slot could be allocated again. Dolphin passed all 639 runtime checks.
The inventory fell from 346 to 341 unverified entries.

This does not verify a Bluetooth connection. See the [batch report](batch-report.md),
[suite result](suite.json), and [completeness audit](completeness-audit.json).
