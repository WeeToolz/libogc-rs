# Dolphin evidence report

Probe `stm_api_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init | STM init opens the interface | PASS | [log L76](stm_api_probe/probe.log#L76) |
| ogc-stm::try_init | STM try init opens an already open interface | PASS | [log L78](stm_api_probe/probe.log#L78) |
| ogc-stm::close | STM close releases the interface | Missing |  |
| ogc-stm::try_close | STM try close accepts a closed interface | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-stm::close — STM close releases the interface (Missing)
- ogc-stm::try_close — STM try close accepts a closed interface (Missing)
