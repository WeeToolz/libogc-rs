# Dolphin evidence report

Probe `stm_api_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init | STM init is safe when the interface is already open | PASS | [log L74](stm_api_probe/probe.log#L74) |
| ogc-stm::init | STM init opens a closed interface with event hook | PASS | [log L78](stm_api_probe/probe.log#L78) |
| ogc-stm::init | STM close releases event hook after init | Missing |  |
| ogc-stm::try_init | STM try init accepts an already open interface | PASS | [log L75](stm_api_probe/probe.log#L75) |
| ogc-stm::try_init | STM try init opens a closed interface | Missing |  |
| ogc-stm::try_init | STM close releases event hook after try init | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-stm::init — STM close releases event hook after init (Missing)
- ogc-stm::try_init — STM try init opens a closed interface (Missing)
- ogc-stm::try_init — STM close releases event hook after try init (Missing)
