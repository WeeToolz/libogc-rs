//! Bluetooth acceptance probe and interactive monitor use the application input path.
use crate::controller::{Buttons, ControllerManager, ControllerSource, bluetooth_stage_text};
use crate::probe_support::{ProbeLogger, send_agent_progress};
use alloc::format;
use ogc_bt_sync as bt;
use ogc_system as system;
use wpad_sys as wpad;

const TEST_SECONDS: u64 = 75;
fn milliseconds() -> u64 {
    system::time() / 60_750
}

struct Observation {
    connected: bool,
    reports: u32,
    buttons: u32,
    released: u32,
    min: [i16; 4],
    max: [i16; 4],
}
impl Observation {
    fn new() -> Self {
        Self {
            connected: false,
            reports: 0,
            buttons: 0,
            released: 0,
            min: [i16::MAX; 4],
            max: [i16::MIN; 4],
        }
    }
    fn moved(&self) -> bool {
        (0..4).any(|i| self.max[i] as i32 - self.min[i] as i32 > 8192)
    }
}

/// Runs a bounded hardware acceptance test. All diagnostic screen text goes
/// through ProbeLogger and is included in the existing final report transcript.
pub fn run(logger: &mut ProbeLogger) {
    logger.note("Bluetooth probe entry; PAD setup starting");
    #[cfg(feature = "dolphin-automation")]
    logger.record(
        "lwbt disconnect waits for both L2CAP responses before completion",
        lwbt_sys::dolphin_disconnect_response_order_probe(),
    );
    send_agent_progress(100);
    let mut manager = ControllerManager::new();
    manager.init();
    logger.note("Bluetooth PAD setup completed");
    send_agent_progress(101);
    logger.section("BLUETOOTH HID HARDWARE TEST");
    logger.note("Put one Joy-Con or the PDP REMATCH pad in pairing mode.");
    logger.note("Test one controller at a time. Press and release buttons.");
    logger.note("Move each available stick and the D-pad or direction buttons.");
    logger.note("The test runs for 75 seconds and then returns a report.");
    logger.note("Bluetooth adapter open submission starting");
    send_agent_progress(102);
    let result = manager.enable_bluetooth_hid();
    logger.note_i32("bluetooth start submission", result);
    logger.record(
        "Bluetooth startup request accepted",
        result == wpad::WPAD_ERR_NONE as i32,
    );
    send_agent_progress(103);
    // Separate the first public PAD and WPAD calls for boot diagnosis. Normal
    // input sampling below still uses ControllerManager for every source.
    logger.note("Bluetooth probe first PAD scan starting");
    send_agent_progress(105);
    let _ = ogc_input::scan_pads();
    logger.note("Bluetooth probe first PAD scan completed");
    send_agent_progress(106);
    logger.note("Bluetooth probe first WPAD poll starting");
    send_agent_progress(107);
    wpad::set_bluetooth_poll_hook(Some(poll_marker));
    let _ = wpad::scan_pads();
    wpad::set_bluetooth_poll_hook(None);
    logger.note("Bluetooth probe first WPAD poll completed");
    send_agent_progress(108);
    let start = milliseconds();
    let mut next_log = start;
    let mut previous = None;
    let mut observations: [Observation; 4] = core::array::from_fn(|_| Observation::new());
    while milliseconds().saturating_sub(start) < TEST_SECONDS * 1000 {
        let snapshot = manager.scan();
        let diagnostic = manager.bluetooth_diagnostics();
        let now = milliseconds();
        if previous != Some((diagnostic.stage, diagnostic.failure)) || now >= next_log {
            log_diagnostics(logger, diagnostic);
            previous = Some((diagnostic.stage, diagnostic.failure));
            next_log = now + 5000;
        }
        for state in snapshot.controllers() {
            let slot = match state.source {
                ControllerSource::BluetoothHid(slot) => slot as usize,
                _ => continue,
            };
            let observation = &mut observations[slot];
            observation.connected = true;
            observation.buttons |= state.buttons.bits();
            observation.released |= state.buttons_up.bits();
            let axes = [
                state.axes.left_x,
                state.axes.left_y,
                state.axes.right_x,
                state.axes.right_y,
            ];
            for (i, axis) in axes.into_iter().enumerate() {
                observation.min[i] = observation.min[i].min(axis);
                observation.max[i] = observation.max[i].max(axis);
            }
            let raw = wpad::bluetooth_hid_state(slot);
            observation.reports = observation.reports.max(raw.report_count);
        }
        if diagnostic.stage == bt::HostStage::Failed {
            break;
        }
        logger.display().wait_vsync();
    }
    let diagnostic = manager.bluetooth_diagnostics();
    logger.record(
        "Bluetooth adapter reaches ready",
        diagnostic.stage == bt::HostStage::Ready,
    );
    logger.record(
        "Bluetooth controller reaches input ready",
        observations.iter().any(|o| o.connected),
    );
    logger.record(
        "Bluetooth HID reports received",
        observations.iter().any(|o| o.reports > 0),
    );
    logger.record(
        "Bluetooth buttons pressed and released",
        observations.iter().any(|o| o.buttons & o.released != 0),
    );
    logger.record(
        "Bluetooth analog stick movement received",
        observations.iter().any(Observation::moved),
    );
    for (slot, observation) in observations.iter().enumerate().filter(|(_, o)| o.connected) {
        logger.note(&format!(
            "bluetooth slot={} reports={} buttons=0x{:x} released=0x{:x} min={:?} max={:?}",
            slot,
            observation.reports,
            observation.buttons,
            observation.released,
            observation.min,
            observation.max
        ));
    }
    shutdown(logger, &mut manager);
}

