# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 360. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::initialize_display | GX frame lifecycle initializes display | PASS | [log L622](video_probe/probe.log#L622), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::present_frame_and_wait | GX frame lifecycle presents white EFB pixels | PASS | [log L624](video_probe/probe.log#L624), [page-028](video_probe/pages/page-028.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Black initialization and a white EFB sample in one Dolphin display mode. Other colors, frame dimensions, asynchronous presentation, and physical Wii behavior remain open.
