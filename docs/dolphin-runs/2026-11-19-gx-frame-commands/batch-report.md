# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 363. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::configure_frame | GX frame commands configure copy flush and finish | PASS | [log L626](video_probe/probe.log#L626), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::configure_frame_with_progress | GX frame progress and clearing copy | PASS | [log L627](video_probe/probe.log#L627), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::copy_display | GX frame commands configure copy flush and finish | PASS | [log L626](video_probe/probe.log#L626), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::copy_display | GX frame progress and clearing copy | PASS | [log L627](video_probe/probe.log#L627), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::finish_frame | GX frame commands configure copy flush and finish | PASS | [log L626](video_probe/probe.log#L626), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::flush | GX frame commands configure copy flush and finish | PASS | [log L626](video_probe/probe.log#L626), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::present_frame | GX asynchronous present copies cleared EFB and signals completion | PASS | [log L628](video_probe/probe.log#L628), [page-028](video_probe/pages/page-028.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One Dolphin display mode and black/white pixel samples. Other dimensions, colors, interrupt races, and physical Wii behavior remain open.