fn poll_marker(stage: &'static str) {
    // Keep this observer free of allocation, WPAD calls, and network sends.
    let _ = crate::probe_support::screen_write(stage.as_bytes());
    let _ = crate::probe_support::screen_write(b"\n");
}

pub fn monitor(logger: &mut ProbeLogger) -> ! {
    let mut manager = ControllerManager::new();
    manager.init();
    logger.section("BLUETOOTH HID MONITOR");
    logger.note("GameCube pad: A searches, B disconnects, X joins Joy-Cons.");
    let _ = manager.enable_bluetooth_hid();
    let mut previous = None;
    let mut next_log = 0;
    let mut buttons = [0u32; 4];
    loop {
        let snapshot = manager.scan();
        // The controller under test must not operate the diagnostic menu.
        let mut down = Buttons::empty();
        for state in snapshot.controllers() {
            if matches!(state.source, ControllerSource::GameCube(_)) {
                down |= state.buttons_down;
            }
        }
        if down.contains(Buttons::A) {
            let _ = wpad::search();
        }
        if down.contains(Buttons::B) {
            for slot in 0..4 {
                let _ = wpad::disconnect(slot);
            }
        }
        if down.contains(Buttons::X) {
            logger.note(if manager.pair_joycons() {
                "Joy-Cons joined"
            } else {
                "Joy-Con pair unavailable"
            });
        }
        let diagnostic = manager.bluetooth_diagnostics();
        let now = milliseconds();
        if previous != Some((diagnostic.stage, diagnostic.failure)) || now >= next_log {
            log_diagnostics(logger, diagnostic);
            previous = Some((diagnostic.stage, diagnostic.failure));
            next_log = now + 5000;
        }
        for state in snapshot.controllers() {
            if let ControllerSource::BluetoothHid(slot) = state.source {
                let slot = slot as usize;
                if buttons[slot] != state.buttons.bits() {
                    buttons[slot] = state.buttons.bits();
                    logger.note(&format!(
                        "bluetooth slot={} kind={:?} buttons=0x{:x} axes={:?}",
                        slot,
                        state.kind,
                        state.buttons.bits(),
                        state.axes
                    ));
                }
            }
        }
        logger.display().wait_vsync();
    }
}
fn log_diagnostics(logger: &mut ProbeLogger, d: bt::HostDiagnostics) {
    logger.note(bluetooth_stage_text(d.stage));
    logger.note(&format!("bluetooth failed_stage={:?} failure={:?} failed_opcode={:?} hci_status={:?} simple_pairing={} searching={} events={} acl={} reports={} malformed={} connections={} disconnects={}",
        d.failed_stage,d.failure,d.failed_opcode,d.hci_status,d.simple_pairing,d.searching,d.events,d.acl_packets,d.reports,d.malformed,d.connections,d.disconnects));
    send_agent_progress(104);
}
fn shutdown(logger: &mut ProbeLogger, manager: &mut ControllerManager) {
    let result = manager.shutdown_bluetooth();
    logger.note_i32("bluetooth shutdown submission", result);
    let deadline = milliseconds() + 6000;
    while !manager.bluetooth_shutdown_complete() && milliseconds() < deadline {
        let _ = manager.scan();
        logger.display().wait_vsync();
    }
    logger.record(
        "Bluetooth adapter close completed",
        manager.bluetooth_shutdown_complete(),
    );
    logger.record(
        "Bluetooth transport has no terminal failure",
        manager.bluetooth_diagnostics().failure.is_none(),
    );
}
