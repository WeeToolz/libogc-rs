# Dolphin evidence report

Probe `pad_command_spec5` reported `pass`.
Passed checks 19. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_debug_origin_byte | full-origin-payload-transform | PASS | [log L163](pad_command_spec5/probe.log#L163), [page-008](pad_command_spec5/pages/page-008.png) |
| ogc-input-sys::pad_debug_origin_byte | main-stick-x-patch | PASS | [log L188](pad_command_spec5/probe.log#L188), [page-010](pad_command_spec5/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One deterministic calibration payload and X-axis patch in Dolphin; invalid channel and offset are rejected. Real-controller calibration and physical Wii behavior remain open.
