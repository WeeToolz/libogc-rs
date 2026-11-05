# GX MEM2 FIFO Dolphin evidence

Dolphin 2503 on Linux ran `video_probe` with **62 passes and zero failures**;
all six captured pages were valid. The [FIFO screenshot](fifo-checks.png)
shows the new checks, and the [transcript](checks.txt) lists the complete
probe results. DOL SHA-256:
`a8e84fde32763d989a7522e722c27f7abec5290a0634aa0f9d64d42b82fca577`.
The source was based on `5ef7d633b0cc` with the probe change in the commit
containing this record.

Before starting GX, the probe calls `GxFifo::initialize_mem2` with zero and
an unaligned size, verifies `InvalidSize`, and confirms that neither advances
the MEM2 arena. It then calls `GxFifo::initialize_default_mem2`, which invokes
`initialize_mem2` with the default size. Using `GxFifo::address` and
`GxFifo::size`, the probe checks the aligned arena start, the 256 KiB size,
and the exact advancement of the arena. Subsequent canvas and video checks
also pass with this FIFO active. The probe does not separately exercise a
successful non-default FIFO size, exhaustion, interrupt conflicts, or cleanup.

Build with `cargo xtask build-dolphin-probes`, then run `cargo xtask
dolphin-validate --no-build --probe video_probe --timeout-seconds 600` with
Dolphin, a display, and a configured NAND. This run used a private NAND copy
and software OpenGL. The screenshot is probe text, not a rendered-pixel
comparison. Results cover the stated checks in Dolphin only.
