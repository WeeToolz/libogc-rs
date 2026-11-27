# Dolphin evidence report

Probe `stm_api_probe` reported `pass`.
Passed checks 8. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::close | STM close releases the interface | PASS | [log L74](stm_api_probe/probe.log#L74), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_close | STM try close accepts a closed interface | PASS | [log L75](stm_api_probe/probe.log#L75), [page-002](stm_api_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
