//! Scan helpers for examples that own a manager outside `controller_input`.

use crate::controller_input::{self, Input};
#[cfg(feature = "wii-controllers")]
use ogc_controller::MAX_CONTROLLERS;
use ogc_controller::{ControllerManager, ControllerSnapshot};

#[cfg(feature = "wii-controllers")]
static mut STICK_MAPPER: ogc_controller::DigitalStickMapper<MAX_CONTROLLERS> =
    ogc_controller::DigitalStickMapper::new();

#[cfg(not(feature = "wii-controllers"))]
pub fn scan_manager(manager: &mut ControllerManager) -> (ControllerSnapshot, Input) {
    let ticks = ogc_system::time();
    let snapshot = manager.scan_at(ticks);
    controller_input::report_controller_errors(&snapshot);
    let input = controller_input::project_snapshot(snapshot, ticks);
    #[cfg(feature = "action-trace")]
    controller_input::action_trace::check_failed();
    (snapshot, input)
}

#[cfg(feature = "wii-controllers")]
pub fn scan_manager_wii(
    manager: &mut ControllerManager,
    profile: crate::half_controller_input::HalfControllerProfile,
) -> (ControllerSnapshot, Input) {
    let ticks = ogc_system::time();
    let snapshot = manager.scan_at(ticks);
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
    (snapshot, input)
}
