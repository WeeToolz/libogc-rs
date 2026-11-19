# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 563. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::set_framebuffer | Video wrapper immediate framebuffer programs VI | PASS | [log L245](runtime_probe/probe.log#L245), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video::set_framebuffer | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::set_framebuffer | Video raw immediate framebuffer programs VI | PASS | [log L244](runtime_probe/probe.log#L244), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::set_framebuffer | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::set_next_framebuffer | Video raw next framebuffer latches after flush | PASS | [log L246](runtime_probe/probe.log#L246), [page-009](runtime_probe/pages/page-009.png) |
| ogc-video-sys::set_next_framebuffer | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::flush | Video raw flush publishes framebuffer | PASS | [log L248](runtime_probe/probe.log#L248), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::flush | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::set_black | Video raw black toggles vertical display | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::set_black | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::have_component_cable | Video raw component cable matches VI register | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::have_component_cable | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::get_preferred_mode | Video raw preferred mode copies into caller storage | PASS | [log L252](runtime_probe/probe.log#L252), [page-010](runtime_probe/pages/page-010.png) |
| ogc-video-sys::get_preferred_mode | Video framebuffer fixture restored | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin active video mode, private framebuffer fixture and current cable configuration only. Other modes, timing and physical Wii remain unverified.
