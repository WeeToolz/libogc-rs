# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 391. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video::get_frame_buffer_size | Video framebuffer size aligns width and adds AA rows | PASS | [log L168](runtime_probe/probe.log#L168), [page-006](runtime_probe/pages/page-006.png) |
| ogc-video::snapshot | Video snapshot retains last queried state | PASS | [log L169](runtime_probe/probe.log#L169), [page-006](runtime_probe/pages/page-006.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin preferred mode and synthetic framebuffer dimensions. Video standards, timing transitions, physical output and hardware remain unverified.
- Snapshot check compares values from the wrapper queries. It does not independently verify live VI query results.
