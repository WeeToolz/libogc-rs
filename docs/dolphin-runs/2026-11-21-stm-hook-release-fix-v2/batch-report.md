# Dolphin evidence report

Probe `stm_api_probe` reported `pass`.
Passed checks 14. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init | STM init is safe when the interface is already open | PASS | [log L74](stm_api_probe/probe.log#L74), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::init | STM init opens a closed interface with event hook | PASS | [log L78](stm_api_probe/probe.log#L78), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_init | STM try init accepts an already open interface | PASS | [log L75](stm_api_probe/probe.log#L75), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_init | STM try init opens a closed interface | PASS | [log L81](stm_api_probe/probe.log#L81), [page-003](stm_api_probe/pages/page-003.png) |
| ogc-stm::close | STM close releases the interface | PASS | [log L76](stm_api_probe/probe.log#L76), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::close | STM close releases event hook after init | PASS | [log L79](stm_api_probe/probe.log#L79), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_close | STM try close accepts a closed interface | PASS | [log L77](stm_api_probe/probe.log#L77), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_close | STM try close releases event hook after try init | PASS | [log L82](stm_api_probe/probe.log#L82), [page-003](stm_api_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
