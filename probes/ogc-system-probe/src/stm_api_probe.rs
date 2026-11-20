use crate::probe_support::ProbeLogger;
use ogc_stm as stm;

unsafe extern "C" fn probe_event_handler(_event: u32) {}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("STM API PROBE");

    let previous = stm::register_event_handler(Some(probe_event_handler));
    let restored = stm::register_event_handler(previous);
    logger.record(
        "STM event handler registration returns and restores the previous callback",
        previous.is_none() && restored.is_some(),
    );

    let no_hook_ready = stm::init_without_event_hook() > 0;
    logger.record(
        "STM init without event hook opens the interface",
        no_hook_ready,
    );
    let led_status = if no_hook_ready {
        stm::set_led_mode(0)
    } else {
        -1
    };
    logger.record("STM LED mode call accepts mode zero", led_status == 0);
    let try_led = no_hook_ready && stm::try_set_led_mode(0).is_ok();
    logger.record("STM try LED mode call accepts mode zero", try_led);
}
