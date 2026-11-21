# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 736. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video-sys::init | Raw video setup has a live mode and framebuffer | PASS | [log L1069](runtime_probe/probe.log#L1069), [page-044](runtime_probe/pages/page-044.png) |
| ogc-video-sys::init | Raw video init clears callbacks and keeps retrace active | PASS | [log L1070](runtime_probe/probe.log#L1070), [page-044](runtime_probe/pages/page-044.png) |
| ogc-video-sys::configure | Raw video configure applies valid geometry despite invalid requests | PASS | [log L1071](runtime_probe/probe.log#L1071), [page-044](runtime_probe/pages/page-044.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin preferred-mode horizontal geometry and retrace checks. Other modes and physical Wii video behavior remain open.
