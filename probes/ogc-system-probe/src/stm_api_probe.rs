use crate::probe_support::ProbeLogger;
use ogc_stm as stm;

#[cfg(not(feature = "stm-api-probe"))]
unsafe extern "C" fn probe_event_handler(_event: u32) {}

// Keep the historical LED evidence entry point intact in regular builds. The
// focused close batch uses `run_close` because the LED path can block in Dolphin.
#[cfg(not(feature = "stm-api-probe"))]
pub fn run(logger: &mut ProbeLogger) {
    logger.section("STM API PROBE");

    let previous = stm::register_event_handler(Some(probe_event_handler));
    let restored = stm::register_event_handler(previous);
    logger.record(
        "STM event handler registration returns and restores the previous callback",
        previous.is_none() && restored.is_some(),
    );

    let no_hook_status = stm::init_without_event_hook();
    logger.record(
        "STM init without event hook opens the interface",
        no_hook_status > 0,
    );

    let led_status = if no_hook_status > 0 {
        stm::set_led_mode(0)
    } else {
        -1
    };
    logger.record("STM LED mode call accepts mode zero", led_status == 0);
    let try_led = no_hook_status > 0 && stm::try_set_led_mode(0).is_ok();
    logger.record("STM try LED mode call accepts mode zero", try_led);
}

#[cfg(feature = "stm-api-probe")]
pub fn run_close(logger: &mut ProbeLogger) {
    logger.section("STM CLOSE API PROBE");

    let no_hook_status = stm::init_without_event_hook();
    logger.record(
        "STM init without event hook opens the interface",
        no_hook_status > 0,
    );

    let close_status = stm::close();
    logger.record("STM close releases the interface", close_status >= 0);

    let try_close_status = stm::try_close();
    logger.record(
        "STM try close accepts a closed interface",
        try_close_status.is_ok(),
    );
}
