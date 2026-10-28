//! Interactive Bluetooth monitor uses the same host as games and WPAD.
use crate::probe_support::ProbeLogger;
pub fn run(logger: &mut ProbeLogger) -> ! {
    crate::bluetooth_probe::monitor(logger)
}
