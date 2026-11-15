# GX and video Dolphin baseline

On Linux with Dolphin 2503, `video_probe` finished with **50 passes and 2
failures**. The GX texture descriptor, conversion, light, fog, FIFO metadata,
and canvas-dimension checks in the [check transcript](checks.txt) passed. The
run as a whole failed: `video control register readback` and `video scan mode
matches selected VI mode` failed. Six non-uniform pages were captured; pages
2–6 are retained in [pages](pages/), including the GX checks and final failure
summary. Page 1 contains startup diagnostic addresses and is omitted.

The DOL SHA-256 was
`3f8fce6d44ee8f603f4c757cebfa6376b126d1b562b732026068f2cb3f0fa74b`.
The run was made at repository revision `ab83d0837cce` using the existing
Dolphin-specific build, an imported Wii NAND copied to a private profile, a
virtual X11 display and software OpenGL. The command was `cargo xtask
dolphin-validate --no-build --probe video_probe --timeout-seconds 600`.
The selected video mode was 22; the probe measured a 16683 µs average retrace
and its timing check passed. The two VI failures remain unexplained and should
not be counted as passes.

An earlier [baseline](../2026-11-02-baseline/suite.json) on Dolphin 2609a
passed both VI checks, but used a different DOL and emulator version; it does
not isolate the cause of this run's failures. These results cover only the
named probe checks and do not verify all GX APIs or pixel rendering. Page PNGs
are framebuffer captures of the probe transcript, not independent proof of the
visual correctness of GX drawing.
