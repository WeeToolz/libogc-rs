//! Input adapter for examples that use the process-global controller manager.

use crate::controller_input::{self, Input};
use ogc_controller::ControllerManager;
#[cfg(feature = "wii-controllers")]
use ogc_controller::MAX_CONTROLLERS;

static mut MANAGER: ControllerManager = ControllerManager::new();
#[cfg(feature = "wii-controllers")]
static mut STICK_MAPPER: ogc_controller::DigitalStickMapper<MAX_CONTROLLERS> =
    ogc_controller::DigitalStickMapper::new();

pub fn init(usb_hid: bool, bluetooth: bool) {
    // SAFETY: each example has one main thread and owns this manager for its lifetime.
    unsafe {
        let manager = &mut *core::ptr::addr_of_mut!(MANAGER);
        if let Err(status) = controller_input::start_manager(manager, usb_hid, bluetooth, |_| {}) {
            ogc_system::report(format_args!(
                "Controller initialization: Bluetooth initialization failed ({status})\n"
            ));
        }
    }
}

#[cfg(not(feature = "wii-controllers"))]
pub fn poll() -> Input {
    let ticks = ogc_system::time();
    let snapshot = unsafe { (&mut *core::ptr::addr_of_mut!(MANAGER)).scan_at(ticks) };
    controller_input::report_controller_errors(&snapshot);
    let input = controller_input::project_snapshot(snapshot, ticks);
    #[cfg(feature = "action-trace")]
    controller_input::action_trace::check_failed();
    input
}

#[cfg(feature = "wii-controllers")]
pub fn poll_with_profile(profile: crate::half_controller_input::HalfControllerProfile) -> Input {
    let ticks = ogc_system::time();
    let snapshot = unsafe { (&mut *core::ptr::addr_of_mut!(MANAGER)).scan_at(ticks) };
    controller_input::report_controller_errors(&snapshot);
    let input = match crate::half_controller_input::project_wii_half_snapshot(
        snapshot,
        ticks,
        profile,
        unsafe { &mut *core::ptr::addr_of_mut!(STICK_MAPPER) },
    ) {
        Ok(input) => input,
        Err(error) => {
            ogc_system::report(format_args!(
                "Joy-Con half input mapping failed: {error:?}\n"
            ));
            controller_input::project_snapshot(snapshot, ticks)
        }
    };
    #[cfg(feature = "action-trace")]
    controller_input::action_trace::check_failed();
    input
}
