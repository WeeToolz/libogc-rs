use crate::probe_support::ProbeLogger;
use core::sync::atomic::{AtomicU32, Ordering};
use wpad_sys as wpad;

static BLUETOOTH_POLL_MARKS: AtomicU32 = AtomicU32::new(0);
static STATUS_CALLBACK_CHANNELS: AtomicU32 = AtomicU32::new(0);
static STATUS_CALLBACK_INVALID: AtomicU32 = AtomicU32::new(0);
static mut EVENT_BUFFER: core::mem::MaybeUninit<wpad::WpadData> = core::mem::MaybeUninit::uninit();

unsafe extern "C" fn record_status_callback(channel: i32) {
    if let Ok(channel) = u32::try_from(channel)
        && channel < wpad::WPAD_MAX_DEVICES as u32
    {
        STATUS_CALLBACK_CHANNELS.fetch_or(1 << channel, Ordering::AcqRel);
    } else {
        STATUS_CALLBACK_INVALID.fetch_add(1, Ordering::AcqRel);
    }
}

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
    #[cfg(feature = "dolphin-automation")]
    probe_wipe_saved_controllers(logger);

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
    probe_empty_channel_commands(logger);
    probe_empty_channel_stream(logger);
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

    probe_active_search(logger);

    STATUS_CALLBACK_CHANNELS.store(0, Ordering::Release);
    STATUS_CALLBACK_INVALID.store(0, Ordering::Release);
    wpad::set_status_callback(Some(record_status_callback));
    let shutdown = wpad::shutdown();
    wpad::set_status_callback(None);
    logger.record(
        "WPAD status callback receives valid shutdown channel notifications",
        STATUS_CALLBACK_CHANNELS.load(Ordering::Acquire) != 0
            && STATUS_CALLBACK_INVALID.load(Ordering::Acquire) == 0,
    );
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
    logger.record(
        "WPAD disabled adapter rejects data format",
        wpad::set_data_format(wpad::WPAD_CHAN_0, i32::MAX) == wpad::WPAD_ERR_NOT_READY as i32,
    );
    logger.record(
        "WPAD disabled adapter rejects disconnect",
        wpad::disconnect(wpad::WPAD_CHAN_0) == wpad::WPAD_ERR_NOT_READY as i32,
    );
    logger.record(
        "WPAD disabled adapter rejects MotionPlus configuration",
        wpad::set_motion_plus(wpad::WPAD_CHAN_0, 1) == wpad::WPAD_ERR_NOT_READY as i32,
    );
    logger.record(
        "WPAD disabled adapter rejects all-channel MotionPlus configuration",
        wpad::set_motion_plus(wpad::WPAD_CHAN_ALL, 1) == wpad::WPAD_ERR_NOT_READY as i32,
    );
}

