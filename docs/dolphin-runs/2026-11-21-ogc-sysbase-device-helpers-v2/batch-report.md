# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 756. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-sysbase::open_append | DeviceFile open_append sets append and create flags | PASS | [log L460](runtime_probe/probe.log#L460), [page-018](runtime_probe/pages/page-018.png) |
| ogc-sysbase::open_truncate | DeviceFile open_truncate sets truncate and create flags | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-sysbase::set_stdin_device | stdin device replacement restores the previous device | PASS | [log L462](runtime_probe/probe.log#L462), [page-018](runtime_probe/pages/page-018.png) |
| ogc-sysbase::restore_stdin_device | stdin device replacement restores the previous device | PASS | [log L462](runtime_probe/probe.log#L462), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
