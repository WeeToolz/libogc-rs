# Wii GX abort while a producer is suspended — Linux Dolphin 2609b

C reference: `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Local Rust baseline: `89a8e0de`. Stock graphical Dolphin 2609b,
OpenGL, private stock-menu NAND. No Dolphin source changes.

## Source comparison

C `libogc/gx.c` `__GXOverflowHandler` suspends the GX producer and
changes the watermark mask to low-only. `__GXUnderflowHandler` balances
that suspension and restores the high-only mask. `__GX_CleanGPFifo`
(lines 340–384) discards pending work, restores high-only interrupts for
a linked FIFO, and clears the breakpoint, but does not release the
suspension or reset `_gxoverflowsuspend`. Rust's previous abort path had
the same gap. The low watermark event that would normally release the
producer is no longer enabled after abort.

The Rust correction records the exact LWP whose suspension the overflow
handler acquired successfully. Normal underflow and abort share the
release operation. When abort starts FIFO cleanup with a suspension
pending, it keeps IRQs and watermark masks disabled through the Wii
revision command flush, restores the linked high-only mask, then releases
one GX-owned suspension and restores IRQ state. It does not resume the
current GX thread designation indiscriminately. This is a documented
resource-management difference from C, not a claim that C resumes a
producer during abort. The ordinary unsuspended abort path is unchanged.

## Regression method and results

The shared pressure probe uses a separate aligned 64 KiB FIFO with
64-byte high and 32-byte low watermarks. A real GP breakpoint holds
processing. A bounded LWP producer submits up to 512 bytes of NOPs.
The controller observes one delivered breakpoint, one overflow, a
suspended producer that has not completed, and CP control 0x1B. It calls
`GX_AbortFrame` instead of releasing the breakpoint. A passing test
requires the producer to finish without manual resume, join, and allow
fresh PE completion and FIFO drain. Manual resume is failure cleanup only.

The first baseline attempt was interrupted during capture. The repeat of
the same DOL passed 54/55, with only automatic resume after abort failing.
With the production correction, the same probe passed 55/55. The extended
final probe also repeats the ordinary high/low watermark test after the
abort, and passed 59/59, with six valid captured pages. CP control returned
to 0x15 and the next producer suspended and recovered normally.

Host GX tests passed 5/5 and xtask tests passed 164/164. The six-package
Wii hardware group compiled, and the 3,419-entry inventory is current.
The private archive includes the exact three DOLs, raw logs, transport,
reports and captures, excluding all user profiles and NAND files. Its
SHA-256 and the individual DOL hashes are recorded in `summary.json`.

Scope: linked Wii FIFO abort with a genuinely overflow-suspended producer,
plus subsequent ordinary overflow recovery. The test does not independently
exercise a second application-owned suspension, change of designated
producer while suspended, detached FIFO abort, wraparound, pending PE
callbacks, GameCube runtime or physical Wii CP clearing and PE timing.
The broad video and runtime suites were not rerun in this scoped pass.
GX remains `needs_re_review`.
