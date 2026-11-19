# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 351. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::set_disp_copy_src | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_src | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_dst | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_dst | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_copy_clamp | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_copy_clamp | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_gamma | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_gamma | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_frame2field | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_frame2_field | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_y_scale | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::set_disp_copy_y_scale | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::copy_disp | GX display copy encodes first configuration | PASS | [log L431](video_probe/probe.log#L431), [page-019](video_probe/pages/page-019.png) |
| ogc-gx::copy_disp | GX display copy encodes second configuration | PASS | [log L432](video_probe/probe.log#L432), [page-019](video_probe/pages/page-019.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin captured GX display-list command bytes for two configurations. This does not test an executed display copy, physical Wii hardware, clear behavior, or other configurations.
