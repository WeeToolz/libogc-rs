# GX light commands and pixel state in Dolphin

Dolphin Metal passed 377 video probe checks. The new checks compare immediate
and indexed light commands with fixed C reference bytes, including invalid
light IDs. They check the pixel control command and compare bounding-box
outputs with stable pixel-engine registers. The [batch report](batch-report.md)
links each of the four APIs to its checks.

The captured light commands were not executed. Light rendering, active
bounding-box updates, other pixel formats, and physical Wii behavior remain
open. A temporary launcher changed the runner's recorded OpenGL backend
argument to Metal.
