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

    let init_status = stm::init();
    logger.record(
        "STM init is safe when the interface is already open",
        init_status > 0,
    );
    let try_init_status = stm::try_init();
    logger.record(
        "STM try init accepts an already open interface",
        try_init_status.is_ok(),
    );

    let close_status = stm::close();
    logger.record("STM close releases the interface", close_status >= 0);

    let try_close_status = stm::try_close();
    logger.record(
        "STM try close accepts a closed interface",
        try_close_status.is_ok(),
    );

    let reopened = stm::init();
    logger.record(
        "STM init opens a closed interface with event hook",
        reopened > 0,
    );
    let closed_after_init = stm::close();
    logger.record(
        "STM close releases event hook after init",
        closed_after_init >= 0,
    );
    let reopened_try = stm::try_init();
    logger.record(
        "STM try init opens a closed interface",
        reopened_try.is_ok(),
    );
    logger.record(
        "STM try close releases event hook after try init",
        stm::try_close().is_ok(),
    );

    // Check the closed-state guards without sending a power-control request.
    // These APIs clear VI before they check STM state, so restore that register
    // after the calls to keep the remaining probe output visible.
    let vi_control = unsafe { core::ptr::read_volatile(0xcc00_2002 as *const u16) };
    let reboot = stm::reboot_system();
    let standby = stm::shutdown_to_standby();
    let idle = stm::shutdown_to_idle();
    let reboot_try = stm::try_reboot_system();
    let standby_try = stm::try_shutdown_to_standby();
    let idle_try = stm::try_shutdown_to_idle();
    unsafe {
        core::ptr::write_volatile(0xcc00_2002 as *mut u16, vi_control);
    }
    logger.note_i32("closed STM reboot result", reboot);
    logger.record(
        "STM reboot rejects a closed interface",
        reboot == stm::STM_ENOTINIT,
    );
    logger.note_i32("closed STM standby result", standby);
    logger.record(
        "STM standby shutdown rejects a closed interface",
        standby == stm::STM_ENOTINIT,
    );
    logger.note_i32("closed STM idle result", idle);
    logger.record(
        "STM idle shutdown rejects a closed interface",
        idle == stm::STM_ENOTINIT,
    );
    logger.record(
        "STM try reboot preserves the closed-interface error",
        reboot_try == Err(stm::ErrorCode::new(stm::STM_ENOTINIT)),
    );
    logger.record(
        "STM try standby preserves the closed-interface error",
        standby_try == Err(stm::ErrorCode::new(stm::STM_ENOTINIT)),
    );
    logger.record(
        "STM try idle preserves the closed-interface error",
        idle_try == Err(stm::ErrorCode::new(stm::STM_ENOTINIT)),
    );
}
