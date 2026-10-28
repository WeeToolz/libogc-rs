//! Small shared input adapter for the playable example DOLs.
//!
//! The default example path uses GameCube PAD and asynchronous USB HID.
//! Bluetooth input uses the default `wii-controllers` feature.

use ogc_controller::{ControllerManager, ControllerSnapshot};

#[path = "controller_input_types.rs"]
mod input_model;
pub use input_model::Input;

#[path = "action_input.rs"]
pub(crate) mod action_input;
pub use action_input::project_snapshot;

#[cfg(feature = "action-trace")]
#[path = "action_trace.rs"]
pub mod action_trace;

/// Initialize PAD, attach input tracing, and enable the selected production
/// transports. The callback configures diagnostics before Bluetooth starts.
pub fn start_manager(
    manager: &mut ControllerManager,
    usb_hid: bool,
    bluetooth: bool,
    configure: impl FnOnce(&mut ControllerManager),
) -> Result<(), i32> {
    manager.init();
    attach_trace(manager);
    configure(manager);
    if usb_hid {
        manager.enable_usb_hid();
    }
    if bluetooth {
        let status = manager.enable_bluetooth_hid();
        if status != 0 {
            return Err(status);
        }
    }
    Ok(())
}

pub(crate) fn report_controller_errors(snapshot: &ControllerSnapshot) {
    for error in snapshot.errors() {
        ogc_system::report(format_args!(
            "Controller input error: source={:?} operation={} failure={:?} cleanup={:?}\n",
            error.source, error.operation, error.failure, error.cleanup_code
        ));
    }
}

pub fn attach_trace(manager: &mut ControllerManager) {
    #[cfg(feature = "action-trace")]
    manager.set_input_trace_observer(Some(action_trace::observe_input));
    #[cfg(not(feature = "action-trace"))]
    let _ = manager;
}

#[cfg(feature = "action-trace")]
pub fn record_action(
    game: &'static str,
    screen: &'static str,
    action: &'static str,
    frame: impl core::fmt::Display,
    input: Input,
) {
    match action_trace::record_action(game, screen, action, frame, input) {
        Ok(()) => {}
        Err(error) => {
            ogc_system::report(format_args!(
                "ACTION TRACE FAILED: {game} action record: {error:?}\n"
            ));
            action_trace::check_failed();
        }
    }
}
