# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 373. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::init_xf_ras_metric | GX XF raster metric initialization commands | PASS | [log L635](video_probe/probe.log#L635), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::clear_v_cache_metric | GX vertex cache metric clear command | PASS | [log L636](video_probe/probe.log#L636), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::set_gp_metric | GX metric selector emits fixed vertex and texel commands | PASS | [log L637](video_probe/probe.log#L637), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_xf_ras_metric | GX XF raster metric outputs match stable CP registers | PASS | [log L638](video_probe/probe.log#L638), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_v_cache_metric | GX vertex cache metric outputs match stable CP registers | PASS | [log L639](video_probe/probe.log#L639), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_gp_metric | GX vertex metric matches CP register and preserves second output | PASS | [log L640](video_probe/probe.log#L640), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_gp_metric | GX disabled metric returns zero | PASS | [log L641](video_probe/probe.log#L641), [page-029](video_probe/pages/page-029.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Selected command encodings and stable Dolphin CP register values. Counter progression, other selector modes, reset behavior, and physical Wii hardware remain untested.
