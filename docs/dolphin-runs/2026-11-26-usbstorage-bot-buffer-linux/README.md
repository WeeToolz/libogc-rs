# USB BOT buffer ownership and CSW validation — Linux Dolphin 2609b

C reference: `7d4b8b3e7b84`. Rust base: `e3727008`.

The production StartStop entry point is called with a controlled pending bulk
state, a valid mutex, and a canary transfer buffer. The first run returns the
expected timeout, but modifies the canary and command tag (64 passed, 1 failed).
Reject pending bulk ownership before writing CBW, CSW, or data buffers. The same
probe then passes 65/65. This guard is a Rust safety improvement, not a claim
that the C implementation safely handles an outstanding DMA transfer.

C `__read_csw` checks the signature before copying status and residue, then
checks the tag. Both Rust CSW paths now share a decoder with that order. A host
regression failed before the order correction, then all 22 lifecycle tests
passed. Three target checks use controlled CSW bytes, not live USB traffic.
The final capacity probe passed 68/68 with six valid framebuffer pages. The
red, green and final check extracts and run hashes are in this directory.
Host xtask tests passed 163/163. Storage-group compilation passed (17 crates).

Stock Dolphin uses the private stock-menu NAND fixture. Raw results are retained
in a private archive outside Git. No console identifiers or NAND data are
included here. There is no USB passthrough device in this environment, so live
USB BOT transfers, late callbacks and physical DMA remain unverified. Neither
this probe nor host fixtures establish full package parity.

Private raw evidence SHA-256: `d20b531fa7f827ad826c3570b7a97f0a7a7d71209210026c66f15850f396f093`.
