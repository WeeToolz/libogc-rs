# Dolphin evidence report

Probe `stm_api_probe` reported `pass`.
Passed checks 10. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init | STM init is safe when the interface is already open | PASS | [log L74](stm_api_probe/probe.log#L74), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_init | STM try init accepts an already open interface | PASS | [log L75](stm_api_probe/probe.log#L75), [page-002](stm_api_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
