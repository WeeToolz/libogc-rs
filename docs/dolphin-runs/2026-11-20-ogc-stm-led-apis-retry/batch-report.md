# Dolphin evidence report

Probe `stm_api_probe` reported `pass`.
Passed checks 8. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-stm::init_without_event_hook | STM init without event hook opens the interface | PASS | [log L75](stm_api_probe/probe.log#L75), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::register_event_handler | STM event handler registration returns and restores the previous callback | PASS | [log L74](stm_api_probe/probe.log#L74), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::set_led_mode | STM LED mode call accepts mode zero | PASS | [log L76](stm_api_probe/probe.log#L76), [page-002](stm_api_probe/pages/page-002.png) |
| ogc-stm::try_set_led_mode | STM try LED mode call accepts mode zero | PASS | [log L77](stm_api_probe/probe.log#L77), [page-002](stm_api_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
