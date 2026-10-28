use ogc_controller::{Axes, Buttons, ControllerId};

#[derive(Copy, Clone, Debug, Default)]
pub struct Input {
    pub held: Buttons,
    pub down: Buttons,
    pub up: Buttons,
    pub axes: Axes,
    pub connected: bool,
    #[cfg(any(feature = "action-trace", test))]
    pub timestamp_ticks: u64,
    #[cfg(any(feature = "action-trace", test))]
    pub timestamp_valid: bool,
    /// Present only when one controller supplied all active input in the scan.
    pub source: Option<ControllerId>,
}
