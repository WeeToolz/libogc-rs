# Dolphin evidence report

Probe `runtime_probe` reported `crash`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wiilaunch::launch_title | Wii launch raw title returns not-initialized code | Missing |  |
| ogc-wiilaunch::launch_title_with_args | Wii launch raw title with args returns not-initialized code | Missing |  |
| ogc-wiilaunch::open_url | Wii launch raw URL returns not-initialized code | Missing |  |
| ogc-wiilaunch::return_to_menu | Wii launch raw menu return returns not-initialized code | Missing |  |
| ogc-wiilaunch::return_to_settings | Wii launch raw settings return returns not-initialized code | Missing |  |
| ogc-wiilaunch::return_to_settings_page | Wii launch raw settings page returns not-initialized code | Missing |  |
| ogc-wiilaunch::try_launch_title | Wii launch try title rejects uninitialized state | FAIL | [log L378](runtime_probe/probe.log#L378) |
| ogc-wiilaunch::try_launch_title_with_args | Wii launch try title with args rejects uninitialized state | FAIL | [log L379](runtime_probe/probe.log#L379) |
| ogc-wiilaunch::try_open_url | Wii launch try URL rejects uninitialized state | FAIL | [log L380](runtime_probe/probe.log#L380) |
| ogc-wiilaunch::try_return_to_menu | Wii launch try menu return rejects uninitialized state | FAIL | [log L381](runtime_probe/probe.log#L381) |
| ogc-wiilaunch::try_return_to_settings | Wii launch try settings return rejects uninitialized state | FAIL | [log L382](runtime_probe/probe.log#L382) |
| ogc-wiilaunch::try_return_to_settings_page | Wii launch try settings page rejects uninitialized state | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-wiilaunch::launch_title — Wii launch raw title returns not-initialized code (Missing)
- ogc-wiilaunch::launch_title_with_args — Wii launch raw title with args returns not-initialized code (Missing)
- ogc-wiilaunch::open_url — Wii launch raw URL returns not-initialized code (Missing)
- ogc-wiilaunch::return_to_menu — Wii launch raw menu return returns not-initialized code (Missing)
- ogc-wiilaunch::return_to_settings — Wii launch raw settings return returns not-initialized code (Missing)
- ogc-wiilaunch::return_to_settings_page — Wii launch raw settings page returns not-initialized code (Missing)
- ogc-wiilaunch::try_launch_title — Wii launch try title rejects uninitialized state (FAIL)
- ogc-wiilaunch::try_launch_title_with_args — Wii launch try title with args rejects uninitialized state (FAIL)
- ogc-wiilaunch::try_open_url — Wii launch try URL rejects uninitialized state (FAIL)
- ogc-wiilaunch::try_return_to_menu — Wii launch try menu return rejects uninitialized state (FAIL)
- ogc-wiilaunch::try_return_to_settings — Wii launch try settings return rejects uninitialized state (FAIL)
- ogc-wiilaunch::try_return_to_settings_page — Wii launch try settings page rejects uninitialized state (Missing)
