# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 555. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::control_register_matches | Video control match distinguishes mode | PASS | [log L234](runtime_probe/probe.log#L234), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::framebuffer_registers_match | Video framebuffer match distinguishes buffers | PASS | [log L236](runtime_probe/probe.log#L236), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::get_current_tv_mode | Video current TV mode matches configured mode | PASS | [log L238](runtime_probe/probe.log#L238), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::get_video_scan_mode | Video scan mode matches VI registers | PASS | [log L240](runtime_probe/probe.log#L240), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::wait_vsync | Video wait_vsync observes retrace | PASS | [log L241](runtime_probe/probe.log#L241), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::control_register_matches | Video raw control match distinguishes mode | PASS | [log L235](runtime_probe/probe.log#L235), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::framebuffer_registers_match | Video raw framebuffer match distinguishes buffers | PASS | [log L237](runtime_probe/probe.log#L237), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::get_current_tv_mode | Video raw TV mode matches configured mode | PASS | [log L239](runtime_probe/probe.log#L239), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::wait_v_sync | Video raw wait_v_sync observes retrace | PASS | [log L242](runtime_probe/probe.log#L242), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::read_register_snapshot | Video raw register snapshot matches VI registers | PASS | [log L233](runtime_probe/probe.log#L233), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin active video mode and two probe framebuffers only. Other modes, timing accuracy, and physical Wii remain unverified.
