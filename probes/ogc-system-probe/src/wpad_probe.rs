use crate::probe_support::ProbeLogger;
use core::sync::atomic::{AtomicU32, Ordering};
use wpad_sys as wpad;

static BLUETOOTH_POLL_MARKS: AtomicU32 = AtomicU32::new(0);
static mut EVENT_BUFFER: core::mem::MaybeUninit<wpad::WpadData> = core::mem::MaybeUninit::uninit();

/// Check the WPAD host lifecycle without starting a Bluetooth inquiry or
/// changing saved controller configuration.
pub fn run(logger: &mut ProbeLogger) {
    logger.section("WPAD LIFECYCLE PROBE");
    #[cfg(feature = "dolphin-automation")]
    probe_disabled_discovery(logger);
    probe_invalid_channel_queries(logger);
    logger.record(
        "WPAD starts disabled",
        wpad::get_status() == wpad::WPAD_STATE_DISABLED,
    );

    wpad::set_bluetooth_poll_hook(Some(count_bluetooth_poll_mark));
    let init = wpad::init();
    logger.note_i32("WPAD init result", init);
    logger.record("WPAD init succeeds", init == wpad::WPAD_ERR_NONE as i32);
    if init != wpad::WPAD_ERR_NONE as i32 {
        wpad::set_bluetooth_poll_hook(None);
        return;
    }
    probe_valid_channel_argument_guards(logger);

    let deadline = ogc_system::time() + 60_750_000 * 6;
    while wpad::get_status() == wpad::WPAD_STATE_ENABLING && ogc_system::time() < deadline {
        let _ = wpad::scan_pads();
        logger.display().wait_vsync();
        if wpad::bluetooth_diagnostics().stage == ogc_bt_sync::HostStage::Failed {
            break;
        }
    }
    wpad::set_bluetooth_poll_hook(None);
    logger.record(
        "WPAD Bluetooth poll hook observes scan boundary",
        BLUETOOTH_POLL_MARKS.load(Ordering::Acquire) > 0,
    );
    logger.record(
        "WPAD reports enabled",
        wpad::get_status() == wpad::WPAD_STATE_ENABLED,
    );
    probe_valid_channel_empty_queries(logger);
    probe_empty_channel_event_queue(logger);
    probe_invalid_channel_commands(logger);
    probe_bluetooth_helpers(logger);

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
        "WPAD Bluetooth shutdown query reports complete",
        wpad::bluetooth_shutdown_complete(),
    );
    logger.record(
        "WPAD reports disabled after shutdown",
        wpad::get_status() == wpad::WPAD_STATE_DISABLED,
    );
}

fn probe_empty_channel_event_queue(logger: &mut ProbeLogger) {
    let channel = wpad::WPAD_CHAN_0;
    let mut event = wpad::WpadData::default();
    // SAFETY: `event` is a writable value for the duration of the call.
    let read = unsafe { wpad::read_event(channel, &mut event) };
    logger.record(
        "WPAD empty channel read reports no controller",
        read == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );
    logger.record(
        "WPAD empty channel flush reports no controller",
        wpad::flush(channel) == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );
    logger.record(
        "WPAD empty channel dropped-event count is zero",
        wpad::dropped_events(channel) == 0,
    );
}

