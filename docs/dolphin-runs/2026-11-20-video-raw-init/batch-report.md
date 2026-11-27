# Dolphin evidence report

Probe `video_probe` reported `probe_fail`.
Passed checks 388. Failed checks 5.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video-sys::init | Raw video init clears retrace callbacks | PASS | [log L121](video_probe/probe.log#L121), [page-004](video_probe/pages/page-004.png) |
| ogc-video-sys::init | VI retrace interrupt arrives after flush | PASS | [log L131](video_probe/probe.log#L131), [page-005](video_probe/pages/page-005.png) |
| ogc-video-sys::configure | Raw video configure commits the valid mode and ignores invalid requests | PASS | [log L132](video_probe/probe.log#L132), [page-005](video_probe/pages/page-005.png) |
| ogc-video-sys::configure | VI retrace interrupt arrives after flush | PASS | [log L131](video_probe/probe.log#L131), [page-005](video_probe/pages/page-005.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- Other failed check — GX EFB Z24 depths preserve X and Y coordinates
- Other failed check — GX EFB replacement preserves adjacent color and depth
- Other failed check — GX EFB copy writes expected tiled RGBA8 bytes
- Other failed check — GX frame lifecycle presents white EFB pixels
- Other failed check — GX frame progress and clearing copy

## Coverage limits

- Dolphin preferred mode and retrace tests. Other modes, stereoscopic scanout, and physical Wii video behavior remain open.
