# GX FIFO watermark interrupts — stock Linux Dolphin 2609b

The C reference `gx.c` at `7d4b8b3e7b84` enables the high-watermark
interrupt and disables the low-watermark interrupt when the CPU and GP
FIFO buffers are linked. `GX_SetCPUFifo` clears pending watermark status
before enabling the interrupt and linking; `GX_SetGPFifo` disables CP
reads and watermark interrupts before programming the GP registers,
selects the watermark mask and link, clears watermark status, then
re-enables reads. Unlinked assignments mask both watermark interrupts.

The regression drains the FIFO, saves valid CPU/GP snapshots, detaches GP,
assigns CPU, reattaches GP, assigns linked CPU again, detaches GP again,
and restores the GP snapshot. It does not log or render while detached.
It reads the actual CP control register after each step. The baseline
passes 30/32, failing the linked CPU and GP high-watermark enable checks.
The corrected production setters pass 32/32, including mask checks
before and after a linked assignment. Five baseline and four corrected
GDB framebuffer pages are valid. See [summary](summary.json),
[baseline checks](red-checks.log), and [corrected checks](green-checks.log).

This corrects the setter mask, reset writes and synchronization order;
it does not establish interrupt delivery or timing during real FIFO
overflow and underflow. The separate Rust MEM2 FIFO initialization and
write-gather redirect/restore paths require their own comparison. Physical
Wii interrupt behavior remains pending. Raw evidence is archived privately
without the NAND profile; the archive hash is in the summary.
