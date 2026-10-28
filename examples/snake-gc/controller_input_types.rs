#[cfg(any(feature = "action-trace", test))]
use ogc_controller::Axes;
use ogc_controller::{Buttons, ControllerId};

#[derive(Copy, Clone, Debug, Default)]
pub struct Input {
    pub held: Buttons,
    pub down: Buttons,
    pub up: Buttons,
    #[cfg(any(feature = "action-trace", test))]
    pub axes: Axes,
    #[cfg(any(feature = "action-trace", test))]
    pub connected: bool,
    #[cfg(any(feature = "action-trace", test))]
    pub timestamp_ticks: u64,
    #[cfg(any(feature = "action-trace", test))]
    pub timestamp_valid: bool,
    /// Present only when one controller supplied all active input in the scan.
    pub source: Option<ControllerId>,
}
