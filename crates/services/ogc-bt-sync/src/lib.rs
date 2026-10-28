#![no_std]
//! Bluetooth HCI/L2CAP wire formats and the Wii internal adapter transport.
//! Protocol code is portable so the same code can be tested on the host.

extern crate alloc;

mod request;
pub use request::TransferRequest;

mod hid_host;
pub use hid_host::*;

mod report_queue;
pub use report_queue::{
    BLUETOOTH_INPUT_QUEUE_CAPACITY, BLUETOOTH_REPORT_MAX_BYTES, BluetoothInputEvent,
    BluetoothInputKind, BluetoothInputQueue, BluetoothInputQueueError,
};

mod protocol;
pub use protocol::*;

#[cfg(target_arch = "powerpc")]
mod transport;
#[cfg(target_arch = "powerpc")]
pub use transport::*;

#[cfg(target_arch = "powerpc")]
mod wii_host;
#[cfg(target_arch = "powerpc")]
pub use wii_host::WiiHidHost;

/// Shared Nintendo Switch HID packets and parsers.
pub mod switch;
