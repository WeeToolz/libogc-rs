# Dolphin evidence report

Probe `stm_api_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init | STM init is safe when the interface is already open | PASS | [log L77](stm_api_probe/probe.log#L77) |
| ogc-stm::try_init | STM try init accepts an already open interface | PASS | [log L79](stm_api_probe/probe.log#L79) |
| ogc-stm::close | STM close releases the interface | Missing |  |
| ogc-stm::try_close | STM try close accepts a closed interface | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-stm::close — STM close releases the interface (Missing)
- ogc-stm::try_close — STM try close accepts a closed interface (Missing)
