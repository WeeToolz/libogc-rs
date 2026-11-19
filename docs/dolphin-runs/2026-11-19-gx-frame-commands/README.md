# GX frame commands in Dolphin

Dolphin passed 363 video probe checks. The new checks exercise frame
configuration, progress callbacks, display copies, FIFO flushing, draw-done
completion, and asynchronous presentation. They compare black and white XFB
pixels with fixed YUV values and check that a clearing copy clears the EFB.
The [batch report](batch-report.md) links each API to its checks.

The runner requested XFB copies to RAM. A temporary launcher changed the
runner's OpenGL backend argument to Metal and also supplied
`Graphics.Hacks.XFBToTextureEnable=False`. The recorded arguments show the
runner's original backend argument. The checks cover one Dolphin display mode.
Other dimensions, colors, interrupt races, and physical Wii behavior remain
open.
