# Dolphin evidence report

Probe `stm_api_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::close | STM close releases the interface | Missing |  |
| ogc-stm::try_close | STM try close accepts a closed interface | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-stm::close — STM close releases the interface (Missing)
- ogc-stm::try_close — STM try close accepts a closed interface (Missing)
