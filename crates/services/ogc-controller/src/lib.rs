#![no_std]
//! Production controller model, decoding, and Wii/GameCube polling.
//! Portable protocol code is tested directly on the host. Hardware polling
//! is available only on PowerPC and always uses the real PAD/WPAD/IOS paths.

mod transitions;
pub use transitions::*;
mod digital_stick;
pub use digital_stick::*;
mod model;
pub use model::*;
pub mod hid;
mod reports;
pub use reports::*;
#[cfg(any(test, target_arch = "powerpc"))]
mod usb_report_state;
mod xbox_report;
pub use ogc_bt_sync::switch::*;
pub mod usb_hid;
#[cfg(any(test, target_arch = "powerpc"))]
mod usb_switch;

#[cfg(target_arch = "powerpc")]
mod hardware;
#[cfg(target_arch = "powerpc")]
pub use hardware::*;
