# System framebuffer null mode

The probe passed a null mode to framebuffer allocation. The function returned
a null pointer, and the saved snapshot reported the null mode and zero bytes.
Dolphin passed all 647 runtime checks. The inventory fell from 335 to 334
unverified entries.

Allocation for a valid mode remains untested. See the [batch report](batch-report.md),
[suite result](suite.json), and [completeness audit](completeness-audit.json).
