# GX frame lifecycle in Dolphin

The video probe passed 360 checks on Dolphin's Metal backend. It checked
`initialize_display` with a black clear and `present_frame_and_wait`
with white EFB pixels. The XFB values were `0x10801080` and
`0xeb80eb80`. The [batch report](batch-report.md) links to the probe log.
These checks use one display mode and two colors. Physical Wii behavior,
other dimensions, and asynchronous presentation remain open.

The runner requests XFB copies to emulated RAM. A temporary launcher changed
the runner's OpenGL backend argument to Metal and also supplied
`Graphics.Hacks.XFBToTextureEnable=False`. The final run records the XFB
setting in its result; the backend argument in that result is the original
runner argument, before the launcher changed it.

An [earlier run](../2026-11-19-gx-frame-lifecycle/batch-report.md) passed
358 checks and failed the two XFB checks without the XFB-to-RAM setting.
The [comparison run](../2026-11-19-gx-frame-lifecycle-ram/batch-report.md)
passed all 360 checks with the setting. Its
[capture](../2026-11-19-gx-frame-lifecycle-ram/video_probe/pages/page-028.png)
shows the results. The final run's capture for these checks is black, so
use its probe log and the comparison capture to inspect the recorded values.
