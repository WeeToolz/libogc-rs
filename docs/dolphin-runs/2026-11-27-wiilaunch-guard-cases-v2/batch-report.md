# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 901. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wiilaunch::launch_title | Wii launch rejects an unknown title and preserves its IOS error | PASS | [log L278](runtime_probe/probe.log#L278), [page-011](runtime_probe/pages/page-011.png) |
| ogc-wiilaunch::launch_title_with_args | Wii launch legacy args API returns the size error | PASS | [log L280](runtime_probe/probe.log#L280), [page-011](runtime_probe/pages/page-011.png) |
| ogc-wiilaunch::try_launch_title | Wii launch rejects an unknown title and preserves its IOS error | PASS | [log L278](runtime_probe/probe.log#L278), [page-011](runtime_probe/pages/page-011.png) |
| ogc-wiilaunch::try_launch_title_with_args | Wii launch rejects oversized arguments before persistence | PASS | [log L279](runtime_probe/probe.log#L279), [page-011](runtime_probe/pages/page-011.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
