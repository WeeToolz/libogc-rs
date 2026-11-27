# Dolphin evidence report

Probe `video_probe` reported `probe_fail`.
Passed checks 388. Failed checks 5.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::console_init | Video console_init initializes once and restores the probe display | PASS | [log L689](video_probe/probe.log#L689), [page-032](video_probe/pages/page-032.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — GX EFB Z24 depths preserve X and Y coordinates
- Other failed check — GX EFB replacement preserves adjacent color and depth
- Other failed check — GX EFB copy writes expected tiled RGBA8 bytes
- Other failed check — GX frame lifecycle presents white EFB pixels
- Other failed check — GX frame progress and clearing copy