fn probe_active_search(logger: &mut ProbeLogger) {
    logger.section("WPAD ACTIVE SEARCH");

    let guest_before = wpad::bluetooth_diagnostics();
    let guest_search = wpad::search();
    let guest_started = wpad::bluetooth_diagnostics();
    logger.record(
        "WPAD guest search enters inquiry state",
        guest_search == wpad::WPAD_ERR_NONE as i32 && guest_started.searching,
    );
    let (_, guest_active) = poll_search_until_adapter_event(guest_before.events, logger);
    logger.note_u32(
        "WPAD guest inquiry adapter events",
        guest_active.events.saturating_sub(guest_before.events),
    );
    logger.note_i32(
        "WPAD guest inquiry active state",
        i32::from(guest_active.searching),
    );
    logger.note_u32(
        "WPAD guest inquiry command error delta",
        guest_active
            .hci_command_failures
            .saturating_sub(guest_before.hci_command_failures),
    );
    logger.note_i32(
        "WPAD guest inquiry HCI status",
        i32::from(guest_active.hci_status.unwrap_or(u8::MAX)),
    );
    logger.note_i32(
        "WPAD guest inquiry failed HCI command",
        i32::from(guest_active.hci_command_failures > guest_before.hci_command_failures),
    );
    logger.note_i32("WPAD status after guest inquiry", wpad::get_status());
    let guest_stop = wpad::stop_search();
    logger.record(
        "WPAD stop_search clears guest inquiry state",
        guest_stop == wpad::WPAD_ERR_NONE as i32 && !wpad::bluetooth_diagnostics().searching,
    );

    let hid_before = wpad::bluetooth_diagnostics();
    let hid_search = wpad::search_bluetooth_hid();
    let hid_started = wpad::bluetooth_diagnostics();
    logger.record(
        "WPAD HID search enters general inquiry state",
        hid_search == wpad::WPAD_ERR_NONE as i32 && hid_started.searching,
    );
    let (_, hid_active) = poll_search_until_adapter_event(hid_before.events, logger);
    logger.note_u32(
        "WPAD HID inquiry adapter events",
        hid_active.events.saturating_sub(hid_before.events),
    );
    logger.note_i32(
        "WPAD HID inquiry active state",
        i32::from(hid_active.searching),
    );
    logger.note_u32(
        "WPAD HID inquiry command error delta",
        hid_active
            .hci_command_failures
            .saturating_sub(hid_before.hci_command_failures),
    );
    logger.note_i32(
        "WPAD HID inquiry HCI status",
        i32::from(hid_active.hci_status.unwrap_or(u8::MAX)),
    );
    logger.note_i32("WPAD status after HID inquiry", wpad::get_status());
    logger.note_i32(
        "WPAD HID inquiry adapter stage",
        host_stage_code(hid_active.stage),
    );
    logger.note_u32("WPAD HID inquiry ACL packet count", hid_active.acl_packets);
    logger.note_u32("WPAD HID inquiry report count", hid_active.reports);
    logger.note_u32(
        "WPAD HID inquiry malformed packet count",
        hid_active.malformed,
    );
    logger.note_u32("WPAD HID inquiry disconnect count", hid_active.disconnects);
    logger.note_i32(
        "WPAD HID inquiry connection status",
        i32::from(hid_active.last_connection_status.unwrap_or(u8::MAX)),
    );
    logger.note_i32(
        "WPAD HID inquiry authentication failure count",
        hid_active.authentication_failures as i32,
    );
    logger.note_u32("WPAD HID inquiry connections", hid_active.connections);
    logger.note_u32(
        "WPAD HID inquiry connection attempts",
        hid_active.connection_attempts,
    );
    logger.note_u32(
        "WPAD HID inquiry connection failures",
        hid_active.connection_failures,
    );
    let mut expansion = 0;
    // SAFETY: The output points to a valid local value.
    let channel_status = unsafe { wpad::probe(wpad::WPAD_CHAN_0, &mut expansion) };
    logger.note_i32("WPAD channel status after HID inquiry", channel_status);
    let hid_input = wpad::bluetooth_hid_state(0);
    logger.note_i32("WPAD HID slot 0 connected", i32::from(hid_input.connected));
    logger.note_i32(
        "WPAD HID slot 0 report length",
        i32::from(hid_input.report_len),
    );
    let cancel_search = wpad::search_bluetooth_hid();
    let cancel_started = wpad::bluetooth_diagnostics();
    let hid_stop = wpad::stop_search();
    let canceled = wpad::bluetooth_diagnostics();
    logger.record(
        "WPAD stop_search clears general inquiry state",
        cancel_search == wpad::WPAD_ERR_NONE as i32
            && cancel_started.searching
            && hid_stop == wpad::WPAD_ERR_NONE as i32
            && !canceled.searching,
    );
}

fn host_stage_code(stage: ogc_bt_sync::HostStage) -> i32 {
    match stage {
        ogc_bt_sync::HostStage::Disabled => 0,
        ogc_bt_sync::HostStage::AdapterOpen => 1,
        ogc_bt_sync::HostStage::Reset => 2,
        ogc_bt_sync::HostStage::BufferSize => 3,
        ogc_bt_sync::HostStage::Configure => 4,
        ogc_bt_sync::HostStage::Address => 5,
        ogc_bt_sync::HostStage::Features => 6,
        ogc_bt_sync::HostStage::Pairing => 7,
        ogc_bt_sync::HostStage::StoredKeys => 8,
        ogc_bt_sync::HostStage::AdapterPatch => 9,
        ogc_bt_sync::HostStage::ScanEnable => 10,
        ogc_bt_sync::HostStage::EventFilter => 11,
        ogc_bt_sync::HostStage::Ready => 12,
        ogc_bt_sync::HostStage::Failed => 13,
        ogc_bt_sync::HostStage::Closing => 14,
    }
}

fn poll_search_until_adapter_event(
    previous_events: u32,
    logger: &mut ProbeLogger,
) -> (bool, ogc_bt_sync::HostDiagnostics) {
    let mut polls_ok = true;
    let mut diagnostics = wpad::bluetooth_diagnostics();
    for _ in 0..60 {
        let result = wpad::scan_pads();
        if result < wpad::WPAD_ERR_NONE as i32 {
            if polls_ok {
                logger.note_i32("WPAD search scan result", result);
            }
            polls_ok = false;
        }
        logger.display().wait_vsync();
        diagnostics = wpad::bluetooth_diagnostics();
        if diagnostics.events > previous_events || !diagnostics.searching {
            break;
        }
    }
    (polls_ok, diagnostics)
}

