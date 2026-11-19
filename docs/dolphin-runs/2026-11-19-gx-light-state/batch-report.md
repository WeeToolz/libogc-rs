# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 377. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::load_light_obj | GX immediate light loads all words and selects slot | PASS | [log L643](video_probe/probe.log#L643), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::load_light_obj_idx | GX indexed light encodes index and slot | PASS | [log L644](video_probe/probe.log#L644), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::pix_mode_sync | GX pixel mode sync emits current pixel control | PASS | [log L645](video_probe/probe.log#L645), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_bounding_box | GX bounding box outputs match stable PE registers | PASS | [log L646](video_probe/probe.log#L646), [page-029](video_probe/pages/page-029.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Captured light and pixel control commands and stable Dolphin bounding-box registers. Light rendering, active bounding-box updates, other pixel formats, and physical Wii behavior remain open.
