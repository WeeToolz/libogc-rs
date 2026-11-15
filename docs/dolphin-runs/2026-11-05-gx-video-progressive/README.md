# GX and progressive video Dolphin follow-up

The rebuilt `video_probe` passed **52 checks, zero failures** in Dolphin 2503
on Linux with an imported Wii NAND, software OpenGL, and virtual X11. The probe
selected EURGB60 progressive mode 22. Five captured pages were valid; pages
2–5 are retained in [pages](pages/) with the [check transcript](checks.txt).
The [earlier run](../2026-11-05-gx-video-baseline/README.md) had 50 passes and
two VI failures with the same NAND and emulator; it used a different DOL.

The earlier probe expected scan mode 1 for all non-interlaced modes, whereas
`VIDEO_GetVideoScanMode` returns progressive mode 2. The expected control
register encoded EURGB60 as format 5 even though the VI format field is two
bits and `VIDEO_Configure` uses NTSC format bits for EURGB60. The shared mode
calculations now match those contracts; the probe compares the actual scan
mode value. The saved page 4 shows both formerly failing checks passing.

DOL SHA-256:
`f35629773045dc1980d2730845081b9971940791fd559d82fb4fb6ebae4322d9`.
The source was based on revision `29974c7913b4` with the two video source
changes in the commit containing this record. Reproduction: build with
`cargo xtask build-dolphin-probes`, then run `cargo xtask dolphin-validate
--no-build --probe video_probe --timeout-seconds 600` with Dolphin and the
NAND configured. The standalone video timing tests ran six checks with zero
failures; `cargo fmt --all --check` passed.

These screenshots capture probe text and register-check results. They do not
independently verify GX-rendered pixels or all graphics APIs. The result covers
only the named checks in this Dolphin run.
