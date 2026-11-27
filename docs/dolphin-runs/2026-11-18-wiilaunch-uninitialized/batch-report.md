# Dolphin evidence report

Probe `runtime_probe` reported `crash`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wiilaunch::try_launch_title | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::launch_title | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::try_launch_title_with_args | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::launch_title_with_args | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::try_return_to_menu | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::return_to_menu | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::try_return_to_settings | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::return_to_settings | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::try_return_to_settings_page | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::return_to_settings_page | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::try_open_url | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |
| ogc-wiilaunch::open_url | Wii launch operations require initialization | FAIL | [log L216](runtime_probe/probe.log#L216), [page-008](runtime_probe/pages/page-008.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-wiilaunch::try_launch_title — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::launch_title — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::try_launch_title_with_args — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::launch_title_with_args — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::try_return_to_menu — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::return_to_menu — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::try_return_to_settings — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::return_to_settings — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::try_return_to_settings_page — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::return_to_settings_page — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::try_open_url — Wii launch operations require initialization (FAIL)
- ogc-wiilaunch::open_url — Wii launch operations require initialization (FAIL)

## Coverage limits

- Dolphin uninitialized launch state only. Initialized launches, ES interactions, persistence and physical Wii remain unverified.
