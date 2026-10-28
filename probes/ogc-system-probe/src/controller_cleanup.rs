use alloc::format;

use ogc_lwp::sys as lwp;
use ogc_system as system;
use ogc_video as video;
use wpad_sys as wpad;

use crate::controller::{ControllerError, ControllerManager, MAX_CONTROLLERS};
use crate::input_probe::controller_cleanup_state::ControllerCleanupState;
use crate::probe_support::ProbeLogger;

const CLEANUP_TIMEOUT_TICKS: u64 = 6 * lwp::TICKS_PER_SECOND;
const CLEANUP_POLL_TIMEOUT_US: u64 = 100_000;

/// Stop Bluetooth and USB input, then drain real controller work before the
/// probe writes its final summary. Any operation error remains a failed result,
/// even if a later retry completes cleanup.
pub(crate) fn shutdown_controller_manager(
    logger: &mut ProbeLogger,
    manager: &mut ControllerManager,
) -> bool {
    let mut state = ControllerCleanupState::new();
    let mut reported_errors = [None; MAX_CONTROLLERS];

    match manager.shutdown_usb_hid() {
        Ok(complete) => state.observe_usb_complete(complete),
        Err(error) => {
            state.record_failure();
            record_error(logger, &mut reported_errors, error);
        }
    }

    let bluetooth_result = manager.shutdown_bluetooth();
    logger.note_i32("Bluetooth shutdown submission", bluetooth_result);
    let bluetooth_submitted = bluetooth_result == wpad::WPAD_ERR_NONE as i32;
    logger.record("Bluetooth shutdown request accepted", bluetooth_submitted);
    if !bluetooth_submitted {
        state.record_failure();
    }

    let deadline = system::time().saturating_add(CLEANUP_TIMEOUT_TICKS);
    let mut wait_timeout_reported = false;
    loop {
        state.observe_bluetooth_complete(manager.bluetooth_shutdown_complete());
        if state.transports_complete() || system::time() >= deadline {
            break;
        }

        let snapshot = manager.scan_at(system::time());
        for error in snapshot.errors() {
            state.record_failure();
            record_error(logger, &mut reported_errors, *error);
        }

        match manager.shutdown_usb_hid() {
            Ok(complete) => state.observe_usb_complete(complete),
            Err(error) => {
                state.record_failure();
                record_error(logger, &mut reported_errors, error);
            }
        }
        state.observe_bluetooth_complete(manager.bluetooth_shutdown_complete());
        if state.transports_complete() {
            break;
        }
        if !video::wait_vsync_timeout_us(CLEANUP_POLL_TIMEOUT_US) && !wait_timeout_reported {
            wait_timeout_reported = true;
            state.record_failure();
            logger.record("bounded cleanup poll wait completed", false);
            logger.note("video retrace did not complete before the 100 ms cleanup poll timeout");
        }
        crate::probe_support::service_power_shutdown();
    }

    let usb_complete = state.usb_complete();
    let bluetooth_complete = manager.bluetooth_shutdown_complete();
    state.observe_bluetooth_complete(bluetooth_complete);
    let diagnostics = manager.bluetooth_diagnostics();
    logger.record("USB requests and descriptors drained", usb_complete);
    logger.record("Bluetooth shutdown completed", bluetooth_complete);
    logger.record(
        "Bluetooth transport has no terminal failure",
        diagnostics.failure.is_none(),
    );
    if diagnostics.failure.is_some() {
        state.record_failure();
        logger.note(&format!("Bluetooth terminal diagnostics: {diagnostics:?}"));
    }
    let complete = state.is_complete();
    if !state.transports_complete() && system::time() >= deadline {
        logger.note("controller cleanup deadline expired");
    }
    complete
}

fn record_error(
    logger: &mut ProbeLogger,
    reported: &mut [Option<ControllerError>; MAX_CONTROLLERS],
    error: ControllerError,
) {
    if reported.iter().flatten().any(|prior| *prior == error) {
        return;
    }
    if let Some(slot) = reported.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(error);
    }
    logger.note(&format!("controller cleanup/scan error: {error:?}"));
    logger.record("controller cleanup and scan completed without error", false);
}
