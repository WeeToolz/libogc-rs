# GX metrics in Dolphin

Dolphin Metal passed 373 video probe checks. The new checks compare selected
metric command bytes with the C reference and compare metric output values
with stable command processor registers. They also check the disabled GP
metric and preservation of its second output. The
[batch report](batch-report.md) lists the six APIs and checks.

Counter progression, other selector modes, reset behavior, and physical Wii
behavior remain open. A temporary launcher changed the runner's recorded
OpenGL backend argument to Metal.

The [earlier run](../2026-11-19-gx-metric/batch-report.md) passed the same
checks, but its audit rejected two evidence rows because the probe used
function pointers. The final probe makes both calls explicitly. The final
capture is black. The earlier [capture](../2026-11-19-gx-metric/video_probe/pages/page-029.png)
shows the checks, and the final report links to its probe log.
