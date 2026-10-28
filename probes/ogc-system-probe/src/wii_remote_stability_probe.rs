//! Long-running Wii Remote stability check through the production input path.
use crate::controller::{ControllerKind, ControllerManager, ControllerSource, ControllerState};
use crate::probe_support::ProbeLogger;
use alloc::format;
use ogc_system as system;

const PAIRING_WINDOW_SECONDS: u64 = 90;
const HOLD_WINDOW_SECONDS: u64 = 180;
const STATUS_INTERVAL_SECONDS: u64 = 15;
const TICKS_PER_SECOND: u64 = 60_750_000;

#[derive(Default)]
struct StabilityCounts {
    scans: u32,
    connected_scans: u32,
    missing_scans: u32,
    disconnects: u32,
    reconnects: u32,
    button_presses: u32,
    button_releases: u32,
    saw_remote: bool,
    was_connected: bool,
    last_kind: Option<ControllerKind>,
}

impl StabilityCounts {
    fn observe(&mut self, remote: Option<ControllerState>) {
        self.scans = self.scans.saturating_add(1);
        let connected = remote.is_some_and(|state| state.connected);
        if let Some(state) = remote.filter(|state| state.connected) {
            self.connected_scans = self.connected_scans.saturating_add(1);
            self.saw_remote = true;
            self.last_kind = Some(state.kind);
            if !state.buttons_down.is_empty() {
                self.button_presses = self.button_presses.saturating_add(1);
            }
            if !state.buttons_up.is_empty() {
                self.button_releases = self.button_releases.saturating_add(1);
            }
        } else {
            self.missing_scans = self.missing_scans.saturating_add(1);
        }
        if self.was_connected && !connected {
            self.disconnects = self.disconnects.saturating_add(1);
        } else if !self.was_connected && connected && self.scans > 1 {
            self.reconnects = self.reconnects.saturating_add(1);
        }
        self.was_connected = connected;
    }

    fn has_input_cycle(&self) -> bool {
        self.button_presses > 0 && self.button_releases > 0
    }
}

/// Check Remote discovery, continuous connection, and physical button input.
/// The probe does not erase saved pairing data or request a disconnect.
pub fn run(logger: &mut ProbeLogger) {
    logger.section("WII REMOTE CONNECTION HOLD TEST");
    logger.enable_activity_indicator();
    logger.note("Use one Wii Remote. Pair it with SYNC or 1+2 if needed.");
    logger.note("Keep the Remote powered on. Press and release any button every 15 seconds.");
    logger.note("A real Wii run is required for hardware acceptance; Dolphin is diagnostic only.");

    let hardware = system::hardware_proof();
    logger.note(&format!(
        "Hollywood revision=0x{:08x} (hardware identity does not detect Dolphin)",
        hardware.hollywood_revision
    ));

    let mut manager = ControllerManager::new();
    manager.init();
    let init = manager.enable_wpad();
    logger.note_i32("WPAD start result", init);
    logger.record("WPAD start request accepted", init == 0);
    if init != 0 {
        return;
    }

    let pair_deadline = system::time().saturating_add(PAIRING_WINDOW_SECONDS * TICKS_PER_SECOND);
    let mut found = false;
    while system::time() < pair_deadline {
        let snapshot = manager.scan();
        found = snapshot.controllers().iter().any(is_wii_remote);
        if found {
            break;
        }
        crate::probe_support::wait_vsync();
    }
    logger.record("Wii Remote discovered on WPAD", found);
    if !found {
        finish_bluetooth(logger, &mut manager);
        return;
    }

    logger.note("Remote found. Hold test started; keep pressing and releasing a button.");
    let start = system::time();
    let deadline = start.saturating_add(HOLD_WINDOW_SECONDS * TICKS_PER_SECOND);
    let mut next_status = start;
    let mut counts = StabilityCounts::default();
    while system::time() < deadline {
        let snapshot = manager.scan();
        let remote = snapshot.controllers().iter().copied().find(is_wii_remote);
        counts.observe(remote);
        let now = system::time();
        if now >= next_status {
            let elapsed = now.saturating_sub(start) / TICKS_PER_SECOND;
            logger.note(&format!(
                "hold={}s connected_scans={} missing_scans={} disconnects={} reconnects={} button_edges={}/{}",
                elapsed,
                counts.connected_scans,
                counts.missing_scans,
                counts.disconnects,
                counts.reconnects,
                counts.button_presses,
                counts.button_releases
            ));
            next_status = now.saturating_add(STATUS_INTERVAL_SECONDS * TICKS_PER_SECOND);
        }
        crate::probe_support::wait_vsync();
    }

    logger.record(
        "Wii Remote remained present in every hold-test scan",
        counts.missing_scans == 0,
    );
    logger.record(
        "Wii Remote delivered button press and release events",
        counts.has_input_cycle(),
    );
    logger.record(
        "Wii Remote had no disconnect during hold test",
        counts.disconnects == 0,
    );
    let diagnostics = manager.bluetooth_diagnostics();
    logger.record(
        "Bluetooth host remained ready without a terminal error",
        diagnostics.stage == ogc_bt_sync::HostStage::Ready && diagnostics.failure.is_none(),
    );
    logger.note(&format!(
        "final scans={} connected={} missing={} disconnects={} reconnects={} presses={} releases={} kind={:?}",
        counts.scans,
        counts.connected_scans,
        counts.missing_scans,
        counts.disconnects,
        counts.reconnects,
        counts.button_presses,
        counts.button_releases,
        counts.last_kind
    ));
    logger.note("Dolphin results do not certify a physical Wii Remote connection.");
    finish_bluetooth(logger, &mut manager);
}

