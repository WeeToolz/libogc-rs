# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 747. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-wiilaunch::try_initialize | Wii launch fallible initialization reads state files | PASS | [log L266](runtime_probe/probe.log#L266), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin verifies the current NAND state files through IOS. Physical Wii NAND behavior remains untested.
