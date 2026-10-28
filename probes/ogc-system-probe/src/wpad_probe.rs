use crate::probe_support::ProbeLogger;
use wpad_sys as wpad;

/// Check the WPAD host lifecycle without starting a Bluetooth inquiry or
/// changing saved controller configuration.
pub fn run(logger: &mut ProbeLogger) {
    logger.section("WPAD LIFECYCLE PROBE");
    logger.record(
        "WPAD starts disabled",
        wpad::get_status() == wpad::WPAD_STATE_DISABLED,
    );

    let init = wpad::init();
    logger.note_i32("WPAD init result", init);
    logger.record("WPAD init succeeds", init == wpad::WPAD_ERR_NONE as i32);
    if init != wpad::WPAD_ERR_NONE as i32 {
        return;
    }

    let deadline = ogc_system::time() + 60_750_000 * 6;
    while wpad::get_status() == wpad::WPAD_STATE_ENABLING && ogc_system::time() < deadline {
        let _ = wpad::scan_pads();
        logger.display().wait_vsync();
        if wpad::bluetooth_diagnostics().stage == ogc_bt_sync::HostStage::Failed {
            break;
        }
    }
    logger.record(
        "WPAD reports enabled",
        wpad::get_status() == wpad::WPAD_STATE_ENABLED,
    );

    let mut expansion = u32::MAX;
    let no_controller = unsafe { wpad::probe(wpad::WPAD_CHAN_0, &mut expansion) };
    logger.note_i32("WPAD channel 0 probe result", no_controller);
    logger.record(
        "empty channel reports no controller",
        no_controller == wpad::WPAD_ERR_NO_CONTROLLER as i32 && expansion == 0,
    );

    let invalid_channel = unsafe { wpad::probe(wpad::WPAD_MAX_WIIMOTES as i32, &mut expansion) };
    logger.record(
        "invalid WPAD channel is rejected",
        invalid_channel == wpad::WPAD_ERR_BAD_CHANNEL as i32,
    );

    let mut event = wpad::WpadData::default();
    let read = unsafe { wpad::read_event(wpad::WPAD_CHAN_0, &mut event) };
    logger.record(
        "empty channel event read reports no controller",
        read == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );

    let shutdown = wpad::shutdown();
    logger.note_i32("WPAD shutdown result", shutdown);
    logger.record(
        "WPAD shutdown succeeds",
        shutdown == wpad::WPAD_ERR_NONE as i32,
    );
    let deadline = ogc_system::time() + 60_750_000 * 6;
    while !wpad::bluetooth_shutdown_complete() && ogc_system::time() < deadline {
        let _ = wpad::scan_pads();
        logger.display().wait_vsync();
    }
    logger.record(
        "WPAD adapter close completed",
        wpad::bluetooth_shutdown_complete(),
    );
    logger.record(
        "WPAD reports disabled after shutdown",
        wpad::get_status() == wpad::WPAD_STATE_DISABLED,
    );
}