fn is_wii_remote(state: &ControllerState) -> bool {
    matches!(state.source, ControllerSource::WiiBluetooth(_))
        && matches!(
            state.kind,
            ControllerKind::WiiRemote | ControllerKind::WiiClassic | ControllerKind::WiiUPro
        )
}

fn finish_bluetooth(logger: &mut ProbeLogger, manager: &mut ControllerManager) {
    let result = manager.shutdown_bluetooth();
    logger.note_i32("WPAD shutdown result", result);
    logger.record("WPAD shutdown request accepted", result == 0);
    let deadline = system::time().saturating_add(6 * TICKS_PER_SECOND);
    let mut controllers_seen_during_shutdown = false;
    while !manager.bluetooth_shutdown_complete() && system::time() < deadline {
        let snapshot = manager.scan();
        controllers_seen_during_shutdown |= !snapshot.controllers().is_empty();
        crate::probe_support::wait_vsync();
    }
    logger.note(if controllers_seen_during_shutdown {
        "controller state remained visible during adapter shutdown"
    } else {
        "no controller state remained visible during adapter shutdown"
    });
    logger.record(
        "Bluetooth shutdown completed",
        manager.bluetooth_shutdown_complete(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::Buttons;

    fn state(
        source: ControllerSource,
        kind: ControllerKind,
        down: Buttons,
        up: Buttons,
    ) -> ControllerState {
        ControllerState {
            source,
            kind,
            connected: true,
            buttons: Buttons::empty(),
            buttons_down: down,
            buttons_up: up,
            axes: Default::default(),
        }
    }

    #[test]
    fn acceptance_counts_disconnect_and_requires_button_edges() {
        let mut counts = StabilityCounts::default();
        counts.observe(Some(state(
            ControllerSource::WiiBluetooth(0),
            ControllerKind::WiiRemote,
            Buttons::A,
            Buttons::empty(),
        )));
        counts.observe(Some(state(
            ControllerSource::WiiBluetooth(0),
            ControllerKind::WiiRemote,
            Buttons::empty(),
            Buttons::A,
        )));
        counts.observe(None);
        assert_eq!(counts.disconnects, 1);
        assert_eq!(counts.missing_scans, 1);
        assert!(counts.has_input_cycle());
    }

    #[test]
    fn a_classic_extension_on_wpad_is_a_wii_remote_connection() {
        assert!(is_wii_remote(&state(
            ControllerSource::WiiBluetooth(2),
            ControllerKind::WiiClassic,
            Buttons::empty(),
            Buttons::empty(),
        )));
        assert!(!is_wii_remote(&state(
            ControllerSource::BluetoothHid(2),
            ControllerKind::GenericBluetoothHid,
            Buttons::empty(),
            Buttons::empty(),
        )));
    }
}
