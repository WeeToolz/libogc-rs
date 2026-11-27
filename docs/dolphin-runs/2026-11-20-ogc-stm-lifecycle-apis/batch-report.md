# Dolphin evidence report

Probe `stm_api_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::close | STM close releases the interface without an event hook | Missing |  |
| ogc-stm::init | STM init is safe when the interface is already open | Missing |  |
| ogc-stm::init_without_event_hook | STM init without event hook opens the interface | PASS | [log L75](stm_api_probe/probe.log#L75) |
| ogc-stm::register_event_handler | STM event handler registration returns and restores the previous callback | PASS | [log L74](stm_api_probe/probe.log#L74) |
| ogc-stm::set_led_mode | STM LED mode call accepts mode zero | Missing |  |
| ogc-stm::try_close | STM try close releases the interface | Missing |  |
| ogc-stm::try_init | STM try init opens the interface | Missing |  |
| ogc-stm::try_set_led_mode | STM try LED mode call accepts mode zero | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-stm::close — STM close releases the interface without an event hook (Missing)
- ogc-stm::init — STM init is safe when the interface is already open (Missing)
- ogc-stm::set_led_mode — STM LED mode call accepts mode zero (Missing)
- ogc-stm::try_close — STM try close releases the interface (Missing)
- ogc-stm::try_init — STM try init opens the interface (Missing)
- ogc-stm::try_set_led_mode — STM try LED mode call accepts mode zero (Missing)
