# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 675. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-video-sys::get_current_line | Video sys current line query stays in range | PASS | [log L343](runtime_probe/probe.log#L343), [page-013](runtime_probe/pages/page-013.png) |
| ogc-video-sys::get_next_field | Video sys next field query is a valid field | PASS | [log L345](runtime_probe/probe.log#L345), [page-014](runtime_probe/pages/page-014.png) |
| ogc-video-sys::timer_clock_hz | Video sys reports the Broadway timer frequency | PASS | [log L346](runtime_probe/probe.log#L346), [page-014](runtime_probe/pages/page-014.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies Wii-mode query bounds and the emulated Broadway timer value. It does not provide physical-console timing evidence.
