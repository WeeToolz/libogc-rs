# GX FIFO read-enable ordering — Linux Dolphin 2609b

Compare C `GX_SetCPUFifo` and `GX_SetGPFifo` with Rust. CPU FIFO
assignment must not enable CP reads when the GP FIFO is detached. Drain
the active FIFO, save both snapshots, detach GP, set CPU, then restore GP.
Inspect CP control bits for read enable and link at each step. Restore both
FIFOs before logging or rendering. The baseline fails the CPU-only check
(27/28). The corrected code stops GP reads before programming the GP FIFO,
enables them only after installing it, and masks PPC interrupts during
both setters. The same regression passes 28/28 on stock Dolphin.

This establishes the tested control-bit sequence only. FIFO watermark
interrupt/reset ordering, overflow and underflow behavior, and broader
GX behavior still require review. Raw evidence is archived privately.

Private evidence SHA-256: `0217cf16bb431587764f33b60893e89c1dd87de4b37fd5deeb65f5454fe455bc`.
