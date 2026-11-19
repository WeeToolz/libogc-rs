use crate::probe_support::ProbeLogger;
use wpad_sys as wpad;

/// Check the WPAD host lifecycle without starting a Bluetooth inquiry or
/// changing saved controller configuration.
pub fn run(logger: &mut ProbeLogger) {
    logger.section("WPAD LIFECYCLE PROBE");
    probe_invalid_channel_queries(logger);
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

    let mut expansion = 0xfeed_beef;
    let no_controller = unsafe { wpad::probe(wpad::WPAD_CHAN_0, &mut expansion) };
    logger.note_i32("WPAD channel 0 probe result", no_controller);
    logger.record(
        "empty channel probe preserves output on no controller",
        no_controller == wpad::WPAD_ERR_NO_CONTROLLER as i32 && expansion == 0xfeed_beef,
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

fn probe_invalid_channel_queries(logger: &mut ProbeLogger) {
    let invalid = wpad::WPAD_MAX_WIIMOTES as i32;
    logger.record(
        "WPAD invalid channel has no data pointer",
        wpad::data(invalid).is_null(),
    );
    logger.record(
        "WPAD invalid channel battery level is zero",
        wpad::battery_level(invalid) == 0,
    );
    logger.record(
        "WPAD invalid channel button edges are zero",
        wpad::buttons_down(invalid) == 0 && wpad::buttons_up(invalid) == 0,
    );
    logger.record(
        "WPAD invalid channel held buttons are zero",
        wpad::buttons_held(invalid) == 0,
    );

    let mut accel = wpad::Vec3w {
        x: 0x1234,
        y: 0x2345,
        z: 0x3456,
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::accel(invalid, &mut accel) };
    logger.record(
        "WPAD invalid channel leaves acceleration output unchanged",
        accel.x == 0x1234 && accel.y == 0x2345 && accel.z == 0x3456,
    );

    let mut ir = wpad::Ir {
        num_dots: 9,
        state: 7,
        valid: 5,
        ..wpad::Ir::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::ir(invalid, &mut ir) };
    logger.record(
        "WPAD invalid channel leaves infrared output unchanged",
        ir.num_dots == 9 && ir.state == 7 && ir.valid == 5,
    );

    let mut orient = wpad::Orient {
        roll: 1.25,
        pitch: 2.5,
        ..wpad::Orient::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::orientation(invalid, &mut orient) };
    logger.record(
        "WPAD invalid channel leaves orientation output unchanged",
        orient.roll == 1.25 && orient.pitch == 2.5,
    );

    let mut force = wpad::GForce {
        x: 3.5,
        y: 4.5,
        ..wpad::GForce::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::g_force(invalid, &mut force) };
    logger.record(
        "WPAD invalid channel leaves g-force output unchanged",
        force.x == 3.5 && force.y == 4.5,
    );

    let mut expansion = wpad::Expansion {
        type_: 0x1234,
        ..wpad::Expansion::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::expansion(invalid, &mut expansion) };
    logger.record(
        "WPAD invalid channel leaves expansion output unchanged",
        expansion.type_ == 0x1234,
    );
}
