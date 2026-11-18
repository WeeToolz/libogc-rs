use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use ogc_aes_sys as aes;
use ogc_conf as conf;
use ogc_es as es;
use ogc_filesystem as filesystem;
use ogc_ios as ios;
use ogc_lwp::sys as lwp;
use ogc_network_sys as raw_network;
use ogc_sha as sha;
use ogc_stm as stm;
use ogc_sysbase as sysbase;
use ogc_system as system;
#[cfg(feature = "dolphin-automation")]
use ogc_system_sys as raw_system;

use crate::probe_support::ProbeLogger;

#[cfg(feature = "dolphin-automation")]
static ALARM_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "dolphin-automation")]
static ALARM_ID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "dolphin-automation")]
static ALARM_ARG: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(feature = "dolphin-automation")]
unsafe extern "C" fn probe_alarm_callback(id: system::SysWd, arg: *mut core::ffi::c_void) {
    ALARM_ID.store(id, Ordering::Relaxed);
    ALARM_ARG.store(arg as usize as u32, Ordering::Relaxed);
    ALARM_COUNT.fetch_add(1, Ordering::Release);
}

#[repr(align(16))]
struct Align16([u8; 16]);

fn probe_inet_addr(logger: &mut ProbeLogger) {
    let parse = |text: &'static [u8]| unsafe { ogc_network_sys::inet_addr(text.as_ptr().cast()) };
    logger.record(
        "inet_addr dotted decimal",
        parse(b"192.0.2.17\0") == 0xc000_0211,
    );
    logger.record(
        "inet_addr abbreviated hexadecimal",
        parse(b"0xc0.0x000211\0") == 0xc000_0211,
    );
    logger.record(
        "inet_addr rejects overflow",
        parse(b"256.0.2.17\0") == u32::MAX,
    );
    logger.record(
        "inet_addr rejects invalid octal",
        parse(b"192.0.02.08\0") == u32::MAX,
    );
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("RUNTIME PROBE");

    probe_network_error_conversion(logger);
    probe_inet_addr(logger);
    probe_ipv4_values(logger);
    probe_owned_audio_buffer(logger);
    probe_l2cap_packets(logger);
    probe_l2cap_parsers(logger);
    probe_hci_events(logger);
    probe_bt_device_classes(logger);
    probe_bt_input_queue(logger);
    probe_bt_discovery_values(logger);
    probe_bt_l2cap_state(logger);
    probe_bt_host_startup(logger);
    probe_bt_key_requests(logger);
    probe_system_runtime_state(logger);
    probe_system_startup_status(logger);
    probe_system_framebuffer_status(logger);
    probe_ui_adapters(logger);
    probe_system(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_system_alarms(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_raw_system_alarms(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_system_sram_getters(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_system_sram_locks(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_system_extended_sram_getters(logger);
    probe_ios_stm(logger);
    probe_conf(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_conf_fixture(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_conf_typed_values(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_conf_pad_records(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_conf_text_settings(logger);
    probe_es(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_raw_es_catalog(logger);
    probe_crypto(logger);
    probe_iso9660(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_raw_udp(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_async_network_init(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_owned_udp_service(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_owned_tcp_service(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_raw_tcp(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_network_tcp(logger);
    #[cfg(feature = "dolphin-automation")]
    probe_network_udp(logger);
}

#[cfg(feature = "dolphin-automation")]
fn probe_owned_tcp_service(logger: &mut ProbeLogger) {
    use raw_network::std_net as service;
    logger.report_step("Owned TCP service");
    let numeric = service::lookup(b"127.0.0.1");
    let empty = service::lookup(b"");
    let embedded_nul = service::lookup(b"host\0name");
    logger.record(
        "Owned resolver returns loopback and rejects invalid names",
        numeric.is_ok_and(|ips| ips == [0x7f00_0001]) && empty.is_err() && embedded_nul.is_err(),
    );
    let before = service::snapshot();
    let listener = service::Socket::bind(0x7f00_0001, 0, false, owned_network_clock);
    if let Ok(listener) = listener {
        let listen_address = listener.address(false);
        let client = match listen_address {
            Ok((ip, port)) if ip == 0x7f00_0001 && port != 0 => {
                service::Socket::connect(ip, port, owned_network_clock)
            }
            _ => Err(service::NetError::local(22)),
        };
        if let Ok(client) = client {
            let accepted = listener.accept();
            if let Ok((accepted, peer_ip, peer_port)) = accepted {
                let receive_timeout = accepted
                    .set_timeout(false, Some(core::time::Duration::from_secs(3)))
                    .is_ok();
                let client_address = client.address(false);
                let peer = accepted.address(true);
                logger.record(
                    "Owned TCP connect and accept match peer addresses",
                    client_address == Ok((peer_ip, peer_port))
                        && peer_ip == 0x7f00_0001
                        && peer_port != 0
                        && peer == client_address,
                );
                let value = b"owned TCP\0\xff";
                let sent = client.send(value, None);
                let mut output = [0xa5u8; 32];
                let received = if receive_timeout && sent == Ok(value.len()) {
                    accepted.receive(&mut output, false, false)
                } else {
                    Err(service::NetError::local(5))
                };
                logger.record(
                    "Owned TCP connection transfers bytes",
                    received == Ok((value.len(), None))
                        && output[..value.len()] == *value
                        && output[value.len()..].iter().all(|byte| *byte == 0xa5),
                );
                let shutdown = client.shutdown(1);
                let eof = if shutdown.is_ok() {
                    accepted.receive(&mut output, false, false)
                } else {
                    Err(service::NetError::local(5))
                };
                logger.record(
                    "Owned TCP shutdown delivers EOF",
                    shutdown.is_ok() && eof == Ok((0, None)),
                );
                let rejected = client.shutdown(3).is_err();
                logger.record("Owned TCP rejects invalid shutdown mode", rejected);
                let close_accepted = accepted.release();
                let close_client = client.release();
                let close_listener = listener.release();
                let after = service::snapshot();
                let deinit = raw_network::net_deinit();
                logger.record(
                    "Owned TCP closes all sockets",
                    close_accepted.is_ok()
                        && close_client.is_ok()
                        && close_listener.is_ok()
                        && after.live_sockets == before.live_sockets
                        && after.close_failures == before.close_failures
                        && deinit == 0,
                );
                return;
            }
        }
    }
    logger.record("Owned TCP connect and accept match peer addresses", false);
    logger.record("Owned TCP connection transfers bytes", false);
    logger.record("Owned TCP shutdown delivers EOF", false);
    logger.record("Owned TCP rejects invalid shutdown mode", false);
    logger.record("Owned TCP closes all sockets", false);
    logger.note_i32("Owned TCP cleanup result", raw_network::net_deinit());
}

#[cfg(feature = "dolphin-automation")]
fn owned_network_clock() -> Result<core::time::Duration, raw_network::std_net::NetError> {
    let ticks = ogc_core::powerpc::read_timebase();
    Ok(core::time::Duration::from_millis(
        ticks.saturating_mul(1000) / u64::from(lwp::TICKS_PER_SECOND),
    ))
}

#[cfg(feature = "dolphin-automation")]
fn probe_owned_udp_service(logger: &mut ProbeLogger) {
    use core::time::Duration;
    use raw_network::std_net as service;
    logger.report_step("Owned UDP service");
    let initialized = service::initialize(false);
    logger.record(
        "Owned UDP service initializes",
        initialized.is_ok() && raw_network::net_get_status() == 0,
    );
    let before = service::snapshot();
    let sender = service::Socket::bind(0x7f00_0001, 0, true, owned_network_clock);
    let receiver = service::Socket::bind(0x7f00_0001, 0, true, owned_network_clock);
    if let (Ok(sender), Ok(receiver)) = (sender, receiver) {
        let sender_address = sender.address(false);
        let receiver_address = receiver.address(false);
        let live = service::snapshot();
        let addresses_ok = matches!((sender_address, receiver_address), (Ok((0x7f00_0001, a)), Ok((0x7f00_0001, b))) if a != 0 && b != 0 && a != b);
        logger.record(
            "Owned UDP binds distinct loopback sockets",
            addresses_ok && live.live_sockets == before.live_sockets + 2,
        );
        let timeout = Duration::from_secs(3);
        let timeout_ok = receiver.set_timeout(false, Some(timeout)).is_ok()
            && receiver.timeout(false) == Ok(Some(timeout))
            && receiver.set_timeout(true, Some(Duration::ZERO)).is_err();
        logger.record("Owned UDP timeout state validates values", timeout_ok);
        let nonblock_set = receiver.set_nonblocking(true).is_ok();
        let mut bytes = [0xa5u8; 32];
        let empty = receiver.receive(&mut bytes, false, true);
        let nonblock_clear = receiver.set_nonblocking(false).is_ok();
        logger.record(
            "Owned UDP nonblocking empty receive preserves bytes",
            nonblock_set && empty.is_err() && bytes == [0xa5; 32] && nonblock_clear,
        );
        let payload = [0x12u8, 0, 0xfe, 0x80, 0x44];
        let sent = receiver_address
            .is_ok_and(|destination| sender.send(&payload, Some(destination)) == Ok(payload.len()));
        let received = if sent {
            receiver.receive(&mut bytes, false, true)
        } else {
            Err(service::NetError::local(5))
        };
        let source_ok = matches!(received, Ok((5, Some(source))) if Ok(source) == sender_address);
        logger.record(
            "Owned UDP addressed transfer preserves payload and source",
            sent && source_ok && bytes[..5] == payload && bytes[5..] == [0xa5; 27],
        );
        let connected =
            receiver_address.is_ok_and(|(ip, port)| sender.udp_connect(ip, port).is_ok());
        let payload = b"owned connected";
        let sent = connected && sender.send(payload, None) == Ok(payload.len());
        bytes.fill(0xa5);
        let received = if sent {
            receiver.receive(&mut bytes, false, false)
        } else {
            Err(service::NetError::local(5))
        };
        logger.record(
            "Owned UDP connected transfer preserves payload",
            sent && received == Ok((payload.len(), None))
                && bytes[..payload.len()] == *payload
                && bytes[payload.len()..].iter().all(|byte| *byte == 0xa5),
        );
        let busy = raw_network::net_deinit();
        let sender_closed = sender.release();
        let receiver_closed = receiver.release();
        let after = service::snapshot();
        let closed = raw_network::net_deinit();
        logger.record(
            "Owned UDP releases leases and closes sockets",
            busy == -16
                && sender_closed.is_ok()
                && receiver_closed.is_ok()
                && after.live_sockets == before.live_sockets
                && after.close_failures == before.close_failures
                && closed == 0
                && raw_network::net_get_status() == -100,
        );
    } else {
        logger.record("Owned UDP binds distinct loopback sockets", false);
        logger.record("Owned UDP timeout state validates values", false);
        logger.record("Owned UDP nonblocking empty receive preserves bytes", false);
        logger.record(
            "Owned UDP addressed transfer preserves payload and source",
            false,
        );
        logger.record("Owned UDP connected transfer preserves payload", false);
        logger.record("Owned UDP releases leases and closes sockets", false);
        logger.note_i32("Owned UDP cleanup result", raw_network::net_deinit());
    }
}

#[cfg(feature = "dolphin-automation")]
static NETWORK_ASYNC_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "dolphin-automation")]
static NETWORK_ASYNC_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
#[cfg(feature = "dolphin-automation")]
static NETWORK_ASYNC_USERDATA: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

#[cfg(feature = "dolphin-automation")]
unsafe extern "C" fn network_async_callback(result: i32, userdata: *mut core::ffi::c_void) -> i32 {
    NETWORK_ASYNC_RESULT.store(result, Ordering::Relaxed);
    NETWORK_ASYNC_USERDATA.store(userdata as usize as u32, Ordering::Relaxed);
    NETWORK_ASYNC_COUNT.fetch_add(1, Ordering::Release);
    0
}

#[cfg(feature = "dolphin-automation")]
fn probe_async_network_init(logger: &mut ProbeLogger) {
    use ogc_network as net;
    use raw_network as raw;
    const TOKEN: usize = 0x1248;
    for wrapper in [false, true] {
        NETWORK_ASYNC_COUNT.store(0, Ordering::Relaxed);
        NETWORK_ASYNC_RESULT.store(i32::MIN, Ordering::Relaxed);
        NETWORK_ASYNC_USERDATA.store(0, Ordering::Relaxed);
        let start = if wrapper {
            unsafe {
                net::init_async(
                    Some(network_async_callback),
                    TOKEN as *mut core::ffi::c_void,
                )
            }
        } else {
            unsafe {
                raw::net_init_async(
                    Some(network_async_callback),
                    TOKEN as *mut core::ffi::c_void,
                )
            }
        };
        for _ in 0..1000 {
            if NETWORK_ASYNC_COUNT.load(Ordering::Acquire) != 0 {
                break;
            }
            lwp::sleep_ticks((lwp::TICKS_PER_SECOND / 100).max(1));
        }
        let completed = start == 0
            && NETWORK_ASYNC_COUNT.load(Ordering::Acquire) == 1
            && NETWORK_ASYNC_RESULT.load(Ordering::Relaxed) == 0
            && NETWORK_ASYNC_USERDATA.load(Ordering::Relaxed) == TOKEN as u32
            && raw::net_get_status() == 0;
        let socket = if completed {
            raw::net_socket(raw::PF_INET, raw::SOCK_DGRAM, 0)
        } else {
            -1
        };
        let closed = socket >= 0 && raw::net_close(socket) == 0;
        let deinit = raw::net_deinit();
        let passed = completed && closed && deinit == 0 && raw::net_get_status() == -100;
        logger.record(
            if wrapper {
                "Network wrapper asynchronous initialization completes"
            } else {
                "Raw network asynchronous initialization completes"
            },
            passed,
        );
    }
}

fn probe_network_error_conversion(logger: &mut ProbeLogger) {
    use ogc_network as net;
    use ogc_network_sys as raw;
    logger.record(
        "Network error conversion preserves nonnegative results",
        raw::_net_convert_error(0) == 0
            && raw::_net_convert_error(17) == 17
            && net::convert_error(17) == 17,
    );
    logger.record(
        "Network error conversion maps IOS errors",
        raw::_net_convert_error(-3) == -112
            && raw::_net_convert_error(-6) == -11
            && raw::_net_convert_error(-28) == -22
            && net::convert_error(-3) == -112,
    );
    logger.record(
        "Network error conversion offsets unknown values",
        raw::_net_convert_error(-78) == -10078 && net::convert_error(-100) == -10100,
    );
}

fn probe_system_framebuffer_status(logger: &mut ProbeLogger) {
    // SAFETY: A null argument requests the video subsystem's live preferred mode.
    let mode = unsafe { ogc_video::get_preferred_mode(core::ptr::null_mut()) };
    if mode.is_null() {
        logger.record(
            "System framebuffer snapshot matches preferred mode allocation",
            false,
        );
        return;
    }
    // SAFETY: The preferred mode remains valid for the probe lifetime.
    let (width, height, aa) = unsafe { ((*mode).fb_width, (*mode).xfb_height, (*mode).aa) };
    let expected = (usize::from(width).div_ceil(16) * 16)
        * (usize::from(height) + if aa != 0 { 4 } else { 0 })
        * 2;
    let snapshot = system::snapshot_framebuffer();
    logger.record(
        "System framebuffer snapshot matches preferred mode allocation",
        expected > 0
            && snapshot.last_request_rmode == mode as usize
            && snapshot.last_request_bytes as usize == expected
            && snapshot.last_bytes == expected
            && system::last_framebuffer_bytes() == expected,
    );
}

fn probe_system_startup_status(logger: &mut ProbeLogger) {
    use system::startup::{self, InitError};
    let pre_main = system::snapshot_pre_main();
    let startup = system::snapshot_startup();
    logger.record(
        "System startup snapshots report completed initialization",
        pre_main.called
            && pre_main.completed
            && pre_main.failed_stage == 0
            && pre_main.result == 0
            && startup.init_completed
            && startup.init_failed_stage == 0
            && startup.init_completed_mask == 0x7ff
            && system::runtime_fp_disabled_stage() == 0,
    );
    logger.record(
        "System startup validation preserves failed stage and incomplete state",
        startup::validate(true, 0) == Ok(())
            && startup::validate(false, 0) == Err(InitError::NotCompleted)
            && startup::validate(false, 7) == Err(InitError::FailedStage(7))
            && startup::validate(true, 9) == Err(InitError::FailedStage(9)),
    );
    let before = system::snapshot_startup();
    let checked = system::init_checked();
    let after = system::snapshot_startup();
    logger.record(
        "Checked initialization preserves completed startup",
        checked == Ok(())
            && before.init_completed
            && before.init_failed_stage == 0
            && after.init_completed
            && after.init_failed_stage == 0
            && after.init_completed_mask == before.init_completed_mask
            && after.init_stage == before.init_stage,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_system_extended_sram_getters(logger: &mut ProbeLogger) {
    let sram = unsafe { system::lock_sram_ex() };
    if sram.is_null() {
        logger.record("Extended SRAM fixture lock and restoration", false);
        return;
    }
    // SAFETY: The lock owns the packed buffer until the paired unlock. Field
    // writes use unaligned access and the original image is restored below.
    let original = unsafe { *sram };
    unsafe {
        core::ptr::write_unaligned(
            core::ptr::addr_of_mut!((*sram).wireless_pad_id),
            [0x1234, 0xabcd, 0, 0xff00],
        );
        core::ptr::write_unaligned(core::ptr::addr_of_mut!((*sram).gbs), 0x2945);
    }
    let released = unsafe { system::unlock_sram_ex(false) };
    logger.record(
        "Wireless ID getter reads each extended SRAM channel",
        released
            && system::get_wireless_id(0) == 0x1234
            && system::get_wireless_id(1) == 0xabcd
            && system::get_wireless_id(2) == 0
            && system::get_wireless_id(3) == 0xff00
            && system::get_wireless_id(4) == 0
            && system::get_wireless_id(u32::MAX) == 0,
    );
    logger.record(
        "GBS getter reads extended SRAM",
        system::get_gbs_mode() == 0x2945,
    );
    let restore = unsafe { system::lock_sram_ex() };
    let restored = if restore.is_null() {
        false
    } else {
        unsafe {
            *restore = original;
            system::unlock_sram_ex(false)
        }
    };
    let checked = system::snapshot_sram_ex() == Some(original);
    logger.record(
        "Extended SRAM fixture lock and restoration",
        restored && checked,
    );
}

fn probe_system_runtime_state(logger: &mut ProbeLogger) {
    use system::{
        MainLoopDecision, MainLoopExitReason, PowerButtonState, StmEvent, SystemRuntimeState,
    };
    let mut wii = SystemRuntimeState::new(true);
    let initial = wii.is_wii
        && !wii.main_loop_done
        && wii.power_enable_time == 0
        && wii.power_button_state == PowerButtonState::Idle
        && wii.main_loop_step(true) == MainLoopDecision::Continue;
    wii.note_stm_event(StmEvent::Reset, false);
    let ignored = wii.power_button_state == PowerButtonState::Idle;
    wii.note_stm_event(StmEvent::Reset, true);
    wii.note_stm_event(StmEvent::Power, false);
    let exit = wii.power_button_state == PowerButtonState::ResetPressed
        && wii.main_loop_step(false) == MainLoopDecision::Exit(MainLoopExitReason::PowerButton)
        && wii.main_loop_step(false) == MainLoopDecision::Exit(MainLoopExitReason::AlreadyDone);
    let mut gc = SystemRuntimeState::new(false);
    let gc_exit = gc.main_loop_step(false) == MainLoopDecision::Continue
        && gc.main_loop_step(true) == MainLoopDecision::Exit(MainLoopExitReason::ResetButton);
    logger.record(
        "System runtime state tracks Wii and GameCube input",
        initial && ignored && exit && gc_exit,
    );

    let mut throttle = SystemRuntimeState::new(true);
    let first = throttle.power_callback_attempt(100, 50) && throttle.power_enable_time == 150;
    let blocked = !throttle.power_callback_attempt(120, 50) && throttle.power_enable_time == 170;
    let due = throttle.power_callback_attempt(170, 50) && throttle.power_enable_time == 220;
    logger.record(
        "System runtime power cooldown extends after attempts",
        first && blocked && due,
    );

    let gc_plan = gc.reset_plan(system::SYS_HOTRESET, true, false);
    let wii_plan = wii.reset_plan(system::SYS_POWEROFF, false, true);
    logger.record(
        "System runtime reset plan reflects target and requested action",
        gc_plan.terminal == Some(system::ResetSystemTerminal::HotReset)
            && gc_plan.sets_boot_ipl
            && !gc_plan.clears_active_input_list
            && wii_plan.terminal == Some(system::ResetSystemTerminal::ShutdownToIdle)
            && wii_plan.clears_active_input_list
            && !wii_plan.sets_boot_ipl,
    );
}

fn probe_bt_key_requests(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{BluetoothAddress, HidHost, Outgoing};

    let mut host = HidHost::new();
    host.load_link_key(BluetoothAddress::new([1, 2, 3, 4, 5, 6]), [0xa5; 16]);
    host.reset_preserving_keys();
    host.receive_event(&[0x17, 6, 1, 2, 3, 4, 5, 6], 1);
    host.start();
    let matched = matches!(host.next_outgoing(2), Some(Outgoing::Hci { opcode, params })
        if opcode.raw() == 0x040b && params.len() == 22
            && params[..6] == [1, 2, 3, 4, 5, 6] && params[6..] == [0xa5; 16])
        && host.diagnostics().link_key_requests == 1
        && host.diagnostics().link_key_matches == 1;
    let mut unknown = HidHost::new();
    unknown.receive_event(&[0x17, 6, 6, 5, 4, 3, 2, 1], 1);
    unknown.start();
    let rejected = matches!(unknown.next_outgoing(2), Some(Outgoing::Hci { opcode, params })
        if opcode.raw() == 0x040c && params == [6, 5, 4, 3, 2, 1])
        && unknown.diagnostics().link_key_misses == 1;
    let before = unknown.diagnostics();
    unknown.receive_event(&[0x17, 6, 6], 3);
    let malformed = unknown.diagnostics().malformed == before.malformed + 1
        && unknown.diagnostics().link_key_requests == before.link_key_requests;
    logger.record(
        "Bluetooth link key requests preserve keys and reject unknown peers",
        matched && rejected && malformed,
    );
}

fn probe_bt_host_startup(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{Failure, HidEvent, HidHost, HostStage, Outgoing};

    let mut host = HidHost::new();
    let initial = host.diagnostics().stage == HostStage::Disabled
        && host.peek_event().is_none()
        && host.next_event().is_none()
        && host.next_outgoing(0).is_none()
        && !host.search(0)
        && host.switch_calibration(0).is_none();
    host.adapter_opening();
    let opening = host.diagnostics().stage == HostStage::AdapterOpen;
    host.start();
    let reset = host.diagnostics().stage == HostStage::Reset
        && matches!(host.next_outgoing(1), Some(Outgoing::Hci { opcode, params })
            if opcode.raw() == 0x0c03 && params.is_empty())
        && host.next_outgoing(2).is_none();
    host.fail_timeout();
    let expected = Some(HidEvent::Failed {
        slot: None,
        failure: Failure::Timeout,
    });
    let failed = host.diagnostics().stage == HostStage::Failed
        && host.diagnostics().failed_stage == Some(HostStage::Reset)
        && host.diagnostics().failure == Some(Failure::Timeout)
        && host.peek_event() == expected
        && host.peek_event() == expected
        && host.next_event() == expected
        && host.next_event().is_none()
        && host.next_outgoing(3).is_none();
    host.reset_preserving_keys();
    let reset_state = host.diagnostics().stage == HostStage::Disabled
        && host.diagnostics().failure.is_none()
        && host.peek_event().is_none();
    host.fail_transport(-7);
    let transport = host.diagnostics().failure == Some(Failure::Transport(-7))
        && host.next_event()
            == Some(HidEvent::Failed {
                slot: None,
                failure: Failure::Transport(-7),
            });
    logger.record(
        "Bluetooth host startup and failure lifecycle",
        initial && opening && reset && failed && reset_state && transport,
    );
}

fn probe_bt_l2cap_state(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{HidL2capState, L2capHidChannelState, L2capSignal};

    let mut channel = L2capHidChannelState::new(0x11, 0x40);
    let initial = channel.psm == 0x11
        && channel.local_cid == 0x40
        && channel.remote_cid == 0
        && !L2capHidChannelState::ready(channel);
    channel.remote_cid = 0x70;
    channel.active = true;
    channel.connected = true;
    channel.local_configured = true;
    channel.remote_configured = true;
    let complete = L2capHidChannelState::ready(channel);
    channel.remote_cid = 0;
    logger.record(
        "L2CAP channel readiness requires a remote channel",
        initial && complete && !L2capHidChannelState::ready(channel),
    );

    let mut state = HidL2capState::new();
    let empty = !HidL2capState::ready(state)
        && state.control.local_cid == 0x40
        && state.interrupt.local_cid == 0x41;
    let conn_control = [0x70, 0, 0x40, 0, 0, 0, 0, 0];
    let continuation = [0x40, 0, 1, 0];
    let config_control = [0x40, 0, 0, 0];
    let reply_control = [0x70, 0, 0, 0, 0, 0];
    let conn_interrupt = [0x71, 0, 0x41, 0, 0, 0, 0, 0];
    let config_interrupt = [0x41, 0, 0, 0];
    let reply_interrupt = [0x71, 0, 0, 0, 0, 0];
    let signal = |code, payload| L2capSignal {
        code,
        identifier: 1,
        payload,
    };
    let first = state.observe_signal(signal(3, &conn_control)) == Ok(())
        && state.control.remote_cid == 0x70
        && !HidL2capState::ready(state)
        && state.observe_signal(signal(4, &continuation)) == Ok(())
        && !state.control.remote_configured;
    let control = state.observe_signal(signal(4, &config_control)) == Ok(())
        && state.observe_signal(signal(5, &reply_control)) == Ok(())
        && L2capHidChannelState::ready(state.control)
        && !HidL2capState::ready(state);
    let interrupt = state.observe_signal(signal(3, &conn_interrupt)) == Ok(())
        && state.observe_signal(signal(4, &config_interrupt)) == Ok(())
        && state.observe_signal(signal(5, &reply_interrupt)) == Ok(())
        && HidL2capState::ready(state);
    let before = state;
    let malformed = state.observe_signal(signal(3, &[0x70])).is_err() && state == before;
    logger.record(
        "L2CAP HID state tracks both configured channels",
        empty && first && control && interrupt && malformed,
    );
}

fn probe_bt_discovery_values(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{ControllerFamily as Family, Error, HciEndpointAddresses};

    let names = [
        (&b"Nintendo RVL-CNT-01-TR"[..], Family::WiiRemote),
        (&b"Joy-Con (L)\0suffix"[..], Family::JoyConLeft),
        (&b"Nintendo Joy-Con (R)"[..], Family::JoyConRight),
        (&b"Nintendo Pro Controller"[..], Family::SwitchPro),
        (&b"xbox wireless controller"[..], Family::Xbox),
        (&b"Fake Pro Controller"[..], Family::Generic),
        (&b""[..], Family::Generic),
    ];
    let classified = names
        .iter()
        .all(|(name, expected)| Family::from_name(name) == *expected);
    let switches = Family::is_switch(Family::SwitchPro)
        && Family::is_switch(Family::JoyConLeft)
        && Family::is_switch(Family::JoyConRight)
        && !Family::is_switch(Family::WiiRemote)
        && !Family::is_switch(Family::Xbox)
        && !Family::is_switch(Family::Generic);
    logger.record(
        "Bluetooth discovery classifies controller names",
        classified && switches,
    );

    let expected = HciEndpointAddresses {
        acl_out: 2,
        event_in: 0x81,
        acl_in: 0x82,
    };
    let valid = HciEndpointAddresses::new(2, 0x81, 0x82) == Ok(expected);
    let invalid = [
        (0, 0x81, 0x82),
        (0x82, 0x81, 0x82),
        (2, 1, 0x82),
        (2, 0x81, 2),
        (2, 0x81, 0x81),
        (0x12, 0x81, 0x82),
    ];
    logger.record(
        "Bluetooth HCI endpoints validate direction and distinct inputs",
        valid
            && invalid.iter().all(|&(out, event, input)| {
                HciEndpointAddresses::new(out, event, input) == Err(Error::InvalidEndpoint)
            }),
    );
}

fn probe_bt_input_queue(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{
        BLUETOOTH_INPUT_QUEUE_CAPACITY, BluetoothAddress, BluetoothInputKind, BluetoothInputQueue,
        BluetoothInputQueueError, ControllerFamily,
    };

    let mut queue = BluetoothInputQueue::new();
    let initial = queue.is_empty()
        && !queue.is_full()
        && queue.len() == 0
        && queue.active_generation(0).is_none()
        && queue.pop().is_none();
    let address = BluetoothAddress::new([1, 2, 3, 4, 5, 6]);
    let generation = queue.push_connected(0, address, 0x123, ControllerFamily::Generic, None);
    let connected = generation == Ok(1)
        && queue.active_generation(0) == Some(1)
        && queue.len() == 1
        && queue.push_connected(0, address, 0x123, ControllerFamily::Generic, None)
            == Err(BluetoothInputQueueError::AlreadyConnected);
    let report = queue.push_report(0, 1, &[0x30, 0xff, 0x00]) == Ok(())
        && queue.push_report(0, 2, &[1]) == Err(BluetoothInputQueueError::StaleGeneration)
        && queue.len() == 2;
    let first = queue.pop();
    let second = queue.pop();
    let ordered = matches!(first, Some(event) if event.slot == 0 && event.generation == 1
        && matches!(event.kind, BluetoothInputKind::Connected { address: a, handle: 0x123, family: ControllerFamily::Generic, calibration: None } if a == address))
        && matches!(second, Some(event) if event.slot == 0 && event.generation == 1
            && event.kind == BluetoothInputKind::Report { len: 3 }
            && event.bytes[..3] == [0x30, 0xff, 0]
            && event.bytes[3..].iter().all(|byte| *byte == 0));
    let disconnected = queue.push_disconnected(0, 1, 0x13) == Ok(())
        && queue.active_generation(0).is_none()
        && matches!(queue.pop(), Some(event) if event.generation == 1
            && event.kind == BluetoothInputKind::Disconnected { reason: 0x13 })
        && queue.is_empty();
    logger.record(
        "Bluetooth queue preserves connection report and disconnect order",
        initial && connected && report && ordered && disconnected,
    );

    let next = queue.push_connected(0, address, 0x123, ControllerFamily::Generic, None);
    let mut filled = next == Ok(2);
    for value in 0..BLUETOOTH_INPUT_QUEUE_CAPACITY - 1 {
        filled &= queue.push_report(0, 2, &[value as u8]) == Ok(());
    }
    let full = filled
        && queue.is_full()
        && queue.len() == BLUETOOTH_INPUT_QUEUE_CAPACITY
        && queue.push_disconnected(0, 2, 0x13) == Err(BluetoothInputQueueError::Full)
        && queue.active_generation(0) == Some(2);
    let released = queue.pop().is_some()
        && queue.push_disconnected(0, 2, 0x13) == Ok(())
        && queue.active_generation(0).is_none()
        && queue.is_full();
    logger.record(
        "Bluetooth queue applies backpressure without losing state",
        full && released,
    );
}

fn probe_bt_device_classes(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{BluetoothAddress, ClassOfDevice};

    let address = BluetoothAddress::new([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
    logger.record(
        "Bluetooth address preserves HCI byte order",
        BluetoothAddress::as_raw(&address) == [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc],
    );
    let controller = ClassOfDevice::new([0x5c, 0xa5, 0xff]);
    let phone = ClassOfDevice::new([0x94, 0x02, 0x00]);
    logger.record(
        "Bluetooth class extracts major and minor fields",
        ClassOfDevice::major(controller) == 5
            && ClassOfDevice::minor(controller) == 23
            && ClassOfDevice::major(phone) == 2
            && ClassOfDevice::minor(phone) == 37,
    );
    logger.record(
        "Bluetooth class recognizes peripheral devices",
        ClassOfDevice::likely_game_controller(controller)
            && !ClassOfDevice::likely_game_controller(phone),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_raw_udp(logger: &mut ProbeLogger) {
    use core::mem::size_of;
    use raw_network as net;

    logger.report_step("Raw UDP loopback operations");
    let initialized = net::net_init();
    logger.record(
        "Raw network initializes and reports status",
        initialized >= 0 && net::net_get_status() == initialized,
    );
    if initialized < 0 {
        return;
    }
    probe_network_mac_address(logger);
    probe_network_interface_addresses(logger);
    probe_network_numeric_resolver(logger);
    let sender = net::net_socket(net::PF_INET, net::SOCK_DGRAM, net::IPPROTO_IP);
    let receiver = net::net_socket(net::PF_INET, net::SOCK_DGRAM, net::IPPROTO_IP);
    logger.record("Raw UDP sockets open", sender >= 0 && receiver >= 0);
    if sender >= 0 && receiver >= 0 {
        let mut bind = net::SockaddrIn {
            sin_len: size_of::<net::SockaddrIn>() as u8,
            sin_family: net::AF_INET as u8,
            sin_port: 0,
            sin_addr: net::InAddr {
                s_addr: 0x7f00_0001,
            },
            sin_zero: [0; 8],
        };
        let addrlen = size_of::<net::SockaddrIn>() as u32;
        // SAFETY: All address and data buffers remain live for each call.
        let bound = unsafe {
            net::net_bind(sender, (&mut bind as *mut net::SockaddrIn).cast(), addrlen) == 0
                && net::net_bind(
                    receiver,
                    (&mut bind as *mut net::SockaddrIn).cast(),
                    addrlen,
                ) == 0
        };
        logger.record("Raw UDP sockets bind loopback", bound);
        if bound {
            let mut source = net::SockaddrIn::default();
            let mut destination = net::SockaddrIn::default();
            let mut source_len = addrlen;
            let mut destination_len = addrlen;
            let names = unsafe {
                net::net_getsockname(
                    sender,
                    (&mut source as *mut net::SockaddrIn).cast(),
                    &mut source_len,
                ) == 0
                    && net::net_getsockname(
                        receiver,
                        (&mut destination as *mut net::SockaddrIn).cast(),
                        &mut destination_len,
                    ) == 0
            } && source.sin_addr.s_addr == 0x7f00_0001
                && destination.sin_addr.s_addr == 0x7f00_0001
                && source.sin_port != 0
                && destination.sin_port != 0
                && source.sin_port != destination.sin_port;
            logger.record("Raw UDP names report distinct loopback ports", names);
            if names {
                let invalid_payload = [0x17u8, 0x42, 0x80];
                let invalid_send = unsafe {
                    net::net_sendto(
                        -1,
                        invalid_payload.as_ptr().cast(),
                        invalid_payload.len() as i32,
                        0,
                        (&mut destination as *mut net::SockaddrIn).cast(),
                        addrlen,
                    )
                };
                logger.record(
                    "Raw UDP invalid descriptor rejects sendto",
                    invalid_send < 0,
                );
                let mut invalid_buffer = [0xa5u8; 8];
                let mut invalid_peer = net::SockaddrIn::default();
                let mut invalid_len = addrlen;
                let invalid_receive = unsafe {
                    net::net_recvfrom(
                        -1,
                        invalid_buffer.as_mut_ptr().cast(),
                        invalid_buffer.len() as i32,
                        net::MSG_DONTWAIT,
                        (&mut invalid_peer as *mut net::SockaddrIn).cast(),
                        &mut invalid_len,
                    )
                };
                logger.record(
                    "Raw UDP invalid descriptor preserves recvfrom outputs",
                    invalid_receive < 0
                        && invalid_buffer == [0xa5; 8]
                        && invalid_peer == net::SockaddrIn::default()
                        && invalid_len == addrlen,
                );
                probe_lwip_readiness(logger, sender, receiver, &mut destination);
                let mut empty = [0xa5; 16];
                let mut empty_peer = net::SockaddrIn::default();
                let mut empty_len = addrlen;
                // SAFETY: The data and address buffers are writable for the call.
                let empty_result = unsafe {
                    net::net_recvfrom(
                        receiver,
                        empty.as_mut_ptr().cast(),
                        empty.len() as i32,
                        net::MSG_DONTWAIT,
                        (&mut empty_peer as *mut net::SockaddrIn).cast(),
                        &mut empty_len,
                    )
                };
                logger.note_i32("Raw UDP empty recvfrom result", empty_result);
                logger.note_u32("Raw UDP empty recvfrom address length", empty_len);
                logger.record(
                    "Raw UDP recvfrom leaves outputs unchanged on would-block",
                    empty_result < 0
                        && empty == [0xa5; 16]
                        && empty_peer == net::SockaddrIn::default()
                        && empty_len == addrlen,
                );
                let payload = [0, 0xff, 0x52, 0x41, 0x57, 0x80];
                let sent = unsafe {
                    net::net_sendto(
                        sender,
                        payload.as_ptr().cast(),
                        payload.len() as i32,
                        0,
                        (&mut destination as *mut net::SockaddrIn).cast(),
                        addrlen,
                    )
                };
                let mut poll = net::PollSd {
                    socket: receiver,
                    events: 3,
                    revents: 0,
                };
                let ready = sent == payload.len() as i32
                    && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                    && poll.revents & 3 != 0;
                logger.record("Raw UDP sendto and poll deliver readiness", ready);
                if ready {
                    let mut bytes = [0xa5; 32];
                    let mut peer = net::SockaddrIn::default();
                    let mut peer_len = addrlen;
                    let got = unsafe {
                        net::net_recvfrom(
                            receiver,
                            bytes.as_mut_ptr().cast(),
                            bytes.len() as i32,
                            0,
                            (&mut peer as *mut net::SockaddrIn).cast(),
                            &mut peer_len,
                        )
                    };
                    logger.note_i32("Raw UDP recvfrom count", got);
                    logger.note_u32("Raw UDP recvfrom peer length", peer_len);
                    logger.note_hex32("Raw UDP expected peer address", source.sin_addr.s_addr);
                    logger.note_hex32("Raw UDP actual peer address", peer.sin_addr.s_addr);
                    logger.note_u32("Raw UDP expected peer port", u32::from(source.sin_port));
                    logger.note_u32("Raw UDP actual peer port", u32::from(peer.sin_port));
                    logger.note_hex32(
                        "Raw UDP received first four bytes",
                        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                    );
                    logger.note_hex32(
                        "Raw UDP received next four bytes",
                        u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                    );
                    logger.record(
                        "Raw UDP recvfrom preserves bytes and sender",
                        got == payload.len() as i32
                            && bytes[..payload.len()] == payload
                            && bytes[payload.len()..].iter().all(|b| *b == 0xa5)
                            && peer.sin_port == source.sin_port
                            && peer.sin_addr.s_addr == source.sin_addr.s_addr,
                    );
                }
                probe_lwip_udp_addressed(logger, sender, receiver, &source, &mut destination);
                let connected = unsafe {
                    net::net_connect(
                        sender,
                        (&mut destination as *mut net::SockaddrIn).cast(),
                        addrlen,
                    )
                } == 0;
                logger.record("Raw UDP connect succeeds", connected);
                if connected {
                    let payload = b"raw connected";
                    let sent = unsafe {
                        net::net_send(sender, payload.as_ptr().cast(), payload.len() as i32, 0)
                    };
                    let mut poll = net::PollSd {
                        socket: receiver,
                        events: 3,
                        revents: 0,
                    };
                    let ready = sent == payload.len() as i32
                        && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                        && poll.revents & 3 != 0;
                    let mut bytes = [0xa5; 32];
                    let got = if ready {
                        unsafe {
                            net::net_recv(
                                receiver,
                                bytes.as_mut_ptr().cast(),
                                bytes.len() as i32,
                                0,
                            )
                        }
                    } else {
                        -1
                    };
                    logger.record(
                        "Raw UDP send and recv preserve connected datagram",
                        got == payload.len() as i32
                            && bytes[..payload.len()] == *payload
                            && bytes[payload.len()..].iter().all(|b| *b == 0xa5),
                    );
                    let payload = b"raw write";
                    let sent = unsafe {
                        net::net_write(sender, payload.as_ptr().cast(), payload.len() as i32)
                    };
                    let mut poll = net::PollSd {
                        socket: receiver,
                        events: 3,
                        revents: 0,
                    };
                    let ready = sent == payload.len() as i32
                        && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                        && poll.revents & 3 != 0;
                    let mut bytes = [0xa5; 32];
                    let got = if ready {
                        unsafe {
                            net::net_read(receiver, bytes.as_mut_ptr().cast(), bytes.len() as i32)
                        }
                    } else {
                        -1
                    };
                    logger.record(
                        "Raw UDP write and read preserve connected datagram",
                        got == payload.len() as i32
                            && bytes[..payload.len()] == *payload
                            && bytes[payload.len()..].iter().all(|b| *b == 0xa5),
                    );
                    probe_lwip_udp_connected(logger, sender, receiver);
                }
            }
        }
    }
    if sender >= 0 {
        logger.record("Raw UDP sender closes", net::net_close(sender) == 0);
    }
    if receiver >= 0 {
        logger.record("Raw UDP receiver closes", net::net_close(receiver) == 0);
    }
    let deinitialized = net::net_deinit();
    logger.record(
        "Raw network deinitializes",
        deinitialized == 0 && net::net_get_status() == -100,
    );
    let closed = unsafe { net::net_gethostbyname(c"127.0.0.1".as_ptr()) };
    logger.record(
        "Network resolver requires an active session",
        deinitialized == 0 && closed.is_null(),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_network_mac_address(logger: &mut ProbeLogger) {
    use ogc_ipc as ipc;
    use raw_network as raw;
    let mut ios_mac = [0u8; 6];
    let mut status = [0u8; 32];
    let reference = match ipc::open_file(c"/dev/net/ncd/manage", 0) {
        Ok(fd) => {
            let rc = fd.ioctlv(
                8,
                0,
                [
                    ipc::IoctlvEntry::Out(&mut status),
                    ipc::IoctlvEntry::Out(&mut ios_mac),
                ],
            );
            let close = fd.close();
            rc == 0 && status[..4] == [0; 4] && close.is_ok()
        }
        Err(_) => false,
    };
    let mut guarded = [0xa5u8; 8];
    let rc = unsafe { raw::net_get_mac_address(guarded.as_mut_ptr().add(1).cast()) };
    logger.record(
        "Network MAC matches independent IOS response",
        reference
            && rc == 0
            && ios_mac != [0; 6]
            && ios_mac[0] & 1 == 0
            && guarded[1..7] == ios_mac
            && guarded[0] == 0xa5
            && guarded[7] == 0xa5,
    );
    let null_rc = unsafe { raw::net_get_mac_address(core::ptr::null_mut()) };
    logger.record("Network MAC rejects null output", null_rc == -1);
}

#[cfg(feature = "dolphin-automation")]
fn probe_network_numeric_resolver(logger: &mut ProbeLogger) {
    use ogc_network as net;
    use raw_network as raw;
    let null_result = unsafe { raw::net_gethostbyname(core::ptr::null()) };
    let empty_result = unsafe { net::get_host_by_name(c"".as_ptr()) };
    logger.record(
        "Network resolver rejects null and empty names",
        null_result.is_null() && empty_result.is_null(),
    );
    let result = unsafe { raw::net_gethostbyname(c"127.0.0.1".as_ptr()) };
    let matches = if result.is_null() {
        false
    } else {
        unsafe {
            let entry = &*result;
            if entry.h_addrtype != 2
                || entry.h_length != 4
                || entry.h_addr_list.is_null()
                || (*entry.h_addr_list).is_null()
            {
                false
            } else {
                core::slice::from_raw_parts((*entry.h_addr_list).cast::<u8>(), 4) == [127, 0, 0, 1]
                    && (*entry.h_addr_list.add(1)).is_null()
            }
        }
    };
    logger.record("Network resolver returns numeric loopback address", matches);
    let result = unsafe { net::get_host_by_name(c"127.0.0.1".as_ptr()) };
    let matches = if result.is_null() {
        false
    } else {
        unsafe {
            let entry = &*result;
            if entry.h_addrtype != 2
                || entry.h_length != 4
                || entry.h_addr_list.is_null()
                || (*entry.h_addr_list).is_null()
            {
                false
            } else {
                core::slice::from_raw_parts((*entry.h_addr_list).cast::<u8>(), 4) == [127, 0, 0, 1]
            }
        }
    };
    logger.record("Network resolver wrapper returns numeric loopback", matches);
}

#[cfg(feature = "dolphin-automation")]
fn probe_network_interface_addresses(logger: &mut ProbeLogger) {
    use ogc_network as net;
    use raw_network as raw;
    let mut address = raw::InAddr {
        s_addr: 0xa5a5_a5a5,
    };
    let mut mask = raw::InAddr {
        s_addr: 0xa5a5_a5a5,
    };
    let unsupported =
        unsafe { raw::if_configex(&mut address, &mut mask, core::ptr::null_mut(), false, 1) };
    logger.record(
        "Network static configuration rejects without writes",
        unsupported == -22 && address.s_addr == 0xa5a5_a5a5 && mask.s_addr == 0xa5a5_a5a5,
    );
    let result =
        unsafe { raw::if_configex(&mut address, &mut mask, core::ptr::null_mut(), true, 1) };
    let host = raw::net_gethostip();
    logger.record(
        "Network DHCP address matches IOS host address",
        result == 0 && address.s_addr != 0 && address.s_addr == host && mask.s_addr != 0,
    );
    let mut text = [0xa5u8; 16];
    let text_result = unsafe {
        raw::if_config(
            text.as_mut_ptr().cast(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            true,
            1,
        )
    };
    let parsed = if text.contains(&0) {
        unsafe { raw::inet_addr(text.as_ptr().cast()) }
    } else {
        u32::MAX
    };
    logger.record(
        "Network DHCP text address matches binary address",
        text_result == 0 && text.contains(&0) && parsed == address.s_addr && parsed != 0,
    );
    let mut wrapper_address = raw::InAddr::default();
    let mut wrapper_mask = raw::InAddr::default();
    let wrapper = unsafe {
        net::if_configex(
            &mut wrapper_address,
            &mut wrapper_mask,
            core::ptr::null_mut(),
            true,
            1,
        )
    };
    logger.record(
        "Network wrapper DHCP addresses match IOS",
        wrapper == 0
            && wrapper_address.s_addr == address.s_addr
            && wrapper_mask.s_addr == mask.s_addr
            && mask.s_addr != 0,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_readiness(
    logger: &mut ProbeLogger,
    sender: i32,
    receiver: i32,
    destination: &mut raw_network::SockaddrIn,
) {
    use lwip_sys as lwip;
    use raw_network as net;
    let mut enabled = 1u32;
    let set = unsafe { lwip::ioctl(receiver, lwip::FIONBIO, (&mut enabled as *mut u32).cast()) };
    let mut byte = [0xa5u8];
    let empty = if set == 0 {
        unsafe { lwip::recv(receiver, byte.as_mut_ptr().cast(), 1, 0) }
    } else {
        0
    };
    enabled = 0;
    let clear = unsafe { lwip::ioctl(receiver, lwip::FIONBIO, (&mut enabled as *mut u32).cast()) };
    logger.record(
        "lwIP ioctl toggles socket nonblocking mode",
        set == 0 && empty < 0 && byte == [0xa5] && clear == 0,
    );
    enabled = 1;
    let wrapper_set =
        unsafe { net::net_ioctl(receiver, net::FIONBIO, (&mut enabled as *mut u32).cast()) };
    let wrapper_flags = net::net_fcntl(receiver, net::F_GETFL, 0);
    enabled = 0;
    let wrapper_clear =
        unsafe { net::net_ioctl(receiver, net::FIONBIO, (&mut enabled as *mut u32).cast()) };
    let wrapper_restored = net::net_fcntl(receiver, net::F_GETFL, 0);
    logger.note_i32("Raw ioctl set result", wrapper_set);
    logger.note_i32("Raw fcntl flags after ioctl", wrapper_flags);
    logger.note_i32("Raw ioctl clear result", wrapper_clear);
    logger.note_i32("Raw fcntl flags after clear", wrapper_restored);
    logger.record(
        "Raw ioctl and fcntl agree on nonblocking mode",
        wrapper_set == 0
            && wrapper_flags == net::O_NONBLOCK as i32
            && wrapper_clear == 0
            && wrapper_restored == 0,
    );
    let mut pending = 0xa5a5_a5a5u32;
    let rc = unsafe { lwip::ioctl(receiver, lwip::FIONREAD, (&mut pending as *mut u32).cast()) };
    logger.record(
        "lwIP ioctl rejects FIONREAD and preserves output",
        rc == -22 && pending == 0xa5a5_a5a5,
    );

    let mut descriptor = lwip::PollFd {
        fd: receiver,
        events: lwip::POLLIN,
        revents: -1,
    };
    let empty_poll = unsafe { lwip::poll(&mut descriptor, 1, 0) };
    logger.record(
        "lwIP poll clears empty read readiness",
        empty_poll == 0 && descriptor.revents == 0,
    );
    let mut readset = lwip::FdSet::default();
    let fd = receiver as usize;
    if fd >= lwip::FD_SETSIZE {
        logger.record("lwIP select clears empty read readiness", false);
        logger.record("lwIP select reports queued read readiness", false);
        return;
    }
    readset.fd_bits[fd / 8] = 1 << (fd % 8);
    let mut timeout = lwip::TimeVal {
        tv_sec: 0,
        tv_usec: 0,
    };
    let empty_select = unsafe {
        lwip::select(
            receiver + 1,
            &mut readset,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut timeout,
        )
    };
    logger.record(
        "lwIP select clears empty read readiness",
        empty_select == 0 && readset.fd_bits[fd / 8] & (1 << (fd % 8)) == 0,
    );
    let mut raw_set = net::FdSet::default();
    raw_set.fd_bits[fd / 8] = 1 << (fd % 8);
    let mut raw_timeout = net::Timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let raw_empty = unsafe {
        net::net_select(
            receiver + 1,
            &mut raw_set,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut raw_timeout,
        )
    };
    logger.record(
        "Raw select clears empty UDP readiness",
        raw_empty == 0 && raw_set.fd_bits[fd / 8] & (1 << (fd % 8)) == 0,
    );

    let payload = [0x21u8, 0x80, 0, 0xff];
    let sent = unsafe {
        net::net_sendto(
            sender,
            payload.as_ptr().cast(),
            payload.len() as i32,
            0,
            (destination as *mut net::SockaddrIn).cast(),
            16,
        )
    };
    descriptor.revents = 0;
    let ready_poll = if sent == payload.len() as i32 {
        unsafe { lwip::poll(&mut descriptor, 1, 3000) }
    } else {
        -1
    };
    logger.record(
        "lwIP poll reports queued read readiness",
        ready_poll == 1 && descriptor.revents & lwip::POLLIN != 0,
    );
    readset.fd_bits[fd / 8] = 1 << (fd % 8);
    let ready_select = unsafe {
        lwip::select(
            receiver + 1,
            &mut readset,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut timeout,
        )
    };
    logger.record(
        "lwIP select reports queued read readiness",
        ready_select == 1 && readset.fd_bits[fd / 8] & (1 << (fd % 8)) != 0,
    );
    raw_set.fd_bits[fd / 8] = 1 << (fd % 8);
    let raw_ready = unsafe {
        net::net_select(
            receiver + 1,
            &mut raw_set,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut raw_timeout,
        )
    };
    logger.record(
        "Raw select reports queued UDP readiness",
        raw_ready == 1 && raw_set.fd_bits[fd / 8] & (1 << (fd % 8)) != 0,
    );
    let mut bytes = [0xa5u8; 8];
    let got = if ready_poll == 1 {
        unsafe { net::net_recv(receiver, bytes.as_mut_ptr().cast(), bytes.len() as i32, 0) }
    } else {
        -1
    };
    logger.record(
        "lwIP readiness corresponds to queued datagram",
        got == 4 && bytes[..4] == payload && bytes[4..] == [0xa5; 4],
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_udp_addressed(
    logger: &mut ProbeLogger,
    sender: i32,
    receiver: i32,
    source: &raw_network::SockaddrIn,
    destination: &mut raw_network::SockaddrIn,
) {
    use lwip_sys as lwip;
    use raw_network as net;
    let data = [0x61u8, 0, 0xfe, 0x37, 0x80];
    let sent = unsafe {
        lwip::send_to(
            sender,
            data.as_ptr().cast(),
            data.len() as i32,
            0,
            (destination as *mut net::SockaddrIn).cast(),
            16,
        )
    };
    let mut output = [0xa5u8; 24];
    let mut peer = net::SockaddrIn::default();
    let mut peer_len = 16;
    let received = if sent == data.len() as i32 && probe_lwip_udp_readable(receiver) {
        unsafe {
            lwip::receive_from(
                receiver,
                output.as_mut_ptr().cast(),
                output.len() as i32,
                0,
                (&mut peer as *mut net::SockaddrIn).cast(),
                &mut peer_len,
            )
        }
    } else {
        -1
    };
    logger.note_i32("lwIP datagram sent", sent);
    logger.note_i32("lwIP datagram received", received);
    logger.note_u32("lwIP datagram peer length", peer_len);
    logger.note_hex32("lwIP datagram peer address", peer.sin_addr.s_addr);
    logger.note_u32("lwIP datagram peer port", u32::from(peer.sin_port));
    logger.note_u32("lwIP datagram expected port", u32::from(source.sin_port));
    logger.note_hex32(
        "lwIP datagram first bytes",
        u32::from_be_bytes([output[0], output[1], output[2], output[3]]),
    );
    logger.record(
        "lwIP send_to and receive_from preserve datagram and source",
        received == data.len() as i32
            && output[..data.len()] == data
            && output[data.len()..] == [0xa5; 19]
            && peer.sin_addr.s_addr == 0x7f00_0001
            && peer.sin_port == source.sin_port
            && peer_len == 16,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_udp_readable(receiver: i32) -> bool {
    let mut poll = raw_network::PollSd {
        socket: receiver,
        events: 3,
        revents: 0,
    };
    unsafe { raw_network::net_poll(&mut poll, 1, 3000) == 1 && poll.revents & 3 != 0 }
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_udp_connected(logger: &mut ProbeLogger, sender: i32, receiver: i32) {
    use lwip_sys as lwip;
    let mut output = [0xa5u8; 24];
    let data = b"lwip send recv";
    let sent = unsafe { lwip::send(sender, data.as_ptr().cast(), data.len() as i32, 0) };
    output.fill(0xa5);
    let received = if sent == data.len() as i32 && probe_lwip_udp_readable(receiver) {
        unsafe { lwip::recv(receiver, output.as_mut_ptr().cast(), output.len() as i32, 0) }
    } else {
        -1
    };
    logger.record(
        "lwIP send and recv preserve connected datagram",
        received == data.len() as i32
            && output[..data.len()] == *data
            && output[data.len()..].iter().all(|byte| *byte == 0xa5),
    );

    let data = b"lwip write read";
    let sent = unsafe { lwip::write(sender, data.as_ptr().cast(), data.len() as i32) };
    output.fill(0xa5);
    let received = if sent == data.len() as i32 && probe_lwip_udp_readable(receiver) {
        unsafe { lwip::read(receiver, output.as_mut_ptr().cast(), output.len() as i32) }
    } else {
        -1
    };
    logger.record(
        "lwIP write and read preserve connected datagram",
        received == data.len() as i32
            && output[..data.len()] == *data
            && output[data.len()..].iter().all(|byte| *byte == 0xa5),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_raw_tcp(logger: &mut ProbeLogger) {
    use core::mem::size_of;
    use raw_network as net;

    logger.report_step("Raw TCP loopback operations");
    let initialized = net::net_init();
    logger.record("Raw TCP network initializes", initialized >= 0);
    if initialized < 0 {
        return;
    }
    let listener = net::net_socket(net::PF_INET, net::SOCK_STREAM, net::IPPROTO_IP);
    let client = net::net_socket(net::PF_INET, net::SOCK_STREAM, net::IPPROTO_IP);
    logger.note_i32("Raw TCP listener descriptor", listener);
    logger.note_i32("Raw TCP client descriptor", client);
    logger.record("Raw TCP sockets open", listener >= 0 && client >= 0);
    if listener >= 0 && client >= 0 {
        probe_lwip_socket_options(logger, listener);
        let value: u32 = 1;
        let mut actual: u32 = 0;
        let mut length = size_of::<u32>() as u32;
        // SAFETY: The option buffers and address structures are live and sized for each call.
        let options = unsafe {
            net::net_setsockopt(
                listener,
                net::SOL_SOCKET,
                net::SO_REUSEADDR,
                (&value as *const u32).cast(),
                length,
            ) == 0
                && net::net_getsockopt(
                    listener,
                    net::SOL_SOCKET,
                    net::SO_REUSEADDR,
                    (&mut actual as *mut u32).cast(),
                    &mut length,
                ) == 0
        };
        logger.note_u32("Raw TCP reuse option", actual);
        logger.record(
            "Raw TCP reuse option is enabled",
            options && length == 4 && actual != 0,
        );
        let disabled = 0u32;
        actual = u32::MAX;
        length = size_of::<u32>() as u32;
        let disabled_ok = unsafe {
            net::net_setsockopt(
                listener,
                net::SOL_SOCKET,
                net::SO_REUSEADDR,
                (&disabled as *const u32).cast(),
                length,
            ) == 0
                && net::net_getsockopt(
                    listener,
                    net::SOL_SOCKET,
                    net::SO_REUSEADDR,
                    (&mut actual as *mut u32).cast(),
                    &mut length,
                ) == 0
        };
        logger.record(
            "Raw TCP reuse option can be cleared",
            disabled_ok && length == 4 && actual == 0,
        );
        let flags = net::net_fcntl(listener, net::F_GETFL, 0) == 0
            && net::net_fcntl(listener, net::F_SETFL, net::O_NONBLOCK) == 0
            && net::net_fcntl(listener, net::F_GETFL, 0) == net::O_NONBLOCK as i32
            && net::net_fcntl(listener, net::F_SETFL, 0) == 0
            && net::net_fcntl(listener, net::F_GETFL, 0) == 0;
        logger.record("Raw TCP nonblocking flags set and clear", flags);
        let mut address = net::SockaddrIn {
            sin_len: size_of::<net::SockaddrIn>() as u8,
            sin_family: net::AF_INET as u8,
            sin_port: 0,
            sin_addr: net::InAddr {
                s_addr: 0x7f00_0001,
            },
            sin_zero: [0; 8],
        };
        let mut addrlen = size_of::<net::SockaddrIn>() as u32;
        let bound = unsafe {
            net::net_bind(
                listener,
                (&mut address as *mut net::SockaddrIn).cast(),
                addrlen,
            ) == 0
                && net::net_getsockname(
                    listener,
                    (&mut address as *mut net::SockaddrIn).cast(),
                    &mut addrlen,
                ) == 0
        };
        let listening = bound && address.sin_port != 0 && lwip_sys::listen(listener, 1) == 0;
        logger.record("Raw TCP listener binds and listens", listening);
        if listening {
            probe_lwip_unconnected_peer(logger, client);
            let connected = unsafe {
                net::net_connect(
                    client,
                    (&mut address as *mut net::SockaddrIn).cast(),
                    addrlen,
                )
            } == 0;
            logger.record("Raw TCP client connects", connected);
            let mut poll = net::PollSd {
                socket: listener,
                events: 3,
                revents: 0,
            };
            let ready = connected
                && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                && poll.revents & 3 != 0;
            logger.record("Raw TCP listener becomes ready", ready);
            if ready {
                let mut peer = net::SockaddrIn::default();
                let mut peer_len = size_of::<net::SockaddrIn>() as u32;
                let accepted = unsafe {
                    lwip_sys::accept(
                        listener,
                        (&mut peer as *mut net::SockaddrIn).cast(),
                        &mut peer_len,
                    )
                };
                logger.record(
                    "Raw TCP accept reports loopback peer",
                    accepted >= 0 && peer.sin_addr.s_addr == 0x7f00_0001 && peer.sin_port != 0,
                );
                if accepted >= 0 {
                    probe_lwip_socket_names(logger, listener, client, accepted, &address, &peer);
                    let payload = b"tcp probe";
                    let sent = unsafe {
                        net::net_send(client, payload.as_ptr().cast(), payload.len() as i32, 0)
                    };
                    let mut poll = net::PollSd {
                        socket: accepted,
                        events: 3,
                        revents: 0,
                    };
                    let ready = sent == payload.len() as i32
                        && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                        && poll.revents & 3 != 0;
                    let mut bytes = [0xa5; 32];
                    let got = if ready {
                        unsafe {
                            net::net_recv(
                                accepted,
                                bytes.as_mut_ptr().cast(),
                                bytes.len() as i32,
                                0,
                            )
                        }
                    } else {
                        -1
                    };
                    logger.record(
                        "Raw TCP accepted socket receives bytes",
                        got == payload.len() as i32
                            && bytes[..payload.len()] == *payload
                            && bytes[payload.len()..].iter().all(|b| *b == 0xa5),
                    );
                    let shutdown = net::net_shutdown(client, 1) == 0;
                    let mut poll = net::PollSd {
                        socket: accepted,
                        events: 3,
                        revents: 0,
                    };
                    let ready = shutdown
                        && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                        && poll.revents & 3 != 0;
                    let eof = if ready {
                        unsafe {
                            net::net_recv(
                                accepted,
                                bytes.as_mut_ptr().cast(),
                                bytes.len() as i32,
                                0,
                            )
                        }
                    } else {
                        -1
                    };
                    logger.record("Raw TCP write shutdown delivers EOF", shutdown && eof == 0);
                    logger.record(
                        "Raw TCP accepted socket closes",
                        net::net_close(accepted) == 0,
                    );
                }
                let relisten = net::net_listen(listener, 1) == 0;
                let second = net::net_socket(net::PF_INET, net::SOCK_STREAM, net::IPPROTO_IP);
                let connected = second >= 0
                    && relisten
                    && unsafe {
                        net::net_connect(
                            second,
                            (&mut address as *mut net::SockaddrIn).cast(),
                            addrlen,
                        )
                    } == 0;
                let mut poll = net::PollSd {
                    socket: listener,
                    events: 3,
                    revents: 0,
                };
                let ready = connected
                    && unsafe { net::net_poll(&mut poll, 1, 3000) } == 1
                    && poll.revents & 3 != 0;
                let mut second_peer = net::SockaddrIn::default();
                let mut second_len = size_of::<net::SockaddrIn>() as u32;
                let second_accepted = if ready {
                    unsafe {
                        net::net_accept(
                            listener,
                            (&mut second_peer as *mut net::SockaddrIn).cast(),
                            &mut second_len,
                        )
                    }
                } else {
                    -1
                };
                logger.record(
                    "Raw TCP wrapper relistens and accepts",
                    second_accepted >= 0
                        && second_peer.sin_addr.s_addr == 0x7f00_0001
                        && second_peer.sin_port != 0,
                );
                let mut second_local = net::SockaddrIn::default();
                let mut second_local_len = size_of::<net::SockaddrIn>() as u32;
                let second_name = if second_accepted >= 0 {
                    unsafe {
                        net::net_getsockname(
                            second,
                            (&mut second_local as *mut net::SockaddrIn).cast(),
                            &mut second_local_len,
                        )
                    }
                } else {
                    -1
                };
                logger.record(
                    "Raw TCP accepted peer matches second client",
                    second_name == 0
                        && second_local.sin_addr.s_addr == 0x7f00_0001
                        && second_local.sin_port != 0
                        && second_peer.sin_addr == second_local.sin_addr
                        && second_peer.sin_port == second_local.sin_port,
                );
                if second_accepted >= 0 {
                    logger.record(
                        "Raw TCP second accepted socket closes",
                        net::net_close(second_accepted) == 0,
                    );
                }
                if second >= 0 {
                    logger.record("Raw TCP second client closes", net::net_close(second) == 0);
                }
            }
        }
    }
    if listener >= 0 {
        logger.record("Raw TCP listener closes", net::net_close(listener) == 0);
    }
    if client >= 0 {
        logger.record("Raw TCP client closes", net::net_close(client) == 0);
    }
    logger.record("Raw TCP network deinitializes", net::net_deinit() == 0);
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_unconnected_peer(logger: &mut ProbeLogger, socket: i32) {
    use lwip_sys as lwip;
    let mut name = lwip::SockAddrIn {
        sin_len: 0xa5,
        sin_family: 0xa5,
        sin_port: 0xa5a5,
        sin_addr: lwip::InAddr {
            s_addr: 0xa5a5_a5a5,
        },
        sin_zero: [0xa5; 8],
    };
    let mut len = core::mem::size_of::<lwip::SockAddrIn>() as u32;
    let result = unsafe {
        lwip::get_peer_name(
            socket,
            (&mut name as *mut lwip::SockAddrIn).cast(),
            &mut len,
        )
    };
    logger.record(
        "lwIP unconnected peer query preserves output",
        result < 0
            && len == 16
            && name.sin_len == 0xa5
            && name.sin_family == 0xa5
            && name.sin_port == 0xa5a5
            && name.sin_addr.s_addr == 0xa5a5_a5a5
            && name.sin_zero == [0xa5; 8],
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_socket_names(
    logger: &mut ProbeLogger,
    listener: i32,
    client: i32,
    accepted: i32,
    expected_server: &raw_network::SockaddrIn,
    expected_client: &raw_network::SockaddrIn,
) {
    use lwip_sys as lwip;
    let mut local = lwip::SockAddrIn::default();
    let mut len = core::mem::size_of::<lwip::SockAddrIn>() as u32;
    let result = unsafe {
        lwip::get_socket_name(
            listener,
            (&mut local as *mut lwip::SockAddrIn).cast(),
            &mut len,
        )
    };
    logger.record(
        "lwIP listener local address matches bind",
        result == 0
            && len == 8
            && local.sin_family == 2
            && local.sin_addr.s_addr == 0x7f00_0001
            && local.sin_port == expected_server.sin_port
            && local.sin_port != 0,
    );
    let mut remote = lwip::SockAddrIn::default();
    len = core::mem::size_of::<lwip::SockAddrIn>() as u32;
    let result = unsafe {
        lwip::get_peer_name(
            client,
            (&mut remote as *mut lwip::SockAddrIn).cast(),
            &mut len,
        )
    };
    logger.record(
        "lwIP client peer matches listener",
        result == 0
            && len == 8
            && remote.sin_family == 2
            && remote.sin_addr.s_addr == 0x7f00_0001
            && remote.sin_port == expected_server.sin_port,
    );
    len = core::mem::size_of::<lwip::SockAddrIn>() as u32;
    let result = unsafe {
        lwip::get_socket_name(
            client,
            (&mut local as *mut lwip::SockAddrIn).cast(),
            &mut len,
        )
    };
    logger.record(
        "lwIP client local address matches accepted peer",
        result == 0
            && len == 8
            && local.sin_family == 2
            && local.sin_addr.s_addr == 0x7f00_0001
            && local.sin_port != 0
            && local.sin_port == expected_client.sin_port,
    );
    len = core::mem::size_of::<lwip::SockAddrIn>() as u32;
    let result = unsafe {
        lwip::get_peer_name(
            accepted,
            (&mut remote as *mut lwip::SockAddrIn).cast(),
            &mut len,
        )
    };
    logger.record(
        "lwIP accepted peer matches client",
        result == 0
            && len == 8
            && remote.sin_family == 2
            && remote.sin_addr.s_addr == 0x7f00_0001
            && remote.sin_port == expected_client.sin_port
            && remote.sin_port != 0,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_lwip_socket_options(logger: &mut ProbeLogger, socket: i32) {
    use core::mem::size_of;
    use lwip_sys as lwip;

    let mut actual = 0x1234_5678u32;
    let mut length = size_of::<u32>() as u32;
    let enabled = 1u32;
    // SAFETY: All option buffers are live and their lengths match their types.
    let set = unsafe {
        lwip::set_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            (&enabled as *const u32).cast(),
            length,
        )
    };
    let get = unsafe {
        lwip::get_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            (&mut actual as *mut u32).cast(),
            &mut length,
        )
    };
    logger.record(
        "lwIP reuse option set and read",
        set == 0 && get == 0 && length == 4 && actual != 0,
    );

    let disabled = 0u32;
    let set = unsafe {
        lwip::set_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            (&disabled as *const u32).cast(),
            4,
        )
    };
    actual = 0x1234_5678;
    length = 4;
    let get = unsafe {
        lwip::get_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            (&mut actual as *mut u32).cast(),
            &mut length,
        )
    };
    logger.record(
        "lwIP reuse option clears",
        set == 0 && get == 0 && length == 4 && actual == 0,
    );

    actual = 0x1234_5678;
    length = 0;
    let get = unsafe {
        lwip::get_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            (&mut actual as *mut u32).cast(),
            &mut length,
        )
    };
    logger.record(
        "lwIP zero-length option read preserves output",
        get == -22 && actual == 0x1234_5678 && length == 0,
    );
    let oversized = [0u8; 21];
    let set = unsafe {
        lwip::set_socket_option(
            socket,
            lwip::SOL_SOCKET,
            lwip::SO_REUSEADDR,
            oversized.as_ptr().cast(),
            oversized.len() as u32,
        )
    };
    logger.record("lwIP oversized option write rejected", set == -22);
}

#[cfg(feature = "dolphin-automation")]
fn probe_network_udp(logger: &mut ProbeLogger) {
    use ogc_network as net;

    const LOOPBACK: u32 = 0x7f00_0001;
    const ADDR_LEN: u32 = 16;
    const PAYLOAD: &[u8] = &[0x00, 0xff, 0x55, 0x44, 0x50, 0x80];

    logger.report_step("Network UDP wrapper operations");
    let initialized = net::init() >= 0;
    logger.record("UDP wrapper initializes", initialized);
    if !initialized {
        return;
    }
    let sender = net::socket(net::PF_INET, net::SOCK_DGRAM, 0);
    let receiver = net::socket(net::PF_INET, net::SOCK_DGRAM, 0);
    logger.record("UDP wrapper sockets open", sender >= 0 && receiver >= 0);

    let bind = net::SockaddrIn {
        sin_len: ADDR_LEN as u8,
        sin_family: net::AF_INET as u8,
        sin_port: 0,
        sin_addr: net::InAddr { s_addr: LOOPBACK },
        sin_zero: [0; 8],
    };
    let mut socket_name = |fd| {
        let mut name = net::SockaddrIn::default();
        let mut len = ADDR_LEN;
        // SAFETY: The address and length have writable storage for the call.
        let result =
            unsafe { net::getsockname(fd, (&mut name as *mut net::SockaddrIn).cast(), &mut len) };
        logger.note_i32("UDP getsockname result", result);
        logger.note_u32("UDP getsockname address length", len);
        logger.note_u32("UDP getsockname address family", u32::from(name.sin_family));
        logger.note_hex32("UDP getsockname address", name.sin_addr.s_addr);
        logger.note_u32("UDP getsockname port", u32::from(name.sin_port));
        (result == 0
            // getsockname reports the compact IOS IPv4 address length.
            && len == 8
            && name.sin_family == bind.sin_family
            && name.sin_addr.s_addr == LOOPBACK
            && name.sin_port != 0)
            .then_some(name)
    };
    let bound = sender >= 0
        && receiver >= 0
        && net::bind_addr(sender, &bind) == 0
        && net::bind_addr(receiver, &bind) == 0;
    let names = if bound {
        socket_name(sender).zip(socket_name(receiver))
    } else {
        None
    };
    logger.record("UDP wrapper binds loopback sockets", bound);
    let mut bytes = [0xa5; 16];
    let mut peer = net::SockaddrIn::default();
    let mut peer_len = ADDR_LEN;
    let send = |fd, to: &mut net::SockaddrIn| {
        // SAFETY: The payload and address are readable for the supplied lengths.
        unsafe {
            net::sendto(
                fd,
                PAYLOAD.as_ptr().cast(),
                PAYLOAD.len() as i32,
                0,
                (to as *mut net::SockaddrIn).cast(),
                ADDR_LEN,
            )
        }
    };
    let recv = |fd, bytes: &mut [u8], peer: &mut net::SockaddrIn, len: &mut u32| {
        // SAFETY: The caller supplies writable data and address storage and its length.
        unsafe {
            net::recvfrom(
                fd,
                bytes.as_mut_ptr().cast(),
                bytes.len() as i32,
                0,
                (peer as *mut net::SockaddrIn).cast(),
                len,
            )
        }
    };
    let rejected = recv(-1, &mut bytes, &mut peer, &mut peer_len);
    logger.record(
        "UDP recvfrom rejects an invalid handle and preserves output",
        rejected == net::SOCKET_ERROR
            && bytes == [0xa5; 16]
            && peer == net::SockaddrIn::default()
            && peer_len == ADDR_LEN,
    );

    let distinct_names = names.is_some_and(|(from, to)| from.sin_port != to.sin_port);
    logger.record("UDP wrapper binds distinct loopback ports", distinct_names);
    if let Some((from, mut to)) = names.filter(|_| distinct_names) {
        logger.record(
            "UDP sendto rejects an invalid handle",
            send(-1, &mut to) == net::SOCKET_ERROR,
        );
        let sent = send(sender, &mut to);
        let ready = sent == PAYLOAD.len() as i32 && wrapper_readable(receiver);
        logger.record("UDP sendto reports byte count and queues datagram", ready);
        if ready {
            let received = recv(receiver, &mut bytes, &mut peer, &mut peer_len);
            logger.note_i32("UDP wrapper received byte count", received);
            logger.note_u32("UDP wrapper received address length", peer_len);
            logger.note_u32("UDP wrapper expected source port", u32::from(from.sin_port));
            logger.note_u32("UDP wrapper received source port", u32::from(peer.sin_port));
            logger.record(
                "UDP recvfrom returns the payload and source address",
                received == PAYLOAD.len() as i32
                    && bytes[..PAYLOAD.len()] == *PAYLOAD
                    && bytes[PAYLOAD.len()..] == [0xa5; 16 - PAYLOAD.len()]
                    && peer_len == ADDR_LEN
                    && peer.sin_family == bind.sin_family
                    && peer.sin_addr.s_addr == LOOPBACK
                    && peer.sin_port == from.sin_port,
            );
        }
    }
    if sender >= 0 {
        logger.record("UDP wrapper sender closes", net::close(sender) == 0);
    }
    if receiver >= 0 {
        logger.record("UDP wrapper receiver closes", net::close(receiver) == 0);
    }
    logger.record("UDP wrapper deinitializes", net::deinit() == 0);
}

#[cfg(feature = "dolphin-automation")]
fn probe_network_tcp(logger: &mut ProbeLogger) {
    use core::mem::size_of;
    use ogc_network as net;

    logger.report_step("Network TCP wrapper operations");
    let initialized = net::init();
    logger.record("TCP wrapper initializes", initialized >= 0);
    if initialized < 0 {
        return;
    }
    let listener = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
    let client = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
    logger.record("TCP wrapper sockets open", listener >= 0 && client >= 0);
    if listener >= 0 && client >= 0 {
        let raw = net::socket_raw_fd(listener);
        let flags = net::fcntl(listener, raw_network::F_GETFL, 0) == 0
            && net::fcntl(listener, raw_network::F_SETFL, net::O_NONBLOCK) == 0
            && net::fcntl(listener, raw_network::F_GETFL, 0) == net::O_NONBLOCK as i32
            && net::fcntl(listener, raw_network::F_SETFL, 0) == 0;
        logger.record("TCP wrapper changes socket flags", flags);
        let mut address = net::SockaddrIn {
            sin_len: size_of::<net::SockaddrIn>() as u8,
            sin_family: net::AF_INET as u8,
            sin_port: 0,
            sin_addr: net::InAddr {
                s_addr: 0x7f00_0001,
            },
            sin_zero: [0; 8],
        };
        let mut len = size_of::<net::SockaddrIn>() as u32;
        // SAFETY: The address buffers and lengths are valid for these calls.
        let bound = net::bind_addr(listener, &address) == 0
            && unsafe {
                net::getsockname(
                    listener,
                    (&mut address as *mut net::SockaddrIn).cast(),
                    &mut len,
                )
            } == 0;
        let mut raw_address = raw_network::SockaddrIn::default();
        let mut raw_len = size_of::<raw_network::SockaddrIn>() as u32;
        let raw_matches = raw.is_some_and(|fd| {
            // SAFETY: The address and length are writable for this call.
            unsafe {
                raw_network::net_getsockname(
                    fd,
                    (&mut raw_address as *mut raw_network::SockaddrIn).cast(),
                    &mut raw_len,
                ) == 0
            }
        });
        logger.record(
            "TCP wrapper resolves raw socket",
            bound
                && raw_matches
                && net::soc_get_fd(listener) == raw.unwrap_or(-1)
                && raw_address.sin_port == address.sin_port
                && raw_address.sin_addr.s_addr == 0x7f00_0001,
        );
        let listening = bound && address.sin_port != 0 && net::listen(listener, 2) == 0;
        logger.record("TCP wrapper binds and listens", listening);
        if listening {
            let connected = net::connect_addr(client, &address) == 0;
            logger.record("TCP wrapper typed connect", connected);
            if connected && wrapper_readable(listener) {
                let mut peer = net::SockaddrIn::default();
                let accepted = net::accept_addr(listener, &mut peer);
                logger.record(
                    "TCP wrapper typed accept",
                    accepted >= 0 && peer.sin_addr.s_addr == 0x7f00_0001 && peer.sin_port != 0,
                );
                if accepted >= 0 {
                    let a = b"send slice";
                    let sent = net::send_slice(client, a, 0);
                    probe_socket_readiness(logger, accepted);
                    let mut bytes = [0xa5; 32];
                    let got = if sent == a.len() as i32 && wrapper_readable(accepted) {
                        net::recv_slice(accepted, &mut bytes, 0)
                    } else {
                        -1
                    };
                    logger.record(
                        "TCP wrapper slice send and receive",
                        got == a.len() as i32
                            && bytes[..a.len()] == *a
                            && bytes[a.len()..].iter().all(|b| *b == 0xa5),
                    );
                    probe_socket_select_empty(logger, accepted);
                    probe_socket_ioctl(logger, accepted);
                    let b = b"write slice";
                    let sent = net::write_slice(accepted, b);
                    let mut bytes = [0xa5; 32];
                    let got = if sent == b.len() as i32 && wrapper_readable(client) {
                        net::read_slice(client, &mut bytes)
                    } else {
                        -1
                    };
                    logger.record(
                        "TCP wrapper slice write and read",
                        got == b.len() as i32
                            && bytes[..b.len()] == *b
                            && bytes[b.len()..].iter().all(|b| *b == 0xa5),
                    );
                    let c = b"raw send";
                    let sent = unsafe { net::send(client, c.as_ptr().cast(), c.len() as i32, 0) };
                    let mut bytes = [0xa5; 32];
                    let got = if sent == c.len() as i32 && wrapper_readable(accepted) {
                        unsafe {
                            net::recv(accepted, bytes.as_mut_ptr().cast(), bytes.len() as i32, 0)
                        }
                    } else {
                        -1
                    };
                    logger.record(
                        "TCP wrapper raw send and receive",
                        got == c.len() as i32
                            && bytes[..c.len()] == *c
                            && bytes[c.len()..].iter().all(|b| *b == 0xa5),
                    );
                    let d = b"raw write";
                    let sent = unsafe { net::write(accepted, d.as_ptr().cast(), d.len() as i32) };
                    let mut bytes = [0xa5; 32];
                    let got = if sent == d.len() as i32 && wrapper_readable(client) {
                        unsafe { net::read(client, bytes.as_mut_ptr().cast(), bytes.len() as i32) }
                    } else {
                        -1
                    };
                    logger.record(
                        "TCP wrapper raw write and read",
                        got == d.len() as i32
                            && bytes[..d.len()] == *d
                            && bytes[d.len()..].iter().all(|b| *b == 0xa5),
                    );
                    logger.record(
                        "TCP wrapper releases accepted socket",
                        net::socket_release_fd(accepted) == 0
                            && net::socket_raw_fd(accepted).is_none(),
                    );
                }
            } else {
                logger.record("TCP wrapper listener ready", false);
            }
            let client2 = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
            let connected2 = client2 >= 0 && net::connect_addr(client2, &address) == 0;
            let accepted2 = if connected2 && wrapper_readable(listener) {
                net::accept_socket(listener)
            } else {
                -1
            };
            logger.record("TCP wrapper accepts without peer output", accepted2 >= 0);
            if accepted2 >= 0 {
                logger.record(
                    "TCP wrapper second accepted socket closes",
                    net::close(accepted2) == 0,
                );
            }
            if client2 >= 0 {
                logger.record("TCP wrapper second client closes", net::close(client2) == 0);
            }

            let client3 = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
            let mut peer = net::SockaddrIn::default();
            let mut peer_len = size_of::<net::SockaddrIn>() as u32;
            let accepted3 = if client3 >= 0
                && net::connect_addr(client3, &address) == 0
                && wrapper_readable(listener)
            {
                // SAFETY: The peer storage and length are valid writable outputs.
                unsafe {
                    net::accept(
                        listener,
                        (&mut peer as *mut net::SockaddrIn).cast(),
                        &mut peer_len,
                    )
                }
            } else {
                -1
            };
            logger.record(
                "TCP pointer accept returns loopback peer",
                accepted3 >= 0 && peer.sin_addr.s_addr == 0x7f00_0001 && peer.sin_port != 0,
            );
            if accepted3 >= 0 {
                logger.record(
                    "TCP pointer accepted socket closes",
                    net::close(accepted3) == 0,
                );
            }
            if client3 >= 0 {
                logger.record("TCP pointer accept client closes", net::close(client3) == 0);
            }
        }
    }
    if listener >= 0 {
        logger.record("TCP wrapper listener closes", net::close(listener) == 0);
    }
    let client_was_open = client >= 0 && net::socket_raw_fd(client).is_some();
    let exited = net::soc_exit() == 0;
    logger.record(
        "Socket table exit closes a live handle",
        client_was_open && exited && net::socket_raw_fd(client).is_none() && net::close(client) < 0,
    );
    let reinitialized = net::soc_init() == 0;
    let fresh = net::socket(net::PF_INET, net::SOCK_STREAM, 0);
    logger.record(
        "Socket table init permits fresh sockets",
        reinitialized && fresh >= 0,
    );
    if fresh >= 0 {
        logger.record("Fresh socket closes", net::close(fresh) == 0);
    }
    logger.record("TCP wrapper deinitializes", net::deinit() == 0);
}

#[cfg(feature = "dolphin-automation")]
fn probe_socket_readiness(logger: &mut ProbeLogger, fd: i32) {
    use ogc_network as net;

    let Some(raw) = net::socket_raw_fd(fd) else {
        logger.record("Readiness probe maps a live socket", false);
        return;
    };
    let events = net::POLLIN as u32;
    let mut readset = net::FdSet::default();
    net::fd_set(&mut readset, fd as usize);
    let mut timeout = net::Timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    // SAFETY: The descriptor set and timeout are writable for the call.
    let selected = unsafe {
        net::select(
            fd + 1,
            &mut readset,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut timeout,
        )
    };
    logger.record(
        "Select reports queued stream data",
        selected == 1 && net::fd_is_set(&readset, fd as usize),
    );
    let mut descriptor = net::PollSd {
        socket: raw,
        events,
        revents: 0,
    };
    // SAFETY: The single descriptor remains live and writable.
    let count = unsafe { net::poll_raw(&mut descriptor, 1, 3000) };
    logger.record(
        "Pointer poll reports queued stream data",
        count == 1 && descriptor.revents & events != 0,
    );
    descriptor.revents = 0;
    let count = net::poll_raw_slice(core::slice::from_mut(&mut descriptor), 0);
    logger.record(
        "Slice poll reports queued stream data",
        count == 1 && descriptor.revents & events != 0,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_socket_select_empty(logger: &mut ProbeLogger, fd: i32) {
    use ogc_network as net;
    let mut readset = net::FdSet::default();
    net::fd_set(&mut readset, fd as usize);
    let mut timeout = net::Timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    // SAFETY: The descriptor set and timeout are writable for the call.
    let selected = unsafe {
        net::select(
            fd + 1,
            &mut readset,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut timeout,
        )
    };
    logger.record(
        "Select clears an empty stream",
        selected == 0 && !net::fd_is_set(&readset, fd as usize),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_socket_ioctl(logger: &mut ProbeLogger, fd: i32) {
    use ogc_network as net;

    let mut pending = u32::MAX;
    // SAFETY: This ioctl accepts a pointer to one writable u32.
    let rc = unsafe { net::ioctl(fd, net::FIONREAD, (&mut pending as *mut u32).cast()) };
    logger.record(
        "TCP FIONREAD is rejected without writing output",
        rc == -1 && pending == u32::MAX,
    );

    let mut enabled = 1u32;
    // SAFETY: FIONBIO reads a u32 from live storage.
    let rc = unsafe { net::ioctl(fd, net::FIONBIO, (&mut enabled as *mut u32).cast()) };
    logger.record("TCP FIONBIO enables nonblocking mode", rc == 0);
    if rc == 0 {
        let mut byte = [0xa5];
        let empty = net::recv_slice(fd, &mut byte, 0);
        logger.note_i32("TCP empty nonblocking receive result", empty);
        logger.record(
            "TCP nonblocking receive leaves empty buffer unchanged",
            empty < 0 && byte == [0xa5],
        );
        enabled = 0;
        // SAFETY: FIONBIO reads a u32 from live storage.
        let restored = unsafe { net::ioctl(fd, net::FIONBIO, (&mut enabled as *mut u32).cast()) };
        logger.record("TCP FIONBIO restores blocking mode", restored == 0);
    }
}

#[cfg(feature = "dolphin-automation")]
fn wrapper_readable(socket: i32) -> bool {
    use ogc_network as net;
    let mut fd = net::PollFd {
        fd: socket,
        events: net::POLLIN,
        revents: 0,
    };
    // SAFETY: The single descriptor remains writable for the duration of the call.
    (unsafe { net::poll(&mut fd, 1, 3000) == 1 }) && fd.revents & net::POLLIN != 0
}

fn probe_hci_events(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;
    let complete = [0x0e, 6, 1, 3, 0x0c, 0, 0x12, 0x34, 0xee];
    let status = [0x0f, 4, 0x0c, 1, 3, 0x0c];
    let command_ok = matches!(bt::parse_event(&complete), Ok(bt::HciEvent::CommandComplete {
        opcode, status: 0, params: [0x12, 0x34] }) if opcode.raw() == 0x0c03)
        && matches!(bt::parse_event(&status), Ok(bt::HciEvent::CommandStatus {
            opcode, status: 0x0c }) if opcode.raw() == 0x0c03);
    let short_complete =
        (2..8).all(|n| bt::parse_event(&complete[..n]) == Err(bt::Error::MalformedEvent));
    let short_status =
        (2..6).all(|n| bt::parse_event(&status[..n]) == Err(bt::Error::MalformedEvent));
    logger.record(
        "HCI command events and truncated packets",
        command_ok && short_complete && short_status,
    );

    let connection = [3, 11, 0, 0x34, 0x02, 1, 2, 3, 4, 5, 6, 1, 0];
    let request = [4, 10, 1, 2, 3, 4, 5, 6, 0x11, 0x22, 0x33, 1];
    let disconnect = [5, 4, 0, 0x34, 0x02, 0x13];
    let connection_ok = matches!(bt::parse_event(&connection), Ok(bt::HciEvent::ConnectionComplete {
        status: 0, address, handle: 0x234, link_type: 1, encryption: 0 })
        if address.bytes == [1, 2, 3, 4, 5, 6]);
    let request_ok = matches!(bt::parse_event(&request), Ok(bt::HciEvent::ConnectionRequest {
        address, class_of_device: [0x11, 0x22, 0x33], link_type: 1 })
        if address.bytes == [1, 2, 3, 4, 5, 6]);
    let disconnect_ok = matches!(
        bt::parse_event(&disconnect),
        Ok(bt::HciEvent::DisconnectionComplete {
            status: 0,
            handle: 0x234,
            reason: 0x13
        })
    );
    logger.record(
        "HCI connection and disconnection events",
        connection_ok && request_ok && disconnect_ok,
    );

    let unknown = [0xfe, 2, 0xaa, 0xbb, 0xcc];
    let malformed = [3, 1, 0];
    logger.record(
        "HCI unknown and malformed events",
        matches!(
            bt::parse_event(&unknown),
            Ok(bt::HciEvent::Unknown {
                event_code: 0xfe,
                payload: [0xaa, 0xbb]
            })
        ) && bt::parse_event(&[]) == Err(bt::Error::EventBufferTooSmall)
            && bt::parse_event(&[3]) == Err(bt::Error::EventBufferTooSmall)
            && bt::parse_event(&malformed) == Err(bt::Error::MalformedEvent),
    );
}

fn probe_l2cap_parsers(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;

    // These bytes are fixed independently of the packet builders.
    let acl = [
        0x23, 0xa1, 12, 0, 8, 0, 1, 0, 2, 7, 4, 0, 0x11, 0, 0x40, 0, 0xee,
    ];
    let parsed_acl = bt::parse_acl_packet(&acl);
    let acl_ok = matches!(parsed_acl, Ok(p) if p.handle == 0x123
        && p.packet_boundary == 2 && p.broadcast == 2 && p.payload == &acl[4..16]);
    let acl_truncated =
        (0..16).all(|n| bt::parse_acl_packet(&acl[..n]) == Err(bt::Error::MalformedAcl));
    logger.record("ACL parsing fields and truncation", acl_ok && acl_truncated);

    let l2cap = &acl[4..16];
    let l2cap_ok = matches!(bt::parse_l2cap_packet(l2cap), Ok(p)
        if p.length == 8 && p.channel_id == 1 && p.payload == &acl[8..16]);
    let l2cap_truncated =
        (0..12).all(|n| bt::parse_l2cap_packet(&l2cap[..n]) == Err(bt::Error::MalformedL2cap));
    logger.record(
        "L2CAP parsing fields and truncation",
        l2cap_ok && l2cap_truncated,
    );

    let signal_bytes = &acl[8..16];
    let signal_ok = matches!(bt::parse_l2cap_signal(signal_bytes), Ok(s)
        if s.code == 2 && s.identifier == 7 && s.payload == [0x11, 0, 0x40, 0]);
    let signal_truncated = (0..8)
        .all(|n| bt::parse_l2cap_signal(&signal_bytes[..n]) == Err(bt::Error::MalformedL2cap));
    let request = bt::L2capSignal {
        code: 2,
        identifier: 7,
        payload: &[0x11, 0, 0x40, 0],
    };
    let request_ok = bt::parse_l2cap_signal_payload(request)
        == Ok(bt::L2capSignalPayload::ConnectionRequest {
            psm: 0x11,
            source_cid: 0x40,
        });
    let short = bt::L2capSignal {
        payload: &[0x11, 0, 0x40],
        ..request
    };
    let short_rejected = bt::parse_l2cap_signal_payload(short) == Err(bt::Error::MalformedL2cap);
    logger.record(
        "L2CAP signal parsing and payload",
        signal_ok && signal_truncated && request_ok && short_rejected,
    );

    let commands = [2, 7, 4, 0, 0x11, 0, 0x40, 0, 6, 9, 0, 0];
    let mut remaining = &commands[..];
    let first = bt::parse_next_l2cap_signal(&mut remaining);
    let second = bt::parse_next_l2cap_signal(&mut remaining);
    let end = bt::parse_next_l2cap_signal(&mut remaining);
    let mut incomplete = &commands[..7];
    let rejected = bt::parse_next_l2cap_signal(&mut incomplete);
    logger.record("L2CAP signal iteration and error cursor",
        matches!(first, Ok(Some(s)) if s == request)
        && matches!(second, Ok(Some(s)) if s.code == 6 && s.identifier == 9 && s.payload.is_empty())
        && end == Ok(None) && rejected == Err(bt::Error::MalformedL2cap)
        && incomplete == &commands[..7]);

    let hidp = [0xa1, 0x12, 0x34];
    logger.record(
        "HIDP parsing fields and empty input",
        matches!(bt::parse_hidp_packet(&hidp), Ok(p)
            if p.transaction == 0xa0 && p.parameter == 1 && p.payload == [0x12, 0x34])
            && bt::parse_hidp_packet(&[]) == Err(bt::Error::MalformedL2cap),
    );
}

fn probe_owned_audio_buffer(logger: &mut ProbeLogger) {
    logger.section("OWNED AUDIO BUFFER");
    let mut source = [0u8; 65];
    for (index, byte) in source.iter_mut().enumerate() {
        *byte = index as u8 ^ 0x5a;
    }
    let mut valid = true;
    for len in [0, 1, 31, 32, 33, 64, 65] {
        let expected = source;
        let buffer = match aesnd::OwnedAudioBuffer::copy_from_slice(&source[..len]) {
            Ok(buffer) => buffer,
            Err(_) => {
                valid = false;
                continue;
            }
        };
        source.fill(0);
        valid &= buffer.len() == len as u32
            && buffer.is_empty() == (len == 0)
            && buffer.as_bytes() == &expected[..len]
            && (buffer.as_bytes().as_ptr() as usize & 31) == 0;
        source = expected;
    }
    logger.record("owned audio buffers preserve aligned copied bytes", valid);
    let handle = aesnd::VoiceHandle::new(core::ptr::null_mut());
    let null = handle.is_null() && handle.as_ptr().is_null() && handle.into_raw().is_null();
    let mut storage = 0u32;
    let pointer = (&mut storage as *mut u32).cast::<aesnd::sys::Aesndpb>();
    let handle = aesnd::VoiceHandle::new(pointer);
    let nonnull = !handle.is_null() && handle.as_ptr() == pointer && handle.into_raw() == pointer;
    logger.record("audio voice handle pointer roundtrip", null && nonnull);
}

fn probe_l2cap_packets(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;
    let mut bytes = [0xa5; 32];
    let n = bt::build_l2cap_connection_request(&mut bytes, 0x123, 7, 0x11, 0x40);
    logger.record(
        "L2CAP connection request bytes",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 2, 7, 4, 0, 0x11, 0, 0x40, 0]
            && bytes[16] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_request(&mut bytes, 0x123, 8, 0x41);
    logger.record(
        "L2CAP configuration request bytes",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 4, 8, 4, 0, 0x41, 0, 0, 0]
            && bytes[16] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_request_with_mtu(&mut bytes, 0x123, 9, 0x41, 0x2a0);
    logger.record(
        "L2CAP configuration MTU bytes",
        n == Ok(20)
            && bytes[..20]
                == [
                    0x23, 0x21, 16, 0, 12, 0, 1, 0, 4, 9, 8, 0, 0x41, 0, 0, 0, 1, 2, 0xa0, 2,
                ]
            && bytes[20] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_connection_response(&mut bytes, 0x123, 10, 0x40, 0x50, 0);
    logger.record(
        "L2CAP connection response bytes and errors",
        n == Ok(20)
            && bytes[..20]
                == [
                    0x23, 0x21, 16, 0, 12, 0, 1, 0, 3, 10, 8, 0, 0x40, 0, 0x50, 0, 0, 0, 0, 0,
                ]
            && bytes[20] == 0xa5
            && bt::build_l2cap_connection_response(&mut bytes[..19], 0x123, 10, 0x40, 0x50, 0)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_connection_response(&mut bytes, 0x1000, 10, 0x40, 0x50, 0)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_response(&mut bytes, 0x123, 11, 0x40, 0);
    logger.record(
        "L2CAP configuration response bytes and errors",
        n == Ok(18)
            && bytes[..18]
                == [
                    0x23, 0x21, 14, 0, 10, 0, 1, 0, 5, 11, 6, 0, 0x40, 0, 0, 0, 0, 0,
                ]
            && bytes[18] == 0xa5
            && bt::build_l2cap_configuration_response(&mut bytes[..17], 0x123, 11, 0x40, 0)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_configuration_response(&mut bytes, 0x1000, 11, 0x40, 0)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_disconnection_response(&mut bytes, 0x123, 12, 0x40, 0x50);
    logger.record(
        "L2CAP disconnection response bytes and errors",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 7, 12, 4, 0, 0x40, 0, 0x50, 0]
            && bytes[16] == 0xa5
            && bt::build_l2cap_disconnection_response(&mut bytes[..15], 0x123, 12, 0x40, 0x50)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_disconnection_response(&mut bytes, 0x1000, 12, 0x40, 0x50)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_command_reject(&mut bytes, 0x123, 13, 2);
    logger.record(
        "L2CAP command reject bytes and errors",
        n == Ok(14)
            && bytes[..14] == [0x23, 0x21, 10, 0, 6, 0, 1, 0, 1, 13, 2, 0, 2, 0]
            && bytes[14] == 0xa5
            && bt::build_l2cap_command_reject(&mut bytes[..13], 0x123, 13, 2)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_command_reject(&mut bytes, 0x1000, 13, 2)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_invalid_cid_reject(&mut bytes, 0x123, 14, 0x40);
    logger.record(
        "L2CAP invalid CID reject bytes and errors",
        n == Ok(18)
            && bytes[..18]
                == [
                    0x23, 0x21, 14, 0, 10, 0, 1, 0, 1, 14, 6, 0, 2, 0, 0x40, 0, 0, 0,
                ]
            && bytes[18] == 0xa5
            && bt::build_l2cap_invalid_cid_reject(&mut bytes[..17], 0x123, 14, 0x40)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_invalid_cid_reject(&mut bytes, 0x1000, 14, 0x40)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_data(&mut bytes, 0x123, 0x42, &[0xaa, 0xbb]);
    logger.record(
        "L2CAP data bytes and errors",
        n == Ok(10)
            && bytes[..10] == [0x23, 0x21, 6, 0, 2, 0, 0x42, 0, 0xaa, 0xbb]
            && bytes[10] == 0xa5
            && bt::build_l2cap_data(&mut bytes[..9], 0x123, 0x42, &[0xaa, 0xbb])
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_data(&mut bytes, 0x1000, 0x42, &[]) == Err(bt::Error::MalformedAcl),
    );
}

fn probe_ui_adapters(logger: &mut ProbeLogger) {
    use ogc_ui::{Direction, PadInputAdapter, UiInput, WpadInputAdapter};
    logger.section("UI INPUT ADAPTERS");
    let mut adapter = PadInputAdapter::default();
    let mut pad = ogc_input::PadStatus {
        button: ogc_input::PAD_BUTTON_A | ogc_input::PAD_BUTTON_UP,
        ..Default::default()
    };
    let first = adapter.update(&pad);
    let held = adapter.update(&pad);
    adapter.reset();
    let reset = adapter.update(&pad);
    pad.err = -1;
    let disconnected = adapter.update(&pad);
    pad.err = ogc_input::PAD_ERR_NONE as i8;
    let reconnected = adapter.update(&pad);
    logger.record(
        "UI PAD edges reset and reconnect",
        first.activate
            && first.direction == Some(Direction::Up)
            && held == UiInput::default()
            && reset == first
            && disconnected == UiInput::default()
            && reconnected == first,
    );

    let invalid = WpadInputAdapter::new(0, 480).is_err() && WpadInputAdapter::new(640, 0).is_err();
    let adapter = WpadInputAdapter::new(640, 480).expect("valid UI viewport");
    let mut sample = wpad_sys::WpadData {
        btns_d: wpad_sys::WPAD_BUTTON_A | wpad_sys::WPAD_BUTTON_LEFT,
        data_present: wpad_sys::WPAD_DATA_IR,
        ..Default::default()
    };
    sample.ir.valid = 1;
    sample.ir.x = 320.0;
    sample.ir.y = 240.0;
    sample.ir.vres = [640, 480];
    let mapped = adapter.update(&sample);
    sample.err = -1;
    let disconnected = adapter.update(&sample);
    logger.record(
        "UI WPAD edges pointer and errors",
        invalid
            && mapped.activate
            && mapped.direction == Some(Direction::Left)
            && mapped.pointer == Some((320, 240))
            && mapped.pointer_activate
            && disconnected == UiInput::default(),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_system_sram_getters(logger: &mut ProbeLogger) {
    logger.report_step("System SRAM getter values");
    // SAFETY: The probe holds the SRAM lock while copying and changing the
    // shared buffer. Unlock without writeback before calling the getters.
    let sram = unsafe { system::lock_sram() };
    if sram.is_null() {
        logger.record("System SRAM lock acquired", false);
        return;
    }
    let original = unsafe { *sram };
    unsafe {
        (*sram).counter_bias = 0x1234_5678;
        (*sram).display_offset_h = -7;
        (*sram).ntd = 0x40;
        (*sram).lang = 4;
        (*sram).flags = 0x86;
        system::unlock_sram(false);
    }
    let fixture = system::SysSram {
        counter_bias: 0x89ab_cdef,
        display_offset_h: -120,
        lang: 0xfe,
        ..system::SysSram::default()
    };
    let other = system::SysSram {
        counter_bias: 0x1020_3040,
        display_offset_h: 75,
        lang: 3,
        ..system::SysSram::default()
    };
    logger.record(
        "SRAM field getters read supplied signed and unsigned values",
        system::sram_counter_bias(&fixture) == 0x89ab_cdef
            && system::sram_display_offset_h(&fixture) == -120
            && system::sram_language(&fixture) == 0xfe
            && system::sram_counter_bias(&other) == 0x1020_3040
            && system::sram_display_offset_h(&other) == 75
            && system::sram_language(&other) == 3,
    );
    logger.record(
        "System counter bias reads SRAM",
        system::get_counter_bias() == 0x1234_5678,
    );
    logger.record(
        "System display offset reads signed SRAM",
        system::get_display_offset_h() == -7,
    );
    logger.record("System RGB60 reads SRAM bit", system::get_eu_rgb60() == 1);
    logger.record("System language reads SRAM", system::get_language() == 4);
    logger.record(
        "System progressive scan reads SRAM bit",
        system::get_progressive_scan() == 1,
    );
    logger.record(
        "System sound mode reads SRAM bit",
        system::get_sound_mode() == 1,
    );
    logger.record(
        "System video mode reads SRAM bits",
        system::get_video_mode() == 2,
    );
    let config = system::snapshot_config();
    logger.record(
        "System configuration snapshot reflects SRAM fixture",
        config.counter_bias == 0x1234_5678
            && config.display_offset_h == -7
            && config.eu_rgb60 == 1
            && config.language == 4
            && config.progressive_scan == 1
            && config.sound_mode == 1
            && config.video_mode == 2
            && config.counter_bias_get_called
            && config.display_offset_get_called
            && config.eu_rgb60_get_called
            && config.language_get_called
            && config.progressive_scan_get_called,
    );

    // SAFETY: Reacquire before restoring all original fields. No writeback is
    // requested; a failed lock is recorded and prevents dereferencing null.
    let sram = unsafe { system::lock_sram() };
    let restored = if sram.is_null() {
        false
    } else {
        unsafe {
            *sram = original;
            system::unlock_sram(false);
        }
        let check = unsafe { system::lock_sram() };
        if check.is_null() {
            false
        } else {
            let matches = unsafe { *check == original };
            unsafe {
                system::unlock_sram(false);
            }
            matches
        }
    };
    logger.record("System SRAM buffer is restored", restored);
}

fn probe_ipv4_values(logger: &mut ProbeLogger) {
    use ogc_network as net;

    logger.report_step("Raw IPv4 address and type-of-service values");
    for (bytes, word) in [
        ([1, 2, 3, 4], 0x0102_0304),
        ([255, 0, 128, 254], 0xff00_80fe),
        ([0, 0, 0, 0], 0),
    ] {
        logger.record(
            "IPv4 wrapper packs and reads fixed network-order values",
            net::ip4_addr(bytes[0], bytes[1], bytes[2], bytes[3]) == word
                && net::ip4_addr1(word) == u32::from(bytes[0])
                && net::ip4_addr2(word) == u32::from(bytes[1])
                && net::ip4_addr3(word) == u32::from(bytes[2])
                && net::ip4_addr4(word) == u32::from(bytes[3]),
        );
        let packed = raw_network::ip4_addr(bytes[0], bytes[1], bytes[2], bytes[3]);
        logger.record(
            "Raw IPv4 packing matches fixed network-order word",
            packed == word,
        );
        logger.record(
            "Raw IPv4 octet getters match fixed bytes",
            raw_network::ip4_addr1(word) == u32::from(bytes[0])
                && raw_network::ip4_addr2(word) == u32::from(bytes[1])
                && raw_network::ip4_addr3(word) == u32::from(bytes[2])
                && raw_network::ip4_addr4(word) == u32::from(bytes[3]),
        );
    }
    for (input, tos, precedence) in [(0xff, 0x1e, 0xe0), (0x95, 0x14, 0x80), (0, 0, 0)] {
        logger.record(
            "IPv4 wrapper masks type of service and precedence",
            net::iptost(input) == tos && net::iptos_prec(input) == precedence,
        );
        logger.record(
            "Raw IPv4 TOS and precedence masks",
            raw_network::iptost(input) == tos && raw_network::iptos_prec(input) == precedence,
        );
    }
    // SAFETY: Each byte string has a terminating NUL and remains live.
    let parsed = unsafe {
        raw_network::inet_addr(c"192.0.2.1".as_ptr()) == 0xc000_0201
            && raw_network::inet_addr(c"127.0.0.1".as_ptr()) == 0x7f00_0001
            && raw_network::inet_addr(c"256.0.0.1".as_ptr()) == u32::MAX
            && raw_network::inet_addr(core::ptr::null()) == u32::MAX
    };
    logger.record(
        "Raw IPv4 parser accepts valid and rejects invalid input",
        parsed,
    );
    // SAFETY: These C strings remain live and are NUL-terminated.
    let parsed = unsafe {
        net::inet_addr(c"192.0.2.1".as_ptr()) == 0xc000_0201
            && net::inet_addr(c"127.0.0.1".as_ptr()) == 0x7f00_0001
            && net::inet_addr(c"256.0.0.1".as_ptr()) == u32::MAX
    };
    logger.record("IPv4 wrapper parses valid and invalid input", parsed);

    logger.record(
        "Network resolver error text matches known and unknown codes",
        [
            (-3, c"temporary failure in name resolution"),
            (42, c"unknown error"),
        ]
        .into_iter()
        .all(|(code, expected)| {
            let text = net::gai_strerror(code);
            // SAFETY: This API returns a pointer to a static NUL-terminated string.
            !text.is_null() && unsafe { core::ffi::CStr::from_ptr(text) } == expected
        }),
    );

    let mut set = net::FdSet { fd_bits: [0xff; 2] };
    net::fd_zero(&mut set);
    logger.record("Descriptor set clears both bytes", set.fd_bits == [0, 0]);
    for fd in [0, 7, 8, 15] {
        net::fd_set(&mut set, fd);
    }
    logger.record(
        "Descriptor set preserves bits across byte boundaries",
        set.fd_bits == [0x81, 0x81]
            && (0..16).all(|fd| net::fd_is_set(&set, fd) == [0, 7, 8, 15].contains(&fd)),
    );
    net::fd_clear(&mut set, 7);
    net::fd_clear(&mut set, 8);
    logger.record(
        "Descriptor clear leaves neighboring bits unchanged",
        set.fd_bits == [1, 0x80]
            && (0..16).all(|fd| net::fd_is_set(&set, fd) == [0, 15].contains(&fd)),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_system_alarms(logger: &mut ProbeLogger) {
    logger.report_step("System alarm callbacks and cancellation");
    ALARM_COUNT.store(0, Ordering::Relaxed);
    let mut alarm = 0;
    let created = system::create_alarm(&mut alarm) == 0 && alarm != 0;
    logger.record("System alarm creation returns a handle", created);
    if !created {
        return;
    }
    let delay = system::TimeSpec {
        tv_sec: 0,
        tv_nsec: 30_000_000,
    };
    let arg = core::ptr::without_provenance_mut::<core::ffi::c_void>(0x1234);
    // SAFETY: The callback treats its argument only as an opaque token.
    let scheduled =
        unsafe { system::set_alarm(alarm, &delay, Some(probe_alarm_callback), arg) } == 0;
    for _ in 0..30 {
        if ALARM_COUNT.load(Ordering::Acquire) >= 1 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let fired = scheduled
        && ALARM_COUNT.load(Ordering::Acquire) == 1
        && ALARM_ID.load(Ordering::Relaxed) == alarm
        && ALARM_ARG.load(Ordering::Relaxed) == 0x1234;
    logger.record("System one-shot alarm delivers handle and argument", fired);
    let periodic = unsafe {
        system::set_periodic_alarm(alarm, &delay, &delay, Some(probe_alarm_callback), arg)
    } == 0;
    let alarm_snapshot = system::snapshot_alarms();
    logger.record(
        "System alarm snapshot records handle and callbacks",
        scheduled
            && periodic
            && alarm_snapshot.alarm == alarm as usize
            && alarm_snapshot.callback == probe_alarm_callback as *const () as usize
            && alarm_snapshot.periodic_callback == probe_alarm_callback as *const () as usize,
    );
    for _ in 0..30 {
        if ALARM_COUNT.load(Ordering::Acquire) >= 4 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let repeated = periodic && ALARM_COUNT.load(Ordering::Acquire) >= 4;
    logger.record(
        "System periodic alarm delivers repeated callbacks",
        repeated,
    );
    let canceled = system::cancel_alarm(alarm) == 0;
    let after_cancel = ALARM_COUNT.load(Ordering::Acquire);
    let mut waited = true;
    for _ in 0..8 {
        waited &= ogc_video::wait_vsync_timeout_us(100_000);
    }
    logger.record(
        "System alarm cancellation stops callbacks",
        canceled && waited && ALARM_COUNT.load(Ordering::Acquire) == after_cancel,
    );
    logger.record(
        "System alarm removal succeeds",
        system::remove_alarm(alarm) == 0,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_raw_system_alarms(logger: &mut ProbeLogger) {
    logger.report_step("Raw system alarm callbacks and state");
    ALARM_COUNT.store(0, Ordering::Relaxed);
    let mut alarm = 0;
    // SAFETY: The output pointer is valid and the callback uses its argument
    // only as an opaque token. A created alarm is cancelled and removed below.
    let created = unsafe { raw_system::sys_create_alarm(&mut alarm) } == 0 && alarm != 0;
    logger.record("Raw system alarm creation returns a handle", created);
    if !created {
        return;
    }
    let delay = raw_system::TimeSpec {
        tv_sec: 0,
        tv_nsec: 30_000_000,
    };
    let arg = core::ptr::without_provenance_mut::<core::ffi::c_void>(0x5678);
    let invalid = unsafe {
        raw_system::sys_create_alarm(core::ptr::null_mut()) == -1
            && raw_system::sys_set_alarm(alarm, core::ptr::null(), Some(probe_alarm_callback), arg)
                == -1
            && raw_system::sys_set_periodic_alarm(
                alarm,
                &delay,
                core::ptr::null(),
                Some(probe_alarm_callback),
                arg,
            ) == -1
    };
    logger.record("Raw system alarms reject invalid inputs", invalid);
    let scheduled =
        unsafe { raw_system::sys_set_alarm(alarm, &delay, Some(probe_alarm_callback), arg) } == 0;
    for _ in 0..30 {
        if ALARM_COUNT.load(Ordering::Acquire) >= 1 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let once = scheduled
        && ALARM_COUNT.load(Ordering::Acquire) == 1
        && ALARM_ID.load(Ordering::Relaxed) == alarm
        && ALARM_ARG.load(Ordering::Relaxed) == 0x5678;
    logger.record(
        "Raw system one-shot alarm delivers handle and argument",
        once,
    );
    let periodic = unsafe {
        raw_system::sys_set_periodic_alarm(alarm, &delay, &delay, Some(probe_alarm_callback), arg)
    } == 0;
    for _ in 0..30 {
        if ALARM_COUNT.load(Ordering::Acquire) >= 4 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    logger.record(
        "Raw system periodic alarm repeats",
        periodic && ALARM_COUNT.load(Ordering::Acquire) >= 4,
    );
    let canceled = unsafe { raw_system::sys_cancel_alarm(alarm) } == 0;
    let after_cancel = ALARM_COUNT.load(Ordering::Acquire);
    let mut waited = true;
    for _ in 0..8 {
        waited &= ogc_video::wait_vsync_timeout_us(100_000);
    }
    logger.record(
        "Raw system alarm cancellation stops callbacks",
        canceled && waited && ALARM_COUNT.load(Ordering::Acquire) == after_cancel,
    );
    let removed = unsafe { raw_system::sys_remove_alarm(alarm) } == 0;
    let state = raw_system::snapshot_alarms();
    logger.record(
        "Raw system alarm removal and state snapshot",
        removed
            && state.created
            && state.cancelled
            && state.removed
            && state.alarm == alarm
            && state.callback == probe_alarm_callback as *const () as usize
            && state.periodic_callback == probe_alarm_callback as *const () as usize,
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_system_sram_locks(logger: &mut ProbeLogger) {
    logger.report_step("System SRAM lock exclusivity");
    // SAFETY: The successful lock is paired with one unlock. Competing lock
    // attempts return null and are not unlocked. No writeback is requested.
    let first = unsafe { system::lock_sram() };
    if first.is_null() {
        logger.record("System standard SRAM lock acquired", false);
        return;
    }
    let standard_busy =
        unsafe { system::lock_sram().is_null() && system::lock_sram_ex().is_null() };
    let expected_standard = unsafe { *first };
    let snapshots_busy = system::snapshot_sram().is_none() && system::snapshot_sram_ex().is_none();
    let standard_release = unsafe { system::unlock_sram(false) };
    logger.record(
        "System standard SRAM lock excludes both lock types",
        standard_busy,
    );
    logger.record("System standard SRAM unlock completes", standard_release);
    logger.record("SRAM snapshots reject a held lock", snapshots_busy);
    let standard_snapshot = system::snapshot_sram();
    let standard_unlock = system::snapshot_sram_unlock().standard;
    logger.record(
        "Standard SRAM snapshot matches locked data",
        standard_snapshot == Some(expected_standard) && standard_unlock,
    );

    let extended = unsafe { system::lock_sram_ex() };
    if extended.is_null() {
        logger.record("System extended SRAM lock acquired after release", false);
        return;
    }
    let extended_busy =
        unsafe { system::lock_sram().is_null() && system::lock_sram_ex().is_null() };
    let expected_extended = unsafe { *extended };
    let extended_release = unsafe { system::unlock_sram_ex(false) };
    logger.record(
        "System extended SRAM lock excludes both lock types",
        extended_busy,
    );
    logger.record("System extended SRAM unlock completes", extended_release);
    let extended_snapshot = system::snapshot_sram_ex();
    let extended_unlock = system::snapshot_sram_unlock().extended;
    logger.record(
        "Extended SRAM snapshot matches locked data",
        extended_snapshot == Some(expected_extended) && extended_unlock,
    );
    let sync_result = system::sync_sram();
    let sync_snapshot = system::snapshot_sram_sync();
    logger.record(
        "SRAM sync snapshot reports the actual result",
        sync_snapshot.called && sync_snapshot.last_result == sync_result,
    );
    let reacquired = unsafe { system::lock_sram() };
    let released = if reacquired.is_null() {
        false
    } else {
        unsafe { system::unlock_sram(false) }
    };
    logger.record(
        "System SRAM lock can be reacquired after release",
        !reacquired.is_null() && released,
    );
}

fn probe_system(logger: &mut ProbeLogger) {
    #[cfg(feature = "dolphin-automation")]
    {
        let arenas = system::snapshot_arenas();
        // SAFETY: These calls read the runtime arena bounds.
        let raw = unsafe {
            (
                raw_system::sys_get_arena1_lo(),
                raw_system::sys_get_arena1_hi(),
                raw_system::sys_get_arena2_lo(),
                raw_system::sys_get_arena2_hi(),
            )
        };
        logger.record(
            "System arena snapshot matches runtime bounds",
            arenas.arena1_lo == raw.0
                && arenas.arena1_hi == raw.1
                && arenas.arena2_lo == raw.2
                && arenas.arena2_hi == raw.3
                && (raw.0 as usize) < raw.1 as usize
                && (raw.2 as usize) < raw.3 as usize,
        );
    }
    // The runtime has already initialized IOS. Repeating pre_main closes
    // live descriptors and invalidates the services this probe must test.
    logger.note_hex32("hollywood revision", system::get_hollywood_revision());
    let revision = ogc_core::powerpc::hollywood_revision();
    logger.record(
        "core hollywood revision matches system",
        revision == system::get_hollywood_revision(),
    );
    #[cfg(feature = "dolphin-automation")]
    {
        let proof = system::hardware_proof();
        let snapshot = system::snapshot_hardware();
        logger.record(
            "System hardware getters match Dolphin default values",
            system::get_font_encoding() == 0
                && system::reset_button_down() == 0
                && system::get_hollywood_revision() == 0x11
                && system::get_hollywood_revision() == revision,
        );
        logger.note_u32("Proof font encoding", proof.font_encoding);
        logger.note_u32("Proof reset button", proof.reset_button_down);
        logger.note_hex32("Proof Hollywood revision", proof.hollywood_revision);
        logger.note_u32(
            "Snapshot font queried",
            u32::from(snapshot.font_encoding_read_called),
        );
        logger.note_u32(
            "Snapshot reset queried",
            u32::from(snapshot.reset_button_read_called),
        );
        logger.note_u32(
            "Snapshot Hollywood queried",
            u32::from(snapshot.hollywood_revision_read_called),
        );
        logger.record(
            "System hardware proof records Dolphin observations",
            proof.hollywood_revision == 0x11
                && proof.hollywood_revision == revision
                && proof.font_encoding == 0
                && proof.reset_button_down == 0
                && proof.time_monotonic
                && proof.time_after >= proof.time_before
                && snapshot.hollywood_revision == 0x11
                && snapshot.font_encoding == 0
                && snapshot.reset_button_down == 0
                && snapshot.hollywood_revision_read_called
                && snapshot.font_encoding_read_called
                && snapshot.reset_button_read_called,
        );
    }
    let timebase_before = ogc_core::powerpc::read_timebase();
    let immediate = ogc_core::powerpc::read_timebase();
    // A timebase tick need not occur between adjacent reads. Bound polling by
    // iterations so a stopped timebase cannot hang its own test.
    let mut timebase_after = immediate;
    for _ in 0..100_000 {
        if timebase_after != timebase_before {
            break;
        }
        core::hint::spin_loop();
        timebase_after = ogc_core::powerpc::read_timebase();
    }
    let timebase_delta = timebase_after.wrapping_sub(timebase_before);
    logger.note_u64(
        "PowerPC immediate delta",
        immediate.wrapping_sub(timebase_before),
    );
    logger.note_u64("PowerPC timebase", timebase_after);
    logger.note_u64("PowerPC timebase delta", timebase_delta);
    logger.record(
        "PowerPC timebase advances",
        timebase_delta > 0 && timebase_delta < (1u64 << 63),
    );
    logger.note_u32("font encoding", system::get_font_encoding());
    logger.note_u32("arena1 size", system::get_arena1_size());
    logger.note_u32("arena2 size", system::get_arena2_size());
    logger.note_u64("system time", system::time());
    system::dump_pmc();
    let pmc = system::sys::snapshot_pmc();
    logger.record("performance counters read", pmc.dumped);
    logger.note_u32("pmc load/stores", pmc.pmc1);
    logger.note_u32("pmc miss cycles", pmc.pmc2);
    logger.note_u32("pmc cycles", pmc.pmc3);
    logger.note_u32("pmc instructions", pmc.pmc4);
}

fn probe_ios_stm(logger: &mut ProbeLogger) {
    logger.section("IOS STM PROBE");
    logger.note_i32("ios preferred", ios::get_preferred_version());
    logger.note_i32("ios version", ios::get_version());
    logger.note_i32("ios revision", ios::get_revision());
    logger.note_i32("ios revision major", ios::get_revision_major());
    logger.note_i32("ios revision minor", ios::get_revision_minor());

    let led = stm::set_led_mode(0);
    logger.note_i32("stm led rc", led);
    logger.record("stm reachable", led == 0);
}

fn probe_conf(logger: &mut ProbeLogger) {
    logger.section("CONF PROBE");
    let init = conf::init();
    logger.note_i32("conf init rc", init);
    if init != conf::CONF_ERR_OK {
        return;
    }

    logger.note_i32("conf video", conf::get_video());
    logger.note_i32("conf region", conf::get_region());
    logger.note_i32("conf area", conf::get_area());
    logger.note_i32("conf language", conf::get_language());
    logger.note_i32("conf sound", conf::get_sound_mode());
    logger.note_i32("conf aspect", conf::get_aspect_ratio());
    logger.note_i32("conf progressive", conf::get_progressive_scan());
    logger.note_i32("conf sensorbar", conf::get_sensor_bar_position());

    let mut bias = 0u32;
    let bias_rc = conf::get_counter_bias(&mut bias);
    logger.note_i32("conf counter bias rc", bias_rc);
    logger.note_u32("conf counter bias", bias);

    let mut offset = 0i8;
    let offset_rc = conf::get_display_offset_h(&mut offset);
    logger.note_i32("conf display offset rc", offset_rc);
    logger.note_i32("conf display offset", offset as i32);

    let mut pads = conf::ConfPads::default();
    let pads_rc = conf::get_pad_devices(&mut pads);
    logger.note_i32("conf pad devices rc", pads_rc);
    logger.note_u32("conf registered pads", pads.num_registered as u32);
}

#[cfg(feature = "dolphin-automation")]
fn probe_conf_fixture(logger: &mut ProbeLogger) {
    logger.report_step("Wii configuration fixture values and mutation");
    let initialized = conf::init() == conf::CONF_ERR_OK;
    logger.record("CONF initialization is repeatable", initialized);
    if !initialized {
        return;
    }

    let keys = [
        "IPL.LNG", "IPL.SND", "IPL.AR", "IPL.PGS", "IPL.E60", "BT.BAR", "BT.SPKV", "BT.MOT",
        "IPL.SSV",
    ];
    let expected = [4u8, 2, 0, 1, 1, 0, 42, 0, 0];
    let mut original = [0u8; 9];
    let mut readable = true;
    for (index, key) in keys.iter().enumerate() {
        readable &= conf::get(key, &mut original[index..index + 1]) == 1;
    }
    logger.record("CONF byte settings are readable", readable);
    if !readable {
        return;
    }
    let mut value = [0xa5; 1];
    let metadata = conf::get_length("IPL.LNG") == 1
        && conf::get_type("IPL.LNG") == i32::from(conf::CONF_BYTE)
        && conf::get("IPL.LNG", &mut value) == 1
        && value[0] == original[0];
    logger.record("CONF generic length type and value", metadata);
    let mut empty = [];
    let missing = conf::get_length("NO.SUCH.KEY") == conf::CONF_ENOENT
        && conf::get_type("NO.SUCH.KEY") == conf::CONF_ENOENT
        && conf::get("NO.SUCH.KEY", &mut value) == conf::CONF_ENOENT
        && conf::get("IPL.LNG", &mut empty) == conf::CONF_ETOOBIG
        && conf::set("IPL.LNG", &[]) == conf::CONF_EBADVALUE;
    logger.record("CONF missing and wrong length inputs fail", missing);

    let mut changed = true;
    for (key, byte) in keys.iter().zip(expected) {
        changed &= conf::set(key, &[byte]) == 1;
    }
    logger.record("CONF known byte values are set in memory", changed);
    logger.record(
        "CONF language reads selected value",
        conf::get_language() == 4,
    );
    logger.record(
        "CONF sound mode reads selected value",
        conf::get_sound_mode() == 2,
    );
    logger.record(
        "CONF aspect ratio reads selected value",
        conf::get_aspect_ratio() == 0,
    );
    logger.record(
        "CONF progressive mode reads selected value",
        conf::get_progressive_scan() == 1,
    );
    logger.record(
        "CONF RGB60 mode reads selected value",
        conf::get_eu_rgb60() == 1,
    );
    logger.record(
        "CONF sensor bar reads selected value",
        conf::get_sensor_bar_position() == 0,
    );
    logger.record(
        "CONF speaker volume reads selected value",
        conf::get_pad_speaker_volume() == 42,
    );
    logger.record(
        "CONF motor mode reads selected value",
        conf::get_pad_motor_mode() == 0,
    );
    logger.record(
        "CONF screen saver reads selected value",
        conf::get_screen_saver_mode() == 0,
    );

    // Restore every value, including after a failed check. Do not save changes.
    let mut restored = true;
    for (key, byte) in keys.iter().zip(original) {
        restored &= conf::set(key, &[byte]) == 1;
        let mut current = [0];
        restored &= conf::get(key, &mut current) == 1 && current == [byte];
    }
    logger.record("CONF in-memory settings are restored", restored);
}

#[cfg(feature = "dolphin-automation")]
fn check_conf_temporary(key: &str, value: &[u8], check: impl FnOnce() -> bool) -> bool {
    let mut original = [0u8; 74];
    if value.len() > original.len()
        || conf::get(key, &mut original[..value.len()]) != value.len() as i32
    {
        return false;
    }
    let changed = conf::set(key, value) == value.len() as i32;
    let checked = check();
    let restored = conf::set(key, &original[..value.len()]) == value.len() as i32;
    let mut actual = [0u8; 74];
    let verified = conf::get(key, &mut actual[..value.len()]) == value.len() as i32
        && actual[..value.len()] == original[..value.len()];
    changed && checked && restored && verified
}

#[cfg(feature = "dolphin-automation")]
fn probe_conf_typed_values(logger: &mut ProbeLogger) {
    logger.report_step("Wii configuration typed values");
    let idle = check_conf_temporary("IPL.IDL", &[1, 2], || {
        conf::get_shutdown_mode() == 1 && conf::get_idle_led_mode() == 2
    });
    logger.record("CONF shutdown and LED decode two-byte setting", idle);
    let bias = check_conf_temporary("IPL.CB", &[0x12, 0x34, 0x56, 0x78], || {
        let mut value = 0;
        conf::get_counter_bias(&mut value) == 0 && value == 0x1234_5678
    });
    logger.record("CONF counter bias decodes big endian word", bias);
    let sensitivity = check_conf_temporary("BT.SENS", &[1, 2, 3, 4], || {
        conf::get_ir_sensitivity() == 0x0102_0304
    });
    logger.record("CONF IR sensitivity decodes big endian word", sensitivity);
    let network = check_conf_temporary("NET.WCFG", &[0x12, 0x34, 0x56, 0x78], || {
        conf::get_wii_connect24() == 0x1234_5678
    });
    logger.record("CONF WiiConnect24 decodes big endian word", network);
    let offset = check_conf_temporary("IPL.DH", &[0xfb], || {
        let mut value = 0;
        conf::get_display_offset_h(&mut value) == 0 && value == -5
    });
    logger.record("CONF horizontal display offset is signed", offset);
    let eula = check_conf_temporary("IPL.EULA", &[1], || conf::get_eula() == 1);
    logger.record("CONF EULA decodes boolean", eula);
    let nickname = [
        0, b'A', 0, b'B', 0, b'C', 0, b'D', 0, b'E', 0, b'F', 0, b'G', 0, b'H', 0, b'I', 0, b'J',
        0, 0,
    ];
    let named = check_conf_temporary("IPL.NIK", &nickname, || {
        let mut value = [0xa5; 11];
        conf::get_nick_name(&mut value) == 22 && value == *b"ABCDEFGHIJ\0"
    });
    logger.record("CONF nickname decodes ten big endian characters", named);
    let empty = check_conf_temporary("IPL.NIK", &[0; 22], || {
        let mut value = [0x5a; 11];
        conf::get_nick_name(&mut value) == conf::CONF_EBADVALUE && value == [0x5a; 11]
    });
    logger.record(
        "CONF rejects an empty nickname without output changes",
        empty,
    );
    let parental = check_conf_temporary("IPL.PC", &[0x41; 74], || {
        let mut password = [7; 5];
        let mut answer = [7; 33];
        conf::get_parental_password(&mut password) == conf::CONF_EBADVALUE
            && conf::get_parental_answer(&mut answer) == conf::CONF_EBADVALUE
            && password == [7; 5]
            && answer == [7; 33]
    });
    logger.record("CONF parental length error preserves outputs", parental);
}

#[cfg(feature = "dolphin-automation")]
fn probe_conf_pad_records(logger: &mut ProbeLogger) {
    logger.report_step("Wii configuration controller records");
    let mut original = conf::ConfPads::default();
    let mut original_guests = conf::ConfPadGuests::default();
    let ready = conf::get_pad_devices(&mut original) == 0
        && conf::get_pad_guest_devices(&mut original_guests) == 0;
    logger.record("CONF controller records are readable", ready);
    if !ready {
        return;
    }

    let mut pads = conf::ConfPads::default();
    pads.num_registered = conf::CONF_PAD_MAX_REGISTERED as u8;
    pads.registered[0].bdaddr = [1, 2, 3, 4, 5, 6];
    pads.registered[0].name[0..4].copy_from_slice(b"TEST");
    pads.active[3].bdaddr = [9, 8, 7, 6, 5, 4];
    let mut actual = conf::ConfPads::default();
    let set = conf::set_pad_devices(&pads) == core::mem::size_of::<conf::ConfPads>() as i32;
    let read = conf::get_pad_devices(&mut actual) == 0 && actual == pads;
    let mut raw = [0u8; core::mem::size_of::<conf::ConfPads>()];
    let bytes = conf::get("BT.DINF", &mut raw) == raw.len() as i32
        && raw[0] == 10
        && raw[1..7] == [1, 2, 3, 4, 5, 6]
        && raw[7..11] == *b"TEST";
    logger.record(
        "CONF registered controller records and byte layout",
        set && read && bytes,
    );
    let mut invalid = pads;
    invalid.num_registered = 11;
    let rejected = conf::set_pad_devices(&invalid) == conf::CONF_EBADVALUE
        && conf::get_pad_devices(&mut actual) == 0
        && actual == pads;
    logger.record("CONF rejects too many registered controllers", rejected);
    let restored = conf::set_pad_devices(&original) == raw.len() as i32
        && conf::get_pad_devices(&mut actual) == 0
        && actual == original;
    logger.record("CONF registered controller record is restored", restored);

    let mut guests = conf::ConfPadGuests::default();
    guests.num_guests = conf::CONF_PAD_MAX_ACTIVE as u8;
    guests.guests[0].bdaddr = [6, 5, 4, 3, 2, 1];
    guests.guests[0].name[0..4].copy_from_slice(b"GUES");
    guests.guests[0].link_key = [0x5a; 16];
    let mut actual_guests = conf::ConfPadGuests::default();
    let size = core::mem::size_of::<conf::ConfPadGuests>();
    let set = conf::set_pad_guest_devices(Some(&guests)) == size as i32;
    let read = conf::get_pad_guest_devices(&mut actual_guests) == 0 && actual_guests == guests;
    let mut raw_guests = [0u8; core::mem::size_of::<conf::ConfPadGuests>()];
    let bytes = conf::get("BT.CDIF", &mut raw_guests) == size as i32
        && raw_guests[0] == 4
        && raw_guests[1..7] == [6, 5, 4, 3, 2, 1]
        && raw_guests[7..11] == *b"GUES"
        && raw_guests[71..87] == [0x5a; 16];
    logger.record(
        "CONF guest controller records and byte layout",
        set && read && bytes,
    );
    let mut invalid_guests = guests;
    invalid_guests.num_guests = 5;
    let rejected = conf::set_pad_guest_devices(Some(&invalid_guests)) == conf::CONF_EBADVALUE
        && conf::get_pad_guest_devices(&mut actual_guests) == 0
        && actual_guests == guests;
    logger.record("CONF rejects too many guest controllers", rejected);
    let cleared = conf::set_pad_guest_devices(None) == size as i32
        && conf::get_pad_guest_devices(&mut actual_guests) == 0
        && actual_guests == conf::ConfPadGuests::default();
    logger.record("CONF null guest input clears guest records", cleared);
    let restored = conf::set_pad_guest_devices(Some(&original_guests)) == size as i32
        && conf::get_pad_guest_devices(&mut actual_guests) == 0
        && actual_guests == original_guests;
    logger.record("CONF guest controller record is restored", restored);
}

#[cfg(feature = "dolphin-automation")]
fn probe_conf_text_settings(logger: &mut ProbeLogger) {
    logger.report_step("Wii configuration text settings");
    #[repr(align(32))]
    struct Settings([u8; 256]);
    let Ok(file) = conf::ipc::open_file(
        c"/title/00000001/00000002/data/setting.txt",
        conf::ipc::IPC_OPEN_READ,
    ) else {
        logger.record("CONF text settings file opens", false);
        return;
    };
    let mut bytes = Settings([0; 256]);
    let read = file.read(&mut bytes.0) == Ok(256);
    let closed = file.close().is_ok();
    logger.record("CONF text settings file reads and closes", read && closed);
    if !read || !closed {
        return;
    }

    // Decode an independent IOS read, rather than using the cached parser.
    let mut key = 0x73b5_dbfau32;
    for byte in &mut bytes.0 {
        *byte ^= key as u8;
        key = key.rotate_left(1);
    }
    let field = |prefix: &[u8]| {
        bytes.0.split(|byte| *byte == b'\n').find_map(|line| {
            let value = line.strip_prefix(prefix)?;
            Some(
                value
                    .split(|byte| *byte == b'\r' || *byte == 0)
                    .next()
                    .unwrap_or(&[]),
            )
        })
    };
    let region = match field(b"GAME=") {
        Some(b"JP") => Some(0),
        Some(b"US") => Some(1),
        Some(b"EU") => Some(2),
        Some(b"KR") => Some(4),
        Some(b"CN") => Some(5),
        _ => None,
    };
    let area = match field(b"AREA=") {
        Some(b"JPN") => Some(0),
        Some(b"USA") => Some(1),
        Some(b"EUR") => Some(2),
        Some(b"AUS") => Some(3),
        Some(b"BRA") => Some(4),
        Some(b"TWN") => Some(5),
        Some(b"ROC") => Some(6),
        Some(b"KOR") => Some(7),
        Some(b"HKG") => Some(8),
        Some(b"ASI") => Some(9),
        Some(b"LTN") => Some(10),
        Some(b"SAF") => Some(11),
        Some(b"CHN") => Some(12),
        _ => None,
    };
    let video = match field(b"VIDEO=") {
        Some(b"NTSC") => Some(0),
        Some(b"PAL") => Some(1),
        Some(b"MPAL") => Some(2),
        _ => None,
    };
    logger.record(
        "CONF region matches decoded GAME field",
        region == Some(conf::get_region()),
    );
    logger.record(
        "CONF area matches decoded AREA field",
        area == Some(conf::get_area()),
    );
    logger.record(
        "CONF video matches decoded VIDEO field",
        video == Some(conf::get_video()),
    );
}

fn probe_es(logger: &mut ProbeLogger) {
    logger.section("ES PROBE");
    let init = es::init();
    logger.note_i32("es init rc", init);
    if init < 0 {
        return;
    }

    let mut title_id = 0u64;
    // SAFETY: each argument below is a valid unique output reference.
    logger.note_i32("es title id rc", unsafe { es::get_title_id(&mut title_id) });
    logger.note_hex32("es title hi", (title_id >> 32) as u32);
    logger.note_hex32("es title lo", title_id as u32);

    let mut device_id = 0u32;
    logger.note_i32("es device id rc", unsafe {
        es::get_device_id(&mut device_id)
    });
    logger.note_hex32("es device id", device_id);

    let mut boot2 = 0u32;
    logger.note_i32("es boot2 rc", unsafe { es::get_boot2_version(&mut boot2) });
    logger.note_u32("es boot2", boot2);
    logger.note_i32("es korean key rc", es::check_has_korean_key());
    logger.note_i32("es close rc", es::close());
}

#[cfg(feature = "dolphin-automation")]
fn probe_raw_es_catalog(logger: &mut ProbeLogger) {
    use es::sys;
    const MENU: u64 = 0x0000_0001_0000_0002;
    logger.section("RAW ES CATALOG");
    let initialized = sys::init();
    logger.record("Raw ES initializes", initialized == 0);
    if initialized != 0 {
        return;
    }
    let mut count = 0u32;
    // SAFETY: Each buffer is writable for its stated capacity.
    let rc = unsafe { sys::get_num_titles(&mut count) };
    logger.note_u32("Raw ES title count", count);
    logger.record(
        "Raw ES title count is bounded and nonzero",
        rc == 0 && count > 0 && count <= 512,
    );
    if rc == 0 && count > 0 && count <= 512 {
        let mut titles = [u64::MAX; 513];
        let list = unsafe { sys::get_titles(titles.as_mut_ptr(), count) };
        logger.record(
            "Raw ES title list includes system menu",
            list == 0
                && titles[..count as usize].contains(&MENU)
                && titles[count as usize..].iter().all(|x| *x == u64::MAX),
        );
    }
    let mut owned_count = 0u32;
    let rc = unsafe { sys::get_num_owned_titles(&mut owned_count) };
    logger.note_u32("Raw ES owned title count", owned_count);
    logger.record(
        "Raw ES owned title count is bounded and nonzero",
        rc == 0 && owned_count > 0 && owned_count <= 512,
    );
    if rc == 0 && owned_count > 0 && owned_count <= 512 {
        let mut titles = [u64::MAX; 513];
        let list = unsafe { sys::get_owned_titles(titles.as_mut_ptr(), owned_count) };
        logger.record(
            "Raw ES owned title list includes system menu",
            list == 0
                && titles[..owned_count as usize].contains(&MENU)
                && titles[owned_count as usize..]
                    .iter()
                    .all(|x| *x == u64::MAX),
        );
    }
    let mut path = [0xa5u8; 32];
    let dir = unsafe { sys::get_data_dir(MENU, path.as_mut_ptr()) };
    logger.note_i32("Raw ES data path result", dir);
    for chunk in path.chunks_exact(4) {
        logger.note_hex32(
            "Raw ES data path bytes",
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        );
    }
    logger.record(
        "Raw ES Dolphin writes 29 path bytes without terminator",
        dir == 0 && &path[..29] == b"/title/00000001/00000002/data" && path[29..] == [0xa5; 3],
    );
    let mut size = 0u32;
    let rc = unsafe { sys::get_stored_tmd_size(MENU, &mut size) };
    logger.note_i32("Raw ES stored TMD size result", rc);
    logger.note_u32("Raw ES stored TMD size", size);
    logger.record(
        "Raw ES system menu TMD size matches NAND fixture",
        rc == 0 && size == 808,
    );
    let mut content_count = 0u32;
    let rc = unsafe { sys::get_title_contents_count(MENU, &mut content_count) };
    logger.note_i32("Raw ES title contents count result", rc);
    logger.note_u32("Raw ES title contents count", content_count);
    logger.record(
        "Raw ES system menu content count matches TMD",
        rc == 0 && content_count == 9,
    );
    if rc == 0 && content_count == 9 {
        let mut ids = [u32::MAX; 10];
        let listed = unsafe { sys::get_title_contents(MENU, ids.as_mut_ptr(), 9) };
        logger.record(
            "Raw ES system menu content IDs match TMD",
            listed == 0
                && ids[..9] == [0x96, 0x97, 9, 10, 11, 12, 0x59, 0x5a, 0x98]
                && ids[9] == u32::MAX,
        );
    }
    #[repr(align(64))]
    struct TmdBytes([u8; 832]);
    let mut tmd = TmdBytes([0xa5; 832]);
    let tmd_rc = unsafe { sys::get_stored_tmd(MENU, tmd.0.as_mut_ptr().cast(), 808) };
    logger.note_i32("Raw ES stored TMD result", tmd_rc);
    let mut digest = [0u8; 20];
    let hash_init = sha::init();
    let hashed = hash_init == 0 && sha::calculate(&tmd.0[..808], &mut digest) == 0;
    let hash_close = if hash_init == 0 { sha::close() } else { -1 };
    logger.record(
        "Raw ES stored TMD matches fixture digest and guard",
        tmd_rc == 0
            && hashed
            && hash_close == 0
            && digest
                == [
                    0x8f, 0x9e, 0x05, 0xf8, 0xd8, 0xd0, 0xb3, 0x41, 0x9e, 0xa4, 0xb8, 0x7e, 0x37,
                    0xa4, 0x07, 0x4a, 0x3a, 0x32, 0x9a, 0xeb,
                ]
            && tmd.0[808..].iter().all(|b| *b == 0xa5),
    );
    if tmd_rc == 0 {
        let mut stored_count = u32::MAX;
        let rc = unsafe {
            sys::get_num_stored_tmd_contents(tmd.0.as_ptr().cast(), 808, &mut stored_count)
        };
        logger.note_i32("Raw ES stored content count result", rc);
        logger.note_u32("Raw ES stored content count", stored_count);
        logger.record(
            "Raw ES stored content count matches NAND files",
            rc == 0 && stored_count == 9,
        );
        if rc == 0 && stored_count == 9 {
            let mut ids = [u32::MAX; 10];
            let listed = unsafe {
                sys::get_stored_tmd_contents(tmd.0.as_ptr().cast(), 808, ids.as_mut_ptr(), 9)
            };
            logger.note_i32("Raw ES stored content list result", listed);
            for id in ids {
                logger.note_hex32("Raw ES stored content ID", id);
            }
            logger.record(
                "Raw ES rejected TMD leaves stored ID buffer unchanged",
                listed == -2011 && ids.iter().all(|id| *id == u32::MAX),
            );
        }
    }
    let mut shared_count = u32::MAX;
    let rc = unsafe { sys::get_num_shared_contents(&mut shared_count) };
    logger.note_i32("Raw ES shared content count result", rc);
    logger.note_u32("Raw ES shared content count", shared_count);
    logger.record(
        "Raw ES shared content count matches NAND files",
        rc == 0 && shared_count == 120,
    );
    if rc == 0 && shared_count == 120 {
        #[repr(align(32))]
        struct Hashes([[u8; 20]; 121]);
        let mut hashes = Hashes([[0xa5; 20]; 121]);
        let listed = unsafe { sys::get_shared_contents(hashes.0.as_mut_ptr(), 120) };
        logger.record(
            "Raw ES shared content list matches fixture and guard",
            listed == 0
                && hashes.0[0]
                    == [
                        0xfb, 0x30, 0x8a, 0x9a, 0x1d, 0x93, 0x41, 0xdf, 0x95, 0x17, 0xdb, 0x15,
                        0x5f, 0x43, 0x83, 0x16, 0x23, 0x25, 0xdc, 0xc0,
                    ]
                && hashes.0[120] == [0xa5; 20],
        );
    }
    let mut views_count = 0u32;
    let ticket_rc = unsafe { sys::get_num_ticket_views(MENU, &mut views_count) };
    logger.note_i32("Raw ES ticket count result", ticket_rc);
    logger.note_u32("Raw ES ticket count", views_count);
    logger.record(
        "Raw ES system menu has one ticket view",
        ticket_rc == 0 && views_count == 1,
    );
    if ticket_rc == 0 && views_count == 1 {
        #[repr(align(32))]
        struct View(sys::Tikview);
        let mut view = View(sys::Tikview::default());
        let rc = unsafe { sys::get_ticket_views(MENU, &mut view.0, 1) };
        let title = view.0.titleid;
        logger.record(
            "Raw ES ticket view identifies system menu",
            rc == 0 && title == MENU,
        );
    }
    let mut view_size = 0u32;
    let view_rc = unsafe { sys::get_tmd_view_size(MENU, &mut view_size) };
    logger.note_i32("Raw ES TMD view size result", view_rc);
    logger.note_u32("Raw ES TMD view size", view_size);
    logger.record(
        "Raw ES TMD view size fits system menu",
        view_rc == 0 && view_size == 236,
    );
    if view_rc == 0 && view_size == 236 {
        #[repr(align(32))]
        struct ViewBytes([u8; 256]);
        let mut view = ViewBytes([0xa5; 256]);
        let rc = unsafe { sys::get_tmd_view(MENU, view.0.as_mut_ptr().cast(), view_size) };
        logger.record(
            "Raw ES TMD view identifies menu and content count",
            rc == 0
                && &view.0[12..20] == &MENU.to_be_bytes()
                && &view.0[90..92] == &[0, 9]
                && view.0[236..].iter().all(|b| *b == 0xa5),
        );
    }
    let mut boot2 = 0u32;
    let rc = unsafe { sys::get_boot2_version(&mut boot2) };
    logger.record(
        "Raw ES boot2 version matches Dolphin fixture",
        rc == 0 && boot2 == 4,
    );
    logger.record("Raw ES closes", sys::close() == 0);
}

fn probe_crypto(logger: &mut ProbeLogger) {
    logger.section("CRYPTO PROBE");

    let sha_init = sha::init();
    logger.record("sha init", sha_init == 0);

    let mut context = sha::AlignedShaContext::new();
    let aligned = (context.as_ptr() as usize) & 31 == 0;
    let mutable_pointer_matches = context.as_mut_ptr() == &mut context.context;
    let mut plain_context = sha::ShaContext::new();
    plain_context.set_bit_length(u64::MAX);
    let length_roundtrip = plain_context.bit_length() == u64::MAX;

    let mut context_digest = [0u8; 20];
    let full_block = [b'a'; 64];
    let context_rc = sha::initialize_context(&mut context);
    let input_rc = sha::input(&mut context, &full_block);
    let length_updated = context.context.bit_length() == 512;
    let finalize_rc = sha::finalize(&mut context, b"x", &mut context_digest);
    let expected_digest = [
        0x8e, 0x18, 0x94, 0x0b, 0x19, 0xbe, 0xf3, 0x15, 0xd3, 0x42, 0x61, 0xeb, 0xf8, 0x26, 0x6d,
        0xd2, 0x9a, 0x1f, 0x3e, 0x66,
    ];
    logger.record(
        "sha context helpers",
        aligned
            && mutable_pointer_matches
            && length_roundtrip
            && context_rc == 0
            && input_rc == 0
            && length_updated
            && finalize_rc == 0
            && context_digest == expected_digest,
    );

    let mut digest = [0u8; 20];
    logger.record("sha calculate", sha::calculate(b"abc", &mut digest) == 0);
    logger.record(
        "sha abc vector",
        digest
            == [
                0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
                0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
            ],
    );
    logger.record("sha close", sha::close() == 0);

    let key = Align16([0; 16]);
    let mut iv = Align16([0; 16]);
    let input = Align16([0; 16]);
    let mut output = Align16([0; 16]);
    // SAFETY: all buffers are 16-byte aligned, correctly sized, live for the
    // call, and occupy distinct storage.
    let aes_rc = unsafe {
        aes::aes_encrypt(aes::AesCommandConfig {
            key: key.0.as_ptr().cast(),
            key_size: 15,
            iv: iv.0.as_mut_ptr().cast(),
            iv_size: 16,
            in_data: input.0.as_ptr().cast(),
            out_data: output.0.as_mut_ptr().cast(),
            data_size: 16,
        })
    };
    logger.note_i32("aes bad key rc", aes_rc);
    logger.record("aes rejects bad key size", aes_rc == -4);

    let init = aes::aes_init();
    logger.note_i32("aes init rc", init);
    if init == 0 {
        let mut encrypt_iv = Align16([0; 16]);
        let mut decrypt_iv = Align16([0; 16]);
        let mut ciphertext = Align16([0; 16]);
        let mut decrypted = Align16([0; 16]);
        let plaintext = Align16(*b"Rust AES test!!!");
        // SAFETY: all buffers are 16-byte aligned, correctly sized, live for
        // each call, and writable buffers do not alias other arguments.
        let encrypt_rc = unsafe {
            aes::aes_encrypt(aes::AesCommandConfig {
                key: key.0.as_ptr().cast(),
                key_size: 16,
                iv: encrypt_iv.0.as_mut_ptr().cast(),
                iv_size: 16,
                in_data: plaintext.0.as_ptr().cast(),
                out_data: ciphertext.0.as_mut_ptr().cast(),
                data_size: 16,
            })
        };
        let decrypt_rc = if encrypt_rc == 0 {
            // SAFETY: the inputs and outputs meet the same requirements as
            // the encrypt call above, with a reset IV for the block.
            unsafe {
                aes::aes_decrypt(aes::AesCommandConfig {
                    key: key.0.as_ptr().cast(),
                    key_size: 16,
                    iv: decrypt_iv.0.as_mut_ptr().cast(),
                    iv_size: 16,
                    in_data: ciphertext.0.as_ptr().cast(),
                    out_data: decrypted.0.as_mut_ptr().cast(),
                    data_size: 16,
                })
            }
        } else {
            -1
        };
        logger.record(
            "aes encrypt/decrypt roundtrip",
            encrypt_rc == 0 && decrypt_rc == 0 && decrypted.0 == plaintext.0,
        );
        logger.note_i32("aes close rc", aes::aes_close());
    }
}

// A controlled read-only medium exercises the production mount path.
static ISO_CASE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
static ISFS_CALLBACK_DONE: AtomicBool = AtomicBool::new(false);
static ISFS_CALLBACK_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

unsafe extern "C" fn isfs_probe_callback(result: i32, _userdata: *mut core::ffi::c_void) -> i32 {
    ISFS_CALLBACK_RESULT.store(result, Ordering::Release);
    ISFS_CALLBACK_DONE.store(true, Ordering::Release);
    filesystem::ISFS_OK
}

fn reset_isfs_callback() {
    ISFS_CALLBACK_RESULT.store(i32::MIN, Ordering::Release);
    ISFS_CALLBACK_DONE.store(false, Ordering::Release);
}

fn wait_isfs_callback() -> bool {
    for _ in 0..1000 {
        if ISFS_CALLBACK_DONE.load(Ordering::Acquire) {
            return true;
        }
        lwp::sleep_ticks((lwp::TICKS_PER_SECOND / 100).max(1));
    }
    ISFS_CALLBACK_DONE.load(Ordering::Acquire)
}

fn run_isfs_async_and_wait(start: impl FnOnce() -> i32) -> (i32, i32) {
    reset_isfs_callback();
    let submission = start();
    if submission < 0 {
        return (submission, i32::MIN);
    }
    while !ISFS_CALLBACK_DONE.load(Ordering::Acquire) {
        lwp::sleep_ticks(1);
    }
    (submission, ISFS_CALLBACK_RESULT.load(Ordering::Acquire))
}

fn run_isfs_async_and_wait_bounded(start: impl FnOnce() -> i32) -> (i32, bool, i32) {
    reset_isfs_callback();
    let submission = start();
    if submission < 0 {
        return (submission, false, i32::MIN);
    }
    let completed = wait_isfs_callback();
    let result = if completed {
        ISFS_CALLBACK_RESULT.load(Ordering::Acquire)
    } else {
        i32::MIN
    };
    (submission, completed, result)
}

fn isfs_file_exists(path: &core::ffi::CStr) -> Option<bool> {
    match filesystem::open(path, filesystem::ISFS_OPEN_READ) {
        Ok(file) => Some(file.close() == filesystem::ISFS_OK),
        Err(filesystem::ISFS_ENOENT) => Some(false),
        Err(_) => None,
    }
}

fn isfs_directory_exists(path: &core::ffi::CStr) -> Option<bool> {
    let mut usage_a = 0;
    let mut usage_b = 0;
    match filesystem::get_usage(path, &mut usage_a, &mut usage_b) {
        filesystem::ISFS_OK => Some(true),
        filesystem::ISFS_ENOENT => Some(false),
        _ => None,
    }
}

fn cleanup_isfs_test_tree(directory: &core::ffi::CStr, file: &core::ffi::CStr) -> bool {
    let file_removed = match isfs_file_exists(file) {
        Some(true) => filesystem::delete(file) == filesystem::ISFS_OK,
        Some(false) => true,
        None => false,
    };
    let directory_removed = match isfs_directory_exists(directory) {
        Some(true) if file_removed => filesystem::delete(directory) == filesystem::ISFS_OK,
        Some(false) => true,
        _ => false,
    };
    file_removed && directory_removed
}

unsafe extern "C" fn iso_ready() -> bool {
    true
}

fn write_iso_directory_record(
    bytes: &mut [u8],
    offset: &mut usize,
    sector: u32,
    size: u32,
    flags: u8,
    name: &[u8],
) -> bool {
    let record_len = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
    let Some(record_end) = offset.checked_add(record_len) else {
        return false;
    };
    if name.len() > u8::MAX as usize || record_end > bytes.len() {
        return false;
    }

    bytes[*offset] = record_len as u8;
    bytes[*offset + 2..*offset + 6].copy_from_slice(&sector.to_le_bytes());
    bytes[*offset + 6..*offset + 10].copy_from_slice(&sector.to_be_bytes());
    bytes[*offset + 10..*offset + 14].copy_from_slice(&size.to_le_bytes());
    bytes[*offset + 14..*offset + 18].copy_from_slice(&size.to_be_bytes());
    bytes[*offset + 25] = flags;
    bytes[*offset + 28..*offset + 30].copy_from_slice(&1u16.to_le_bytes());
    bytes[*offset + 30..*offset + 32].copy_from_slice(&1u16.to_be_bytes());
    bytes[*offset + 32] = name.len() as u8;
    bytes[*offset + 33..*offset + 33 + name.len()].copy_from_slice(name);
    *offset = record_end;
    true
}

unsafe extern "C" fn iso_read(sector: u32, count: u32, buffer: *mut core::ffi::c_void) -> bool {
    use core::sync::atomic::Ordering;
    let case = ISO_CASE.load(Ordering::Relaxed);
    if count != 1 || buffer.is_null() || (case == 1 && sector == 16) || (case == 2 && sector == 20)
    {
        return false;
    }
    // SAFETY: the disc interface requests one writable 2048-byte sector.
    let bytes = unsafe { core::slice::from_raw_parts_mut(buffer.cast::<u8>(), 2048) };
    bytes.fill(0);
    match sector {
        16 => {
            bytes[0] = 1;
            bytes[1..6].copy_from_slice(b"CD001");
            bytes[6] = 1;
            bytes[40..47].copy_from_slice(b"TESTVOL");
            if case == 2 || case == 3 {
                bytes[136..140]
                    .copy_from_slice(&(if case == 3 { 7u32 } else { 8u32 }).to_be_bytes());
                bytes[148..152].copy_from_slice(&20u32.to_be_bytes());
            } else if case == 4 {
                bytes[136..140].copy_from_slice(&10u32.to_be_bytes());
                bytes[148..152].copy_from_slice(&20u32.to_be_bytes());
                let mut root_record = 156;
                if !write_iso_directory_record(bytes, &mut root_record, 21, 2048, 2, &[0]) {
                    return false;
                }
            }
        }
        17 => {
            bytes[0] = 0xff;
            bytes[1..6].copy_from_slice(b"CD001");
            bytes[6] = 1;
        }
        20 if case == 4 => {
            bytes[0] = 1;
            bytes[2..6].copy_from_slice(&21u32.to_le_bytes());
            bytes[6..8].copy_from_slice(&1u16.to_be_bytes());
        }
        20 => {}
        21 if case == 4 => {
            let mut offset = 0;
            let root_records = write_iso_directory_record(bytes, &mut offset, 21, 2048, 2, &[0])
                && write_iso_directory_record(bytes, &mut offset, 21, 2048, 2, &[1])
                && write_iso_directory_record(bytes, &mut offset, 22, 5, 0, b"HELLO.TXT;1");
            if !root_records {
                return false;
            }
        }
        22 if case == 4 => bytes[..5].copy_from_slice(b"hello"),
        _ => return false,
    }
    true
}

fn probe_iso9660(logger: &mut ProbeLogger) {
    use core::sync::atomic::Ordering;
    logger.section("ISO9660 PROBE");
    let disc = filesystem::DiscInterface {
        io_type: 0,
        features: 0,
        startup: Some(iso_ready),
        is_inserted: Some(iso_ready),
        read_sectors: Some(iso_read),
        write_sectors: None,
        clear_status: None,
        shutdown: None,
    };
    let name = c"iso_test";
    let null_arguments_rejected = unsafe {
        !iso9660::ISO9660_Mount(core::ptr::null(), &disc)
            && !iso9660::ISO9660_Mount(name.as_ptr(), core::ptr::null())
            && !iso9660::ISO9660_Unmount(core::ptr::null())
            && iso9660::ISO9660_GetVolumeLabel(core::ptr::null()).is_null()
            && !iso9660::iso9660_mount(core::ptr::null(), &disc)
            && !iso9660::iso9660_mount(name.as_ptr(), core::ptr::null())
            && !iso9660::iso9660_unmount(core::ptr::null())
            && iso9660::iso9660_get_volume_label(core::ptr::null()).is_null()
    };
    logger.record(
        "ISO9660 C exports reject null arguments",
        null_arguments_rejected,
    );
    for (case, label) in [
        (1, "iso descriptor read failure"),
        (2, "iso path table read failure"),
        (3, "iso truncated path table"),
    ] {
        ISO_CASE.store(case, Ordering::Relaxed);
        let rejected = unsafe { !iso9660::ISO9660_Mount(name.as_ptr(), &disc) };
        let absent = unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()).is_null() };
        let cleanup = if rejected {
            true
        } else {
            unsafe { iso9660::ISO9660_Unmount(name.as_ptr()) }
        };
        logger.record(label, rejected && absent && cleanup);
    }
    ISO_CASE.store(4, Ordering::Relaxed);
    let rust_c_mounted = unsafe { iso9660::iso9660_mount(name.as_ptr(), &disc) };
    let rust_c_label = unsafe { iso9660::iso9660_get_volume_label(name.as_ptr()) };
    let rust_c_label_matches = !rust_c_label.is_null()
        && unsafe { core::ffi::CStr::from_ptr(rust_c_label).to_bytes() } == b"TESTVOL";
    let rust_c_unmounted = unsafe { iso9660::iso9660_unmount(name.as_ptr()) };
    let rust_c_label_cleared =
        unsafe { iso9660::iso9660_get_volume_label(name.as_ptr()) }.is_null();
    logger.record(
        "ISO9660 Rust C wrappers mount read the label and unmount the volume",
        rust_c_mounted && rust_c_label_matches && rust_c_unmounted && rust_c_label_cleared,
    );
    let mounted = unsafe { iso9660::ISO9660_Mount(name.as_ptr(), &disc) };
    let label = unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()) };
    let label_ok =
        !label.is_null() && unsafe { core::ffi::CStr::from_ptr(label).to_bytes() } == b"TESTVOL";
    if mounted {
        probe_iso9660_files(logger, name);
    }
    let removed = mounted && unsafe { iso9660::ISO9660_Unmount(name.as_ptr()) };
    logger.record(
        "iso mount after failures",
        mounted
            && label_ok
            && removed
            && unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()).is_null() },
    );

    let rust_name = c"isfs";
    let rust_mounted = filesystem::mount(rust_name, &disc);
    let rust_label = filesystem::volume_label(rust_name);
    let rust_label_ok = !rust_label.is_null()
        && unsafe { core::ffi::CStr::from_ptr(rust_label).to_bytes() } == b"TESTVOL";
    if rust_mounted {
        probe_iso9660_files(logger, rust_name);
    }
    let rust_removed = filesystem::unmount(rust_name);
    let rust_label_cleared = filesystem::volume_label(rust_name).is_null();
    logger.note_i32("Rust filesystem ISO mount", rust_mounted as i32);
    logger.note_i32("Rust filesystem ISO volume label", rust_label_ok as i32);
    logger.note_i32("Rust filesystem ISO unmount", rust_removed as i32);
    logger.note_i32(
        "Rust filesystem ISO label cleared",
        rust_label_cleared as i32,
    );
    logger.record("Rust filesystem ISO mount", rust_mounted);
    logger.record("Rust filesystem ISO volume label", rust_label_ok);
    logger.record("Rust filesystem ISO unmount", rust_removed);
    logger.record("Rust filesystem ISO cleanup", rust_label_cleared);
    probe_isfs_uninitialized(logger);
    probe_isfs_successful_file_cycle(logger);
}

fn isfs_file_contents_after_reopen(path: &core::ffi::CStr, expected: &[u8]) -> bool {
    if expected.len() > 32 {
        return false;
    }
    let Ok(file) = filesystem::open(path, filesystem::ISFS_OPEN_READ) else {
        return false;
    };
    let mut bytes = AlignedIsfsBytes([0; 32]);
    let read = file.read(&mut bytes.0[..expected.len()]);
    let close = file.close();
    read == Ok(expected.len())
        && &bytes.0[..expected.len()] == expected
        && close == filesystem::ISFS_OK
}

fn probe_isfs_successful_file_cycle(logger: &mut ProbeLogger) {
    logger.section("ISFS SUCCESSFUL FILE CYCLE");
    let directory = c"/tmp/ogc-rs-isfs-verify";
    let file_path = c"/tmp/ogc-rs-isfs-verify/file";
    let renamed_path = c"/tmp/ogc-rs-isfs-verify/renamed";
    let initialized = filesystem::initialize() == filesystem::ISFS_OK;
    let mut service_stats = filesystem::IsfsStats {
        a: u32::MAX,
        b: u32::MAX,
        c: u32::MAX,
        d: u32::MAX,
        e: u32::MAX,
        f: u32::MAX,
        g: u32::MAX,
    };
    let service_stats_result = if initialized {
        filesystem::get_stats(&mut service_stats)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.record(
        "ISFS get volume stats",
        service_stats_result == filesystem::ISFS_OK
            && service_stats
                != filesystem::IsfsStats {
                    a: u32::MAX,
                    b: u32::MAX,
                    c: u32::MAX,
                    d: u32::MAX,
                    e: u32::MAX,
                    f: u32::MAX,
                    g: u32::MAX,
                },
    );
    let directory_created =
        initialized && filesystem::create_dir(directory, 0, 3, 3, 3) == filesystem::ISFS_OK;
    logger.record("ISFS create test directory", directory_created);

    let file_created =
        directory_created && filesystem::create_file(file_path, 0, 3, 3, 3) == filesystem::ISFS_OK;
    logger.record("ISFS create test file", file_created);

    let mut aligned_write = AlignedIsfsBytes([0; 32]);
    aligned_write.0[..8].copy_from_slice(b"ISFS-rs!");
    let file = if file_created {
        filesystem::open(file_path, filesystem::ISFS_OPEN_RW).ok()
    } else {
        None
    };
    let write_result = file.as_ref().map_or(Err(filesystem::ISFS_EINVAL), |file| {
        file.write_all(&aligned_write.0[..8])
    });
    let seek_result = file
        .as_ref()
        .map_or(Err(filesystem::ISFS_EINVAL), |file| file.seek(0, 0));
    let mut aligned_read = AlignedIsfsBytes([0; 32]);
    let read_result = file.as_ref().map_or(Err(filesystem::ISFS_EINVAL), |file| {
        file.read(&mut aligned_read.0[..8])
    });
    let mut stats = AlignedFstats(filesystem::Fstats::default());
    let stats_result = file.as_ref().map_or(filesystem::ISFS_EINVAL, |file| {
        filesystem::get_file_stats(file, &mut stats.0)
    });
    let file_closed = file.is_none_or(|file| file.close() == filesystem::ISFS_OK);
    logger.record(
        "ISFS file write seek read stats and close",
        write_result == Ok(())
            && seek_result == Ok(0)
            && read_result == Ok(8)
            && aligned_read.0[..8] == *b"ISFS-rs!"
            && stats_result == filesystem::ISFS_OK
            && stats.0
                == filesystem::Fstats {
                    file_length: 8,
                    file_pos: 8,
                }
            && file_closed,
    );
    logger.record(
        "ISFS sync file data persists after close and reopen",
        file_created && file_closed && isfs_file_contents_after_reopen(file_path, b"ISFS-rs!"),
    );

    let mut owner_id = u32::MAX;
    let mut group_id = u16::MAX;
    let mut attributes = u8::MAX;
    let mut owner_perm = u8::MAX;
    let mut group_perm = u8::MAX;
    let mut other_perm = u8::MAX;
    let get_attr_result = if file_created {
        filesystem::get_attr(
            file_path,
            &mut owner_id,
            &mut group_id,
            &mut attributes,
            &mut owner_perm,
            &mut group_perm,
            &mut other_perm,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let set_attr_result = if get_attr_result == filesystem::ISFS_OK {
        filesystem::set_attr(file_path, owner_id, group_id, attributes, 3, 1, 1)
    } else {
        filesystem::ISFS_EINVAL
    };
    let mut changed_owner_id = u32::MAX;
    let mut changed_group_id = u16::MAX;
    let mut changed_attributes = u8::MAX;
    let mut changed_owner_perm = u8::MAX;
    let mut changed_group_perm = u8::MAX;
    let mut changed_other_perm = u8::MAX;
    let changed_attr_result = if set_attr_result == filesystem::ISFS_OK {
        filesystem::get_attr(
            file_path,
            &mut changed_owner_id,
            &mut changed_group_id,
            &mut changed_attributes,
            &mut changed_owner_perm,
            &mut changed_group_perm,
            &mut changed_other_perm,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let mut usage_blocks = u32::MAX;
    let mut usage_inodes = u32::MAX;
    let usage_result = if directory_created {
        filesystem::get_usage(directory, &mut usage_blocks, &mut usage_inodes)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.record(
        "ISFS get set get attributes and usage",
        get_attr_result == filesystem::ISFS_OK
            && set_attr_result == filesystem::ISFS_OK
            && changed_attr_result == filesystem::ISFS_OK
            && changed_owner_id == owner_id
            && changed_group_id == group_id
            && changed_attributes == attributes
            && changed_owner_perm == 3
            && changed_group_perm == 1
            && changed_other_perm == 1
            && usage_result == filesystem::ISFS_OK
            && usage_blocks != u32::MAX
            && usage_inodes != u32::MAX,
    );

    let mut directory_count = 4;
    let mut directory_names = AlignedIsfsBytes([0; 32 * 4]);
    let read_dir_result = if directory_created {
        filesystem::read_dir(
            directory,
            Some(&mut directory_names.0),
            &mut directory_count,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let entry_present = directory_names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"file");
    logger.note_i32("ISFS successful readdir result", read_dir_result);
    logger.note_i32("ISFS successful readdir count", directory_count as i32);
    logger.note_i32("ISFS successful readdir file entry", entry_present as i32);
    logger.record(
        "ISFS read directory lists test file",
        read_dir_result == filesystem::ISFS_OK && directory_count > 0 && entry_present,
    );
    probe_isfs_async_file_cycle(directory, logger);
    probe_isfs_async_directory_cycle(directory, logger);
    probe_isfs_directory_rename_cycles(logger);
    probe_isfs_async_rename_cycle(logger);

    let rename_result = if file_created {
        filesystem::rename(file_path, renamed_path)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.note_i32("ISFS successful rename result", rename_result);
    logger.record(
        "ISFS sync rename result matches file state",
        file_created && isfs_rename_result_matches_paths(file_path, renamed_path, rename_result),
    );
    let renamed_open = if rename_result == filesystem::ISFS_OK {
        filesystem::open(renamed_path, filesystem::ISFS_OPEN_READ).ok()
    } else {
        None
    };
    let renamed_file_exists = renamed_open.is_some();
    let renamed_file_closed = renamed_open.is_none_or(|file| file.close() == filesystem::ISFS_OK);
    if rename_result == filesystem::ISFS_OK {
        logger.record(
            "ISFS open renamed file and close",
            renamed_file_exists && renamed_file_closed,
        );
    } else {
        logger.note("ISFS renamed-file reopen skipped because Dolphin rejected rename");
    }

    let rename_paths_removed = cleanup_isfs_rename_paths(file_path, renamed_path);
    let directory_delete_result = if directory_created {
        filesystem::delete(directory)
    } else {
        filesystem::ISFS_OK
    };
    let source_after_cleanup = filesystem::open(file_path, filesystem::ISFS_OPEN_READ);
    let destination_after_cleanup = filesystem::open(renamed_path, filesystem::ISFS_OPEN_READ);
    let mut deleted_directory_usage_a = u32::MAX;
    let mut deleted_directory_usage_b = u32::MAX;
    let deleted_directory_usage =
        if directory_delete_result == filesystem::ISFS_OK && directory_created {
            filesystem::get_usage(
                directory,
                &mut deleted_directory_usage_a,
                &mut deleted_directory_usage_b,
            )
        } else {
            filesystem::ISFS_EINVAL
        };
    logger.note_i32(
        "ISFS usage after directory delete result",
        deleted_directory_usage,
    );
    logger.record(
        "ISFS delete test file and directory",
        file_created && rename_paths_removed && directory_delete_result == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS file and directory are absent after delete",
        source_after_cleanup.is_err_and(|error| error == filesystem::ISFS_ENOENT)
            && destination_after_cleanup.is_err_and(|error| error == filesystem::ISFS_ENOENT)
            && deleted_directory_usage == filesystem::ISFS_ENOENT
            && deleted_directory_usage_a == u32::MAX
            && deleted_directory_usage_b == u32::MAX,
    );
    probe_isfs_format_apis(logger);
}

fn probe_isfs_format_apis(logger: &mut ProbeLogger) {
    logger.section("ISFS FORMAT API PROBE");
    let directory = c"/tmp/ogc-rs-format-verify";
    let marker = c"/tmp/ogc-rs-format-verify/marker";

    let initialized = filesystem::initialize() == filesystem::ISFS_OK;
    let directory_created =
        initialized && filesystem::create_dir(directory, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let marker_created =
        directory_created && filesystem::create_file(marker, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let (submission, completed, result) = if marker_created {
        run_isfs_async_and_wait_bounded(|| unsafe {
            filesystem::format_async(Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, false, i32::MIN)
    };
    logger.note_i32("ISFS format_async submission result", submission);
    logger.note_i32("ISFS format_async callback result", result);
    let deinitialize = filesystem::deinitialize();
    let reinitialize = filesystem::initialize();
    let file_exists = isfs_file_exists(marker);
    let directory_exists = isfs_directory_exists(directory);
    logger.record(
        "ISFS format_async removes the marker file",
        file_exists == Some(false),
    );
    logger.record(
        "ISFS format_async removes the marker directory",
        directory_exists == Some(false),
    );
    logger.record(
        "ISFS format_async completes and removes marker state",
        marker_created
            && submission >= 0
            && completed
            && result == filesystem::ISFS_OK
            && deinitialize == filesystem::ISFS_OK
            && reinitialize == filesystem::ISFS_OK
            && file_exists == Some(false)
            && directory_exists == Some(false),
    );
    let cleanup = cleanup_isfs_test_tree(directory, marker);
    logger.record("ISFS format_async marker cleanup", cleanup);
    probe_isfs_format_sync(logger);
}

fn probe_isfs_format_sync(logger: &mut ProbeLogger) {
    logger.section("ISFS SYNCHRONOUS FORMAT PROBE");
    let temp_root = c"/tmp";
    let directory = c"/tmp/ogc-rs-format-sync-verify";
    let marker = c"/tmp/ogc-rs-format-sync-verify/marker";

    let initialize_result = filesystem::initialize();
    let temp_root_exists = if initialize_result == filesystem::ISFS_OK {
        isfs_directory_exists(temp_root)
    } else {
        None
    };
    let temp_root_ready = match temp_root_exists {
        Some(true) => true,
        Some(false) => filesystem::create_dir(temp_root, 0, 3, 3, 3) == filesystem::ISFS_OK,
        None => false,
    };
    let stale_state_removed = initialize_result == filesystem::ISFS_OK
        && temp_root_ready
        && cleanup_isfs_test_tree(directory, marker);
    let directory_result = if stale_state_removed {
        filesystem::create_dir(directory, 0, 3, 3, 3)
    } else {
        filesystem::ISFS_EINVAL
    };
    let directory_created = directory_result == filesystem::ISFS_OK;
    let marker_result = if directory_created {
        filesystem::create_file(marker, 0, 3, 3, 3)
    } else {
        filesystem::ISFS_EINVAL
    };
    let marker_created = marker_result == filesystem::ISFS_OK;
    let format_result = if marker_created {
        filesystem::format()
    } else {
        filesystem::ISFS_EINVAL
    };
    let deinitialize = filesystem::deinitialize();
    let reinitialize = filesystem::initialize();
    let file_exists = isfs_file_exists(marker);
    let directory_exists = isfs_directory_exists(directory);
    logger.note_i32(
        "ISFS synchronous format initialize result",
        initialize_result,
    );
    logger.record(
        "ISFS synchronous format prepares the temporary root",
        temp_root_ready,
    );
    logger.note_i32(
        "ISFS synchronous format marker directory result",
        directory_result,
    );
    logger.note_i32("ISFS synchronous format marker file result", marker_result);
    logger.note_i32("ISFS synchronous format result", format_result);
    logger.note_i32("ISFS synchronous format deinitialize result", deinitialize);
    logger.note_i32("ISFS synchronous format reinitialize result", reinitialize);
    logger.record(
        "ISFS synchronous format creates isolated marker state",
        initialize_result == filesystem::ISFS_OK
            && stale_state_removed
            && directory_created
            && marker_created,
    );
    logger.record(
        "ISFS synchronous format clears marker state after service restart",
        format_result == filesystem::ISFS_OK
            && deinitialize == filesystem::ISFS_OK
            && reinitialize == filesystem::ISFS_OK
            && file_exists == Some(false)
            && directory_exists == Some(false),
    );
    let cleanup = reinitialize == filesystem::ISFS_OK && cleanup_isfs_test_tree(directory, marker);
    logger.record("ISFS synchronous format cleanup succeeds", cleanup);
}

fn probe_isfs_async_file_cycle(directory: &core::ffi::CStr, logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC FILE CYCLE");
    let path = c"/tmp/ogc-rs-isfs-verify/async-file";
    let (create_submit, create_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::create_file_async(
            path,
            0,
            3,
            3,
            3,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    logger.record(
        "ISFS create_file_async completes successfully",
        create_submit >= 0 && create_result == filesystem::ISFS_OK,
    );

    let (open_submit, open_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::open_async(
                path,
                filesystem::ISFS_OPEN_RW,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut file =
        (open_submit >= 0 && open_result >= 0).then(|| filesystem::IsfsFile::from_raw(open_result));
    logger.record("ISFS open_async returns an open descriptor", file.is_some());

    let mut write_buffer = AlignedIsfsBytes([0; 32]);
    write_buffer.0[..8].copy_from_slice(b"async-rs");
    let (write_submit, write_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.write_async(
                &write_buffer.0[..8],
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let (seek_submit, seek_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.seek_async(0, 0, Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, i32::MIN)
    };
    let mut read_buffer = AlignedIsfsBytes([0; 32]);
    let (read_submit, read_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.read_async(
                &mut read_buffer.0[..8],
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut stats = AlignedFstats(filesystem::Fstats::default());
    let (stats_submit, stats_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.get_stats_async(
                &mut stats.0,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.record(
        "ISFS async write seek read and file stats",
        write_submit >= 0
            && write_result == 8
            && seek_submit >= 0
            && seek_result == 0
            && read_submit >= 0
            && read_result == 8
            && read_buffer.0[..8] == *b"async-rs"
            && stats_submit >= 0
            && stats_result == filesystem::ISFS_OK
            && stats.0
                == filesystem::Fstats {
                    file_length: 8,
                    file_pos: 8,
                },
    );

    let mut owner_id = u32::MAX;
    let mut group_id = u16::MAX;
    let mut attributes = u8::MAX;
    let mut owner_perm = u8::MAX;
    let mut group_perm = u8::MAX;
    let mut other_perm = u8::MAX;
    let (get_attr_submit, get_attr_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_attr_async(
            path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner_id,
                group_id: &mut group_id,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let (set_attr_submit, set_attr_result) = if get_attr_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::set_attr_async(
                path,
                filesystem::FsAttributes {
                    owner_id,
                    group_id,
                    attributes,
                    owner_perm: 3,
                    group_perm: 1,
                    other_perm: 1,
                },
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut changed_owner_id = u32::MAX;
    let mut changed_group_id = u16::MAX;
    let mut changed_attributes = u8::MAX;
    let mut changed_owner_perm = u8::MAX;
    let mut changed_group_perm = u8::MAX;
    let mut changed_other_perm = u8::MAX;
    let (changed_attr_submit, changed_attr_result) = if set_attr_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_attr_async(
                path,
                filesystem::FsAttributeOutputs {
                    owner_id: &mut changed_owner_id,
                    group_id: &mut changed_group_id,
                    attributes: &mut changed_attributes,
                    owner_perm: &mut changed_owner_perm,
                    group_perm: &mut changed_group_perm,
                    other_perm: &mut changed_other_perm,
                },
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };

    let mut usage_a = u32::MAX;
    let mut usage_b = u32::MAX;
    let (usage_submit, usage_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_usage_async(
            directory,
            &mut usage_a,
            &mut usage_b,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let mut num = 4;
    let mut names = AlignedIsfsBytes([0; 32 * 4]);
    let (readdir_submit, readdir_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::read_dir_async(
            directory,
            Some(&mut names.0),
            &mut num,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let async_entry_present = names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"async-file");
    logger.record(
        "ISFS async attributes usage and directory listing",
        get_attr_submit >= 0
            && get_attr_result == filesystem::ISFS_OK
            && set_attr_submit >= 0
            && set_attr_result == filesystem::ISFS_OK
            && changed_attr_submit >= 0
            && changed_attr_result == filesystem::ISFS_OK
            && changed_owner_id == owner_id
            && changed_group_id == group_id
            && changed_attributes == attributes
            && changed_owner_perm == 3
            && changed_group_perm == 1
            && changed_other_perm == 1
            && usage_submit >= 0
            && usage_result == filesystem::ISFS_OK
            && usage_a != u32::MAX
            && usage_b != u32::MAX
            && readdir_submit >= 0
            && readdir_result == filesystem::ISFS_OK
            && num > 0
            && async_entry_present,
    );

    let mut closed = false;
    if let Some(file) = file.take() {
        let (close_submit, close_result) = run_isfs_async_and_wait(|| unsafe {
            file.close_async(Some(isfs_probe_callback), core::ptr::null_mut())
        });
        closed = close_submit >= 0 && close_result == filesystem::ISFS_OK;
    }
    logger.record(
        "ISFS async file data persists after close and reopen",
        closed && isfs_file_contents_after_reopen(path, b"async-rs"),
    );
    let (delete_submit, delete_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::delete_async(path, Some(isfs_probe_callback), core::ptr::null_mut())
    });
    let deleted_file_open = filesystem::open(path, filesystem::ISFS_OPEN_READ);
    logger.record(
        "ISFS async close delete and cleanup",
        closed
            && delete_submit >= 0
            && delete_result == filesystem::ISFS_OK
            && deleted_file_open.is_err_and(|error| error == filesystem::ISFS_ENOENT),
    );
}

fn probe_isfs_async_directory_cycle(directory: &core::ffi::CStr, logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC DIRECTORY CYCLE");
    let path = c"/tmp/ogc-rs-isfs-verify/async-dir";
    let mut stats = filesystem::IsfsStats {
        a: u32::MAX,
        b: u32::MAX,
        c: u32::MAX,
        d: u32::MAX,
        e: u32::MAX,
        f: u32::MAX,
        g: u32::MAX,
    };
    let (stats_submit, stats_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_stats_async(&mut stats, Some(isfs_probe_callback), core::ptr::null_mut())
    });
    logger.record(
        "ISFS get_stats_async returns service stats",
        stats_submit >= 0
            && stats_result == filesystem::ISFS_OK
            && stats
                != filesystem::IsfsStats {
                    a: u32::MAX,
                    b: u32::MAX,
                    c: u32::MAX,
                    d: u32::MAX,
                    e: u32::MAX,
                    f: u32::MAX,
                    g: u32::MAX,
                },
    );

    let (create_submit, create_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::create_dir_async(
            path,
            0,
            3,
            3,
            3,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let mut usage_a = u32::MAX;
    let mut usage_b = u32::MAX;
    let (usage_submit, usage_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_usage_async(
                path,
                &mut usage_a,
                &mut usage_b,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };

    let mut count = 8;
    let mut names = AlignedIsfsBytes([0; 32 * 8]);
    let (read_dir_submit, read_dir_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::read_dir_async(
            directory,
            Some(&mut names.0),
            &mut count,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let directory_entry_present = names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"async-dir");
    logger.record(
        "ISFS create_dir_async usage and read_dir_async",
        create_submit >= 0
            && create_result == filesystem::ISFS_OK
            && usage_submit >= 0
            && usage_result == filesystem::ISFS_OK
            && usage_a != u32::MAX
            && usage_b != u32::MAX
            && read_dir_submit >= 0
            && read_dir_result == filesystem::ISFS_OK
            && count > 0
            && directory_entry_present,
    );

    let (delete_submit, delete_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::delete_async(path, Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, i32::MIN)
    };
    let mut absent_usage_a = u32::MAX;
    let mut absent_usage_b = u32::MAX;
    let (absent_usage_submit, absent_usage_result) = if delete_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_usage_async(
                path,
                &mut absent_usage_a,
                &mut absent_usage_b,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.record(
        "ISFS delete_async removes directory and preserves error outputs",
        delete_submit >= 0
            && delete_result == filesystem::ISFS_OK
            && absent_usage_submit >= 0
            && absent_usage_result == filesystem::ISFS_ENOENT
            && absent_usage_a == u32::MAX
            && absent_usage_b == u32::MAX,
    );
}

fn probe_isfs_async_rename_cycle(logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC RENAME CYCLE");
    let source = c"/tmp/ogc-rs-isfs-verify/async-rename-source";
    let destination = c"/tmp/ogc-rs-isfs-verify/async-rename-destination";
    let source_created = filesystem::create_file(source, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let (rename_submit, rename_result) = if source_created {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::rename_async(
                source,
                destination,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.note_i32("ISFS rename_async callback result", rename_result);
    logger.record(
        "ISFS rename_async callback matches file state",
        source_created
            && rename_submit >= 0
            && isfs_rename_result_matches_paths(source, destination, rename_result),
    );
    logger.record(
        "ISFS async rename test files are cleaned up",
        cleanup_isfs_rename_paths(source, destination),
    );
}

fn probe_isfs_directory_rename_cycles(logger: &mut ProbeLogger) {
    logger.section("ISFS DIRECTORY RENAME CYCLES");
    let source = c"/tmp/ogc-rs-isfs-verify/rename-dir-source";
    let destination = c"/tmp/ogc-rs-isfs-verify/rename-dir-destination";
    let stale_cleanup = cleanup_isfs_directory_paths(source, destination);
    let create_result = if stale_cleanup {
        filesystem::create_dir(source, 0, 3, 3, 3)
    } else {
        filesystem::ISFS_EINVAL
    };
    let rename_result = if create_result == filesystem::ISFS_OK {
        filesystem::rename(source, destination)
    } else {
        filesystem::ISFS_EINVAL
    };
    let source_after_sync = isfs_directory_exists(source);
    let destination_after_sync = isfs_directory_exists(destination);
    logger.note_i32("ISFS sync directory rename result", rename_result);
    logger.record(
        "ISFS sync rename moves an empty directory",
        stale_cleanup
            && create_result == filesystem::ISFS_OK
            && rename_result == filesystem::ISFS_OK
            && source_after_sync == Some(false)
            && destination_after_sync == Some(true),
    );
    let sync_cleanup = cleanup_isfs_directory_paths(source, destination);
    logger.record(
        "ISFS sync directory rename cleanup removes both paths",
        sync_cleanup,
    );

    let async_source = c"/tmp/ogc-rs-isfs-verify/async-rename-dir-source";
    let async_destination = c"/tmp/ogc-rs-isfs-verify/async-rename-dir-destination";
    let async_stale_cleanup = cleanup_isfs_directory_paths(async_source, async_destination);
    let async_create_result = if async_stale_cleanup {
        filesystem::create_dir(async_source, 0, 3, 3, 3)
    } else {
        filesystem::ISFS_EINVAL
    };
    let (rename_submit, rename_result) = if async_create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::rename_async(
                async_source,
                async_destination,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let async_source_after = isfs_directory_exists(async_source);
    let async_destination_after = isfs_directory_exists(async_destination);
    logger.note_i32("ISFS async directory rename submit result", rename_submit);
    logger.note_i32("ISFS async directory rename callback result", rename_result);
    logger.record(
        "ISFS async rename completes and moves an empty directory",
        async_stale_cleanup
            && async_create_result == filesystem::ISFS_OK
            && rename_submit >= 0
            && rename_result == filesystem::ISFS_OK
            && async_source_after == Some(false)
            && async_destination_after == Some(true),
    );
    let async_cleanup = cleanup_isfs_directory_paths(async_source, async_destination);
    logger.record(
        "ISFS async directory rename cleanup removes both paths",
        async_cleanup,
    );
}

fn cleanup_isfs_directory_paths(source: &core::ffi::CStr, destination: &core::ffi::CStr) -> bool {
    let remove_path = |path: &core::ffi::CStr| match isfs_directory_exists(path) {
        Some(true) => filesystem::delete(path) == filesystem::ISFS_OK,
        Some(false) => true,
        None => false,
    };
    let source_removed = remove_path(source);
    let destination_removed = remove_path(destination);
    source_removed
        && destination_removed
        && isfs_directory_exists(source) == Some(false)
        && isfs_directory_exists(destination) == Some(false)
}

fn isfs_rename_result_matches_paths(
    source: &core::ffi::CStr,
    destination: &core::ffi::CStr,
    result: i32,
) -> bool {
    let source_open = filesystem::open(source, filesystem::ISFS_OPEN_READ);
    let destination_open = filesystem::open(destination, filesystem::ISFS_OPEN_READ);
    let source_exists = source_open.is_ok();
    let source_absent = source_open
        .as_ref()
        .is_err_and(|error| *error == filesystem::ISFS_ENOENT);
    let destination_exists = destination_open.is_ok();
    let destination_absent = destination_open
        .as_ref()
        .is_err_and(|error| *error == filesystem::ISFS_ENOENT);
    let source_closed = match source_open {
        Ok(file) => file.close() == filesystem::ISFS_OK,
        Err(error) => error == filesystem::ISFS_ENOENT,
    };
    let destination_closed = match destination_open {
        Ok(file) => file.close() == filesystem::ISFS_OK,
        Err(error) => error == filesystem::ISFS_ENOENT,
    };

    match result {
        filesystem::ISFS_OK => {
            source_absent && destination_exists && source_closed && destination_closed
        }
        filesystem::ISFS_EINVAL => {
            source_exists && destination_absent && source_closed && destination_closed
        }
        _ => false,
    }
}

fn cleanup_isfs_rename_paths(source: &core::ffi::CStr, destination: &core::ffi::CStr) -> bool {
    let remove_path =
        |path: &core::ffi::CStr| match filesystem::open(path, filesystem::ISFS_OPEN_READ) {
            Ok(file) => {
                let close_result = file.close();
                let delete_result = filesystem::delete(path);
                close_result == filesystem::ISFS_OK && delete_result == filesystem::ISFS_OK
            }
            Err(error) => error == filesystem::ISFS_ENOENT,
        };
    let source_removed = remove_path(source);
    let destination_removed = remove_path(destination);
    source_removed
        && destination_removed
        && filesystem::open(source, filesystem::ISFS_OPEN_READ)
            .is_err_and(|error| error == filesystem::ISFS_ENOENT)
        && filesystem::open(destination, filesystem::ISFS_OPEN_READ)
            .is_err_and(|error| error == filesystem::ISFS_ENOENT)
}

fn probe_iso9660_files(logger: &mut ProbeLogger, mount_name: &core::ffi::CStr) {
    let file_path = if mount_name.to_bytes() == b"iso_test" {
        c"iso_test:/HELLO.TXT"
    } else {
        c"isfs:/HELLO.TXT"
    };
    let device_found = sysbase::device_op_tab(file_path).is_some();
    let (file, open_error) = match sysbase::DeviceFile::open_checked(file_path, sysbase::O_RDONLY) {
        Ok(file) => (Some(file), 0),
        Err(error) => (None, error),
    };
    logger.note_i32("ISO9660 mounted device lookup", device_found as i32);
    logger.note_i32("ISO9660 file open error", open_error);
    let file_opened = file.is_some();
    let mut bytes = [0xA5; 8];
    let file_read = file
        .as_ref()
        .and_then(|file| file.read(&mut bytes).ok())
        .is_some_and(|count| count == 5 && bytes[..5] == *b"hello");
    let file_seek = file
        .as_ref()
        .and_then(|file| file.seek_off_t(1, sysbase::SEEK_SET).ok())
        == Some(1);
    let mut suffix = [0; 4];
    let file_seek_read = file
        .as_ref()
        .and_then(|file| file.read(&mut suffix).ok())
        .is_some_and(|count| count == 4 && suffix == *b"ello");
    let mut eof_bytes = [0xa5; 4];
    let eof_preserved = file
        .as_ref()
        .is_some_and(|file| file.read(&mut eof_bytes) == Ok(0) && eof_bytes == [0xa5; 4]);
    let write_rejected = file
        .as_ref()
        .is_some_and(|file| file.write_all(b"x") == Err(-sysbase::ENOTSUP));
    let file_closed = file.is_some_and(|file| file.close().is_ok());
    let missing_path = if mount_name.to_bytes() == b"iso_test" {
        c"iso_test:/MISSING.TXT"
    } else {
        c"isfs:/MISSING.TXT"
    };
    let missing_rejected = matches!(
        sysbase::DeviceFile::open_checked(missing_path, sysbase::O_RDONLY),
        Err(error) if error == -sysbase::ENOENT
    );

    let stat_fn = sysbase::device_op_tab(file_path).and_then(|tab| unsafe { (*tab).stat_r });
    let mut stat = sysbase::Stat::default();
    let stat_result = stat_fn.map_or(-1, |callback| unsafe {
        callback(
            sysbase::current_reent(),
            file_path.as_ptr(),
            (&mut stat as *mut sysbase::Stat).cast(),
        )
    });
    logger.note_i32("ISO9660 stat callback result", stat_result);
    let stat_ok = stat_result == 0
        && stat.st_ino == 22
        && stat.st_size == 5
        && stat.st_blocks == 1
        && stat.st_mode & 0o170000 == 0o100000;

    let root_path = if mount_name.to_bytes() == b"iso_test" {
        c"iso_test:/"
    } else {
        c"isfs:/"
    };
    let chdir_fn = sysbase::device_op_tab(root_path).and_then(|tab| unsafe { (*tab).chdir_r });
    let chdir_ok = chdir_fn.is_some_and(|callback| unsafe {
        callback(sysbase::current_reent(), root_path.as_ptr()) == 0
    });

    logger.record("ISO9660 mounted file opens", file_opened);
    logger.record("ISO9660 file read returns expected bytes", file_read);
    logger.record("ISO9660 file seek returns expected position", file_seek);
    logger.record(
        "ISO9660 read after seek returns expected bytes",
        file_seek_read,
    );
    logger.record("ISO9660 EOF preserves the output buffer", eof_preserved);
    logger.record("ISO9660 rejects writes to a mounted file", write_rejected);
    logger.record("ISO9660 missing file returns ENOENT", missing_rejected);
    logger.record("ISO9660 file close completes", file_closed);
    logger.record("ISO9660 stat reports file metadata", stat_ok);
    logger.record("ISO9660 chdir accepts the root directory", chdir_ok);
}

fn probe_isfs_uninitialized(logger: &mut ProbeLogger) {
    logger.section("ISFS INVALID-STATE API PROBE");

    // The probe logger can hold a file handle, but it does not use the shared
    // /dev/fs service descriptor. Close that descriptor to exercise safe
    // pre-initialization results without issuing an IOS write operation.
    let init_before_deinit = filesystem::initialize();
    let mut owner = 31;
    let mut group = 32;
    let mut attributes = 33;
    let mut owner_perm = 34;
    let mut group_perm = 35;
    let mut other_perm = 36;
    let mut long_path_bytes = [b'P'; 65];
    long_path_bytes[64] = 0;
    let long_path = core::ffi::CStr::from_bytes_with_nul(&long_path_bytes)
        .expect("fixed overlong ISFS path is NUL terminated");
    let mut long_path_count = 41;
    let long_read_dir = filesystem::read_dir(long_path, None, &mut long_path_count);
    let long_create_dir = filesystem::create_dir(long_path, 0, 0, 0, 0);
    let long_create_file = filesystem::create_file(long_path, 0, 0, 0, 0);
    let long_get_attr = filesystem::get_attr(
        long_path,
        &mut owner,
        &mut group,
        &mut attributes,
        &mut owner_perm,
        &mut group_perm,
        &mut other_perm,
    );
    let long_set_attr = filesystem::set_attr(long_path, 0, 0, 0, 0, 0, 0);
    let mut long_usage_a = 42;
    let mut long_usage_b = 43;
    let long_get_usage = filesystem::get_usage(long_path, &mut long_usage_a, &mut long_usage_b);
    let long_delete = filesystem::delete(long_path);
    let long_rename_old = filesystem::rename(long_path, c"/tmp/short");
    let long_rename_new = filesystem::rename(c"/tmp/short", long_path);
    let long_open = filesystem::open(long_path, filesystem::ISFS_OPEN_READ);

    reset_isfs_callback();
    // SAFETY: every call must reject the overlong path before IOS submission.
    let long_read_dir_async = unsafe {
        filesystem::read_dir_async(
            long_path,
            None,
            &mut long_path_count,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_create_dir_async = unsafe {
        filesystem::create_dir_async(
            long_path,
            0,
            0,
            0,
            0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_create_file_async = unsafe {
        filesystem::create_file_async(
            long_path,
            0,
            0,
            0,
            0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_get_attr_async = unsafe {
        filesystem::get_attr_async(
            long_path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner,
                group_id: &mut group,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_set_attr_async = unsafe {
        filesystem::set_attr_async(
            long_path,
            filesystem::FsAttributes::default(),
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_get_usage_async = unsafe {
        filesystem::get_usage_async(
            long_path,
            &mut long_usage_a,
            &mut long_usage_b,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_delete_async = unsafe {
        filesystem::delete_async(long_path, Some(isfs_probe_callback), core::ptr::null_mut())
    };
    let long_rename_old_async = unsafe {
        filesystem::rename_async(
            long_path,
            c"/tmp/short",
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_rename_new_async = unsafe {
        filesystem::rename_async(
            c"/tmp/short",
            long_path,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_open_async = unsafe {
        filesystem::open_async(
            long_path,
            filesystem::ISFS_OPEN_READ,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_path_async_callback_not_called = !ISFS_CALLBACK_DONE.load(Ordering::Acquire)
        && ISFS_CALLBACK_RESULT.load(Ordering::Acquire) == i32::MIN;
    logger.record(
        "ISFS sync path APIs reject overlong paths and preserve outputs",
        [
            long_read_dir,
            long_create_dir,
            long_create_file,
            long_get_attr,
            long_set_attr,
            long_get_usage,
            long_delete,
            long_rename_old,
            long_rename_new,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && matches!(long_open, Err(filesystem::ISFS_EINVAL))
            && long_path_count == 41
            && long_usage_a == 42
            && long_usage_b == 43
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36,
    );
    logger.record(
        "ISFS async path APIs reject overlong paths before callback",
        [
            long_read_dir_async,
            long_create_dir_async,
            long_create_file_async,
            long_get_attr_async,
            long_set_attr_async,
            long_get_usage_async,
            long_delete_async,
            long_rename_old_async,
            long_rename_new_async,
            long_open_async,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && long_path_async_callback_not_called
            && long_path_count == 41
            && long_usage_a == 42
            && long_usage_b == 43
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36,
    );
    let deinit = filesystem::deinitialize();
    let repeated_deinit = filesystem::deinitialize();

    let mut stats = filesystem::IsfsStats {
        a: 1,
        b: 2,
        c: 3,
        d: 4,
        e: 5,
        f: 6,
        g: 7,
    };
    let get_stats = filesystem::get_stats(&mut stats);
    reset_isfs_callback();
    // SAFETY: the initialized service is closed, so the call must return
    // before it can submit an IOS request or retain the output buffer.
    let get_stats_async = unsafe {
        filesystem::get_stats_async(&mut stats, Some(isfs_probe_callback), core::ptr::null_mut())
    };
    let get_stats_async_callback_not_called = !ISFS_CALLBACK_DONE.load(Ordering::Acquire)
        && ISFS_CALLBACK_RESULT.load(Ordering::Acquire) == i32::MIN;
    // SAFETY: the initialized service is closed, so this call returns before
    // it can submit the destructive format ioctl.
    let format_async = unsafe { filesystem::format_async(None, core::ptr::null_mut()) };
    let format = filesystem::format();

    let path = c"/tmp/libogc-fs-probe";
    let mut directory_count = 11;
    let read_dir = filesystem::read_dir(path, None, &mut directory_count);
    // SAFETY: the uninitialized service returns an error before submission.
    let read_dir_async = unsafe {
        filesystem::read_dir_async(
            path,
            None,
            &mut directory_count,
            None,
            core::ptr::null_mut(),
        )
    };
    let mut usage_a = 21;
    let mut usage_b = 22;
    let get_usage = filesystem::get_usage(path, &mut usage_a, &mut usage_b);
    // SAFETY: the uninitialized service returns an error before submission.
    let get_usage_async = unsafe {
        filesystem::get_usage_async(
            path,
            &mut usage_a,
            &mut usage_b,
            None,
            core::ptr::null_mut(),
        )
    };
    let create_dir = filesystem::create_dir(path, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let create_dir_async =
        unsafe { filesystem::create_dir_async(path, 0, 0, 0, 0, None, core::ptr::null_mut()) };
    let create_file = filesystem::create_file(path, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let create_file_async =
        unsafe { filesystem::create_file_async(path, 0, 0, 0, 0, None, core::ptr::null_mut()) };
    let delete = filesystem::delete(path);
    // SAFETY: the uninitialized service returns an error before submission.
    let delete_async = unsafe { filesystem::delete_async(path, None, core::ptr::null_mut()) };
    let rename = filesystem::rename(path, c"/tmp/libogc-fs-probe-renamed");
    // SAFETY: the uninitialized service returns an error before submission.
    let rename_async = unsafe {
        filesystem::rename_async(
            path,
            c"/tmp/libogc-fs-probe-renamed",
            None,
            core::ptr::null_mut(),
        )
    };

    let get_attr = filesystem::get_attr(
        path,
        &mut owner,
        &mut group,
        &mut attributes,
        &mut owner_perm,
        &mut group_perm,
        &mut other_perm,
    );
    // SAFETY: the uninitialized service returns an error before submission.
    let get_attr_async = unsafe {
        filesystem::get_attr_async(
            path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner,
                group_id: &mut group,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            None,
            core::ptr::null_mut(),
        )
    };
    let set_attr = filesystem::set_attr(path, 0, 0, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let set_attr_async = unsafe {
        filesystem::set_attr_async(
            path,
            filesystem::FsAttributes::default(),
            None,
            core::ptr::null_mut(),
        )
    };

    let open = filesystem::open(path, filesystem::ISFS_OPEN_READ);
    // SAFETY: the uninitialized service returns before it submits an IOS open.
    let open_async = unsafe {
        filesystem::open_async(
            path,
            filesystem::ISFS_OPEN_READ,
            None,
            core::ptr::null_mut(),
        )
    };
    let file = filesystem::IsfsFile::from_raw(-1);
    let raw = file.raw();
    let empty_write = file.write(&[]);
    let empty_write_all = file.write_all(&[]);
    let empty_read = file.read(&mut []);
    // SAFETY: these empty slices hit validation errors before IOS submission.
    let empty_read_async = unsafe { file.read_async(&mut [], None, core::ptr::null_mut()) };
    // SAFETY: the empty slice hits validation before IOS submission.
    let empty_write_async = unsafe { file.write_async(&[], None, core::ptr::null_mut()) };
    let seek = file.seek(0, 0);
    reset_isfs_callback();
    // SAFETY: the callback uses atomics and remains valid until IOS completes.
    let seek_async =
        unsafe { file.seek_async(0, 0, Some(isfs_probe_callback), core::ptr::null_mut()) };
    let seek_async_done = wait_isfs_callback();
    let seek_async_result = ISFS_CALLBACK_RESULT.load(Ordering::Acquire);
    let mut file_stats = AlignedFstats(filesystem::Fstats::default());
    let get_file_stats = filesystem::get_file_stats(&file, &mut file_stats.0);
    reset_isfs_callback();
    // SAFETY: the callback uses atomics. The API owns its request context and
    // frees it after IOS reports the failed descriptor.
    let get_file_stats_async = unsafe {
        file.get_stats_async(
            &mut file_stats.0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let get_file_stats_async_done = wait_isfs_callback();
    let get_file_stats_async_result = ISFS_CALLBACK_RESULT.load(Ordering::Acquire);
    // SAFETY: a negative raw descriptor is rejected by the ownership guard.
    let close_async = unsafe { file.close_async(None, core::ptr::null_mut()) };
    let close_invalid = filesystem::IsfsFile::from_raw(-1).close();

    let init = filesystem::initialize();
    logger.note_i32("ISFS deinitialize result", deinit);
    logger.note_i32("ISFS repeated deinitialize result", repeated_deinit);
    logger.note_i32("ISFS reinitialize result", init);
    logger.record(
        "ISFS service deinitialize and reinitialize",
        init_before_deinit == filesystem::ISFS_OK
            && deinit == filesystem::ISFS_OK
            && repeated_deinit == filesystem::ISFS_EINVAL
            && init == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS uninitialized synchronous APIs reject and preserve outputs",
        [
            get_stats,
            format,
            read_dir,
            get_usage,
            create_dir,
            create_file,
            delete,
            rename,
            get_attr,
            set_attr,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && stats
                == filesystem::IsfsStats {
                    a: 1,
                    b: 2,
                    c: 3,
                    d: 4,
                    e: 5,
                    f: 6,
                    g: 7,
                }
            && directory_count == 11
            && usage_a == 21
            && usage_b == 22
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36
            && matches!(open, Err(filesystem::ISFS_EINVAL))
            && seek.is_err()
            && get_file_stats < 0
            && close_invalid == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS get_stats_async rejects a closed service and preserves output",
        get_stats_async == filesystem::ISFS_EINVAL
            && get_stats_async_callback_not_called
            && stats
                == filesystem::IsfsStats {
                    a: 1,
                    b: 2,
                    c: 3,
                    d: 4,
                    e: 5,
                    f: 6,
                    g: 7,
                },
    );
    logger.record(
        "ISFS uninitialized async APIs reject without destructive submission",
        [
            format_async,
            read_dir_async,
            get_usage_async,
            create_dir_async,
            create_file_async,
            delete_async,
            rename_async,
            get_attr_async,
            set_attr_async,
            open_async,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && close_async == filesystem::ISFS_OK
            && raw == -1
            && empty_write == Err(filesystem::ISFS_EINVAL)
            && empty_write_all == Err(filesystem::ISFS_EINVAL)
            && empty_read == Err(filesystem::ISFS_EINVAL)
            && empty_read_async == filesystem::ISFS_EINVAL
            && empty_write_async == filesystem::ISFS_EINVAL
            && seek_async >= 0
            && seek_async_done
            && seek_async_result < 0
            && get_file_stats_async >= 0
            && get_file_stats_async_done
            && get_file_stats_async_result < 0,
    );
}

#[repr(align(32))]
struct AlignedFstats(filesystem::Fstats);

#[repr(align(32))]
struct AlignedIsfsBytes<const N: usize>([u8; N]);