#[cfg(feature = "dolphin-automation")]
fn probe_wipe_saved_controllers(logger: &mut ProbeLogger) {
    use ogc_conf as conf;

    logger.report_step("WPAD saved controller configuration wipe");
    let mut original_pads = conf::ConfPads::default();
    let mut original_guests = conf::ConfPadGuests::default();
    let read_original = conf::get_pad_devices(&mut original_pads) == 0
        && conf::get_pad_guest_devices(&mut original_guests) == 0;
    if !read_original {
        logger.record("WPAD wipe reads saved controller configuration", false);
        return;
    }

    let mut seeded_pads = original_pads;
    seeded_pads.num_registered = 1;
    seeded_pads.registered[0].bdaddr = [1, 2, 3, 4, 5, 6];
    seeded_pads.registered[0].name = [0x53; 0x40];
    let mut seeded_guests = original_guests;
    seeded_guests.num_guests = 1;
    seeded_guests.guests[0].bdaddr = [6, 5, 4, 3, 2, 1];
    seeded_guests.guests[0].name = [0x47; 0x40];
    seeded_guests.guests[0].link_key = [0xa5; 16];

    let seed_result = conf::set_pad_devices(&seeded_pads);
    let guest_seed_result = conf::set_pad_guest_devices(Some(&seeded_guests));
    let seed_save_result = if seed_result >= 0 && guest_seed_result >= 0 {
        conf::save_changes()
    } else {
        -1
    };
    logger.note_i32("WPAD wipe seed saved pad result", seed_result);
    logger.note_i32("WPAD wipe seed guest result", guest_seed_result);
    logger.note_i32("WPAD wipe seed save result", seed_save_result);
    let mut observed_pads = conf::ConfPads::default();
    let mut observed_guests = conf::ConfPadGuests::default();
    let observed_pads_result = conf::get_pad_devices(&mut observed_pads);
    let observed_guests_result = conf::get_pad_guest_devices(&mut observed_guests);
    logger.note_i32("WPAD wipe verify seeded pads result", observed_pads_result);
    logger.note_i32(
        "WPAD wipe verify seeded guests result",
        observed_guests_result,
    );
    let seed_observed = seed_save_result == 0
        && observed_pads_result == 0
        && observed_guests_result == 0
        && observed_pads == seeded_pads
        && observed_guests == seeded_guests;
    logger.record("WPAD wipe seeds saved controller records", seed_observed);

    let wipe_result = if seed_observed {
        wpad::wipe_saved_controllers()
    } else {
        -1
    };
    let mut cleared_pads = conf::ConfPads::default();
    let mut cleared_guests = conf::ConfPadGuests::default();
    let wipe_cleared_records = wipe_result == wpad::WPAD_ERR_NONE as i32
        && conf::get_pad_devices(&mut cleared_pads) == 0
        && conf::get_pad_guest_devices(&mut cleared_guests) == 0
        && cleared_pads == conf::ConfPads::default()
        && cleared_guests == conf::ConfPadGuests::default();
    logger.record(
        "WPAD wipe clears saved controller and guest records",
        wipe_cleared_records,
    );

    let restore_pads = conf::set_pad_devices(&original_pads);
    let restore_guests = conf::set_pad_guest_devices(Some(&original_guests));
    let restore_save = if restore_pads >= 0 && restore_guests >= 0 {
        conf::save_changes()
    } else {
        -1
    };
    logger.note_i32("WPAD wipe restore pads result", restore_pads);
    logger.note_i32("WPAD wipe restore guests result", restore_guests);
    logger.note_i32("WPAD wipe restore save result", restore_save);
    let mut restored_pads = conf::ConfPads::default();
    let mut restored_guests = conf::ConfPadGuests::default();
    let restored_pads_result = conf::get_pad_devices(&mut restored_pads);
    let restored_guests_result = conf::get_pad_guest_devices(&mut restored_guests);
    logger.note_i32(
        "WPAD wipe verify restored pads result",
        restored_pads_result,
    );
    logger.note_i32(
        "WPAD wipe verify restored guests result",
        restored_guests_result,
    );
    let restored = restore_save == 0
        && restored_pads_result == 0
        && restored_guests_result == 0
        && restored_pads == original_pads
        && restored_guests == original_guests;
    logger.record("WPAD wipe restores original saved records", restored);
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

fn probe_empty_channel_commands(logger: &mut ProbeLogger) {
    let channel = wpad::WPAD_CHAN_0;
    let mut expansion = 0xfeed_beef;
    // SAFETY: The output points to a valid local value.
    let channel_status = unsafe { wpad::probe(channel, &mut expansion) };
    logger.record(
        "WPAD disconnect checks confirm an empty channel",
        channel_status == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );
    logger.record(
        "WPAD empty channel disconnect succeeds",
        channel_status == wpad::WPAD_ERR_NO_CONTROLLER as i32
            && wpad::disconnect(channel) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel speaker control succeeds",
        wpad::control_speaker(channel, 1) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel rumble control succeeds",
        wpad::rumble(channel, 1) == wpad::WPAD_ERR_NONE as i32,
    );

    let invalid = wpad::WPAD_MAX_WIIMOTES as i32;
    let bad_channel = wpad::WPAD_ERR_BAD_CHANNEL as i32;
    logger.record(
        "WPAD invalid channel rejects speaker control",
        wpad::control_speaker(invalid, 1) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel rejects rumble control",
        wpad::rumble(invalid, 1) == bad_channel,
    );
    logger.record(
        "WPAD invalid channel disconnect is rejected",
        wpad::disconnect(invalid) == bad_channel,
    );
}

fn probe_empty_channel_stream(logger: &mut ProbeLogger) {
    let channel = wpad::WPAD_CHAN_0;
    let mut expansion = 0xfeed_beef;
    // SAFETY: The output points to a valid local value.
    let status = unsafe { wpad::probe(channel, &mut expansion) };
    logger.record(
        "WPAD stream probe confirms empty channel",
        status == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );

    let mut payload = [0x5a_u8; 1];
    // SAFETY: The pointer references one readable byte for this call.
    let stream = unsafe { wpad::send_stream_data(channel, payload.as_mut_ptr().cast(), 1) };
    logger.record(
        "WPAD empty channel speaker stream succeeds",
        status == wpad::WPAD_ERR_NO_CONTROLLER as i32 && stream == wpad::WPAD_ERR_NONE as i32,
    );

    // SAFETY: The null and zero-length inputs are rejected before any read.
    let null_stream =
        unsafe { wpad::send_stream_data(channel, core::ptr::null_mut(), payload.len() as u32) };
    // SAFETY: The pointer is valid, and the zero length is rejected before any read.
    let empty_stream = unsafe { wpad::send_stream_data(channel, payload.as_mut_ptr().cast(), 0) };
    logger.record(
        "WPAD empty channel rejects malformed speaker stream arguments",
        null_stream == wpad::WPAD_ERR_BADVALUE as i32
            && empty_stream == wpad::WPAD_ERR_BADVALUE as i32,
    );
}

fn probe_valid_channel_empty_queries(logger: &mut ProbeLogger) {
    let chan = wpad::WPAD_CHAN_0;
    let mut expansion = 0xfeed_beef;
    // SAFETY: The output points to a valid local value.
    let channel_status = unsafe { wpad::probe(chan, &mut expansion) };
    logger.record(
        "WPAD setter checks confirm an empty channel",
        channel_status == wpad::WPAD_ERR_NO_CONTROLLER as i32,
    );

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
        "WPAD empty channel data format update returns success",
        wpad::set_data_format(chan, wpad::WPAD_FMT_BTNS_ACC) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD initialized channel rejects an invalid data format",
        wpad::set_data_format(chan, i32::MAX) == wpad::WPAD_ERR_BADVALUE as i32,
    );
    let resolution_result = wpad::set_v_res(chan, 640, 480);
    let data = wpad::data(chan);
    // SAFETY: WPAD returns a stable pointer to this channel's runtime data.
    let resolution_retained = !data.is_null()
        && unsafe {
            (*data).ir.vres == [640, 480] && (*data).data_present & wpad::WPAD_DATA_IR != 0
        };
    logger.record(
        "WPAD empty channel retains IR resolution before Remote connection",
        resolution_result == wpad::WPAD_ERR_NONE as i32 && resolution_retained,
    );
    logger.record(
        "WPAD empty channel idle thresholds update returns success",
        wpad::set_idle_thresholds(chan, 5, 11, 17, 23, 29, 31) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD empty channel MotionPlus configuration succeeds",
        wpad::set_motion_plus(chan, 1) == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record(
        "WPAD all-channel MotionPlus configuration succeeds without controllers",
        wpad::set_motion_plus(wpad::WPAD_CHAN_ALL, 1) == wpad::WPAD_ERR_NONE as i32,
    );
    let invalid = wpad::WPAD_MAX_WIIMOTES as i32;
    logger.record(
        "WPAD invalid channel MotionPlus configuration is rejected",
        wpad::set_motion_plus(invalid, 1) == wpad::WPAD_ERR_BAD_CHANNEL as i32,
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