fn probe_valid_channel_empty_queries(logger: &mut ProbeLogger) {
    let chan = wpad::WPAD_CHAN_0;
    logger.record(
        "WPAD empty channel data pointer is available",
        !wpad::data(chan).is_null(),
    );
    logger.record(
        "WPAD empty channel battery level is zero",
        wpad::battery_level(chan) == 0,
    );
    logger.record(
        "WPAD empty channel battery is not critical",
        !wpad::is_battery_critical(chan),
    );
    logger.record(
        "WPAD empty channel speaker query reports not ready",
        wpad::is_speaker_enabled(chan) == wpad::WPAD_ERR_NOT_READY as i32,
    );
    logger.record(
        "WPAD empty channel data format update succeeds",
        wpad::set_data_format(chan, wpad::WPAD_FMT_BTNS_ACC) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel IR resolution update succeeds",
        wpad::set_v_res(chan, 640, 480) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel idle thresholds update succeeds",
        wpad::set_idle_thresholds(chan, 5, 11, 17, 23, 29, 31) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel button states are zero",
        wpad::buttons_down(chan) == 0
            && wpad::buttons_held(chan) == 0
            && wpad::buttons_up(chan) == 0,
    );

    let mut accel = wpad::Vec3w {
        x: 0x1234,
        y: 0x2345,
        z: 0x3456,
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::accel(chan, &mut accel) };
    logger.record(
        "WPAD empty channel acceleration is zero",
        accel.x == 0 && accel.y == 0 && accel.z == 0,
    );

    let mut force = wpad::GForce {
        x: 3.5,
        y: 4.5,
        ..wpad::GForce::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::g_force(chan, &mut force) };
    logger.record(
        "WPAD empty channel g-force is zero",
        force.x == 0.0 && force.y == 0.0,
    );

    let mut ir = wpad::Ir {
        num_dots: 9,
        state: 7,
        valid: 5,
        ..wpad::Ir::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::ir(chan, &mut ir) };
    logger.record(
        "WPAD empty channel infrared state is zero",
        ir.num_dots == 0 && ir.state == 0 && ir.valid == 0,
    );

    let mut expansion = wpad::Expansion {
        type_: 0x1234,
        ..wpad::Expansion::default()
    };
    // SAFETY: The output points to a valid local value.
    unsafe { wpad::expansion(chan, &mut expansion) };
    logger.record(
        "WPAD empty channel expansion type is none",
        expansion.type_ == 0,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_disabled_discovery(logger: &mut ProbeLogger) {
    static SYNC_EVENTS: AtomicU32 = AtomicU32::new(0);
    static SYNC_VALUE: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn record_sync(held: u32) {
        SYNC_VALUE.store(held, Ordering::Relaxed);
        SYNC_EVENTS.fetch_add(1, Ordering::Relaxed);
    }

    let disabled = wpad::get_status() == wpad::WPAD_STATE_DISABLED;
    logger.record("WPAD discovery starts with adapter disabled", disabled);
    if !disabled {
        return;
    }
    let not_ready = wpad::WPAD_ERR_NOT_READY as i32;
    logger.record(
        "WPAD Wii discovery requires an adapter",
        wpad::search() == not_ready,
    );
    logger.record(
        "WPAD HID discovery requires an adapter",
        wpad::search_bluetooth_hid() == not_ready,
    );
    logger.record(
        "WPAD stop discovery requires an adapter",
        wpad::stop_search() == not_ready,
    );

    SYNC_EVENTS.store(0, Ordering::Relaxed);
    SYNC_VALUE.store(0, Ordering::Relaxed);
    wpad::set_host_sync_button_callback(Some(record_sync));
    let with_callback = wpad::start_pairing();
    let notified =
        SYNC_EVENTS.load(Ordering::Relaxed) == 1 && SYNC_VALUE.load(Ordering::Relaxed) == 1;
    wpad::set_host_sync_button_callback(None);
    let without_callback = wpad::start_pairing();
    logger.record(
        "WPAD pairing rejects discovery without an adapter",
        with_callback == not_ready && without_callback == not_ready,
    );
    logger.record(
        "WPAD sync callback fires once and can be removed",
        notified && SYNC_EVENTS.load(Ordering::Relaxed) == 1,
    );
    logger.record(
        "WPAD discovery leaves adapter disabled",
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

fn probe_invalid_channel_commands(logger: &mut ProbeLogger) {
    let invalid = wpad::WPAD_MAX_WIIMOTES as i32;
    let bad_channel = wpad::WPAD_ERR_BAD_CHANNEL as i32;
    logger.record(
        "WPAD invalid channel rejects speaker control",
        wpad::control_speaker(invalid, 1) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects disconnect",
        wpad::disconnect(invalid) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects dropped-event query",
        wpad::dropped_events(invalid) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects event flush",
        wpad::flush(invalid) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects speaker query",
        wpad::is_speaker_enabled(invalid) == bad_channel,
    );

    let mut event = wpad::WpadData {
        err: 0x1234,
        battery_level: 0x5a,
        ..wpad::WpadData::default()
    };
    // SAFETY: The output points to a valid local value.
    let read = unsafe { wpad::read_event(invalid, &mut event) };
    logger.record(
        "WPAD invalid channel rejects event read and preserves output",
        read == bad_channel && event.err == 0x1234 && event.battery_level == 0x5a,
    );
    logger.record(
        "WPAD invalid channel rejects pending-event read",
        wpad::read_pending(invalid, None) == bad_channel,
    );
    let status_before_pad_status = wpad::get_status();
    wpad::pad_status(invalid);
    logger.record(
        "WPAD invalid channel pad status leaves lifecycle state unchanged",
        wpad::get_status() == status_before_pad_status,
    );

    logger.record(
        "WPAD invalid channel rejects rumble",
        wpad::rumble(invalid, 1) == bad_channel,
    );
    let mut stream_data = [0x5a_u8];
    // SAFETY: The pointer references one readable byte for this call.
    let stream = unsafe { wpad::send_stream_data(invalid, stream_data.as_mut_ptr().cast(), 1) };
    logger.record(
        "WPAD invalid channel rejects speaker stream",
        stream == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects data format",
        wpad::set_data_format(invalid, wpad::WPAD_FMT_BTNS) == bad_channel,
    );
    let mut event_buffer = wpad::WpadData::default();
    // SAFETY: The pointer references one writable event value.
    let buffers = unsafe { wpad::set_event_bufs(invalid, &mut event_buffer, 1) };
    logger.record(
        "WPAD invalid channel rejects event buffers",
        buffers == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects idle thresholds",
        wpad::set_idle_thresholds(invalid, 0, 0, 0, 0, 0, 0) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects MotionPlus configuration",
        wpad::set_motion_plus(invalid, 1) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects IR resolution",
        wpad::set_v_res(invalid, 640, 480) == bad_channel,
    );
}

fn probe_valid_channel_argument_guards(logger: &mut ProbeLogger) {
    let chan = wpad::WPAD_CHAN_0;
    let bad_value = wpad::WPAD_ERR_BADVALUE as i32;

    // SAFETY: The API rejects a null event buffer before it reads or writes it.
    let null_events = unsafe { wpad::set_event_bufs(chan, core::ptr::null_mut(), 1) };
    logger.record(
        "WPAD valid channel rejects a null event buffer",
        null_events == bad_value,
    );

    let mut event = wpad::WpadData {
        err: 7,
        data_present: 0x1234,
        ..wpad::WpadData::default()
    };
    // SAFETY: `event` is a valid writable value. The API rejects zero count
    // before it accesses the buffer.
    let zero_events = unsafe { wpad::set_event_bufs(chan, &mut event, 0) };
    logger.record(
        "WPAD valid channel rejects a zero event count without writing",
        zero_events == bad_value && event.err == 7 && event.data_present == 0x1234,
    );

    let mut expansion = 0;
    // SAFETY: `expansion` is a writable output for this query.
    let channel_empty =
        unsafe { wpad::probe(chan, &mut expansion) } == wpad::WPAD_ERR_NO_CONTROLLER as i32;
    let event_buffer = core::ptr::addr_of_mut!(EVENT_BUFFER).cast::<wpad::WpadData>();
    let initial_event = if channel_empty {
        // SAFETY: This buffer has static storage and one writable WpadData
        // slot. The API copies current channel state before it returns.
        unsafe {
            event_buffer.write(wpad::WpadData {
                err: 7,
                data_present: 0x1234,
                ..wpad::WpadData::default()
            });
        }
        // SAFETY: `event_buffer` remains valid for the lifetime of the driver.
        let configured_events = unsafe { wpad::set_event_bufs(chan, event_buffer, 1) };
        if configured_events == wpad::WPAD_ERR_NONE as i32 {
            // SAFETY: No controller is attached, so no asynchronous report can
            // write the slot while the probe reads it.
            Some(unsafe { *event_buffer })
        } else {
            None
        }
    } else {
        None
    };
    logger.record(
        "WPAD empty channel installs and initializes an event buffer",
        channel_empty
            && initial_event.is_some_and(|event| {
                event.err == wpad::WPAD_ERR_NO_CONTROLLER
                    && event.data_present == 0
                    && event.battery_level == 0
            }),
    );

    // SAFETY: The API rejects a null data buffer before it creates a slice.
    let null_stream = unsafe { wpad::send_stream_data(chan, core::ptr::null_mut(), 1) };
    logger.record(
        "WPAD valid channel rejects a null stream buffer",
        null_stream == bad_value,
    );

    let mut byte = 0u8;
    // SAFETY: `byte` is a valid pointer. A zero length requires no readable
    // bytes and is rejected before the stream is scheduled.
    let empty_stream = unsafe { wpad::send_stream_data(chan, (&mut byte as *mut u8).cast(), 0) };
    logger.record(
        "WPAD valid channel rejects a zero stream length",
        empty_stream == bad_value,
    );
}

fn probe_bluetooth_helpers(logger: &mut ProbeLogger) {
    let diagnostics = wpad::bluetooth_diagnostics();
    logger.record(
        "WPAD Bluetooth diagnostics report ready adapter",
        diagnostics.stage == ogc_bt_sync::HostStage::Ready,
    );

    let state = wpad::bluetooth_hid_state(usize::MAX);
    logger.record(
        "WPAD Bluetooth invalid HID slot is empty",
        !state.connected && state.report_count == 0 && state.report_len == 0,
    );
    logger.record(
        "WPAD Bluetooth event queue is empty without HID devices",
        wpad::bluetooth_hid_next_event().is_none(),
    );
    logger.record(
        "WPAD Switch calibration rejects invalid slot",
        wpad::bluetooth_switch_calibration(usize::MAX).is_none(),
    );
}

fn count_bluetooth_poll_mark(stage: &'static str) {
    if stage == "WPAD adapter service state check starting" {
        BLUETOOTH_POLL_MARKS.fetch_add(1, Ordering::AcqRel);
    }
}
