//! Internal driver contracts shared by Wii implementations.

use core::task::{Context, Poll};

use crate::WiiError;

/// Lifecycle state shared by globally owned Wii services.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SubsystemState {
    Uninitialized,
    Initializing,
    Ready,
    ShuttingDown,
    Failed(WiiError),
}

/// Explicit initialization and shutdown contract.
pub trait Lifecycle {
    fn state(&self) -> SubsystemState;
    fn initialize(&mut self) -> Result<(), WiiError>;
    fn shutdown(&mut self) -> Result<(), WiiError>;
}

/// Internal block-device interface used by filesystem and media services.
pub trait BlockDevice {
    fn block_size(&self) -> u32;
    fn block_count(&self) -> u64;
    fn read_blocks(&mut self, lba: u64, blocks: &mut [u8]) -> Result<(), WiiError>;
    fn write_blocks(&mut self, lba: u64, blocks: &[u8]) -> Result<(), WiiError>;
    fn flush(&mut self) -> Result<(), WiiError>;
}

/// Poll-based asynchronous operation contract.
pub trait AsyncOperation {
    type Output;

    fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<Self::Output, WiiError>>;
    fn cancel(&mut self) -> Result<(), WiiError>;
}

/// Transport used by debugger protocol implementations.
pub trait DebugTransport {
    fn open(&mut self) -> Result<(), WiiError>;
    fn read(&mut self, output: &mut [u8]) -> Result<usize, WiiError>;
    fn write(&mut self, input: &[u8]) -> Result<usize, WiiError>;
    fn close(&mut self) -> Result<(), WiiError>;
}

/// Transport used by USB and Bluetooth HID protocol layers.
pub trait HidTransport {
    fn send_report(&mut self, report: &[u8]) -> Result<(), WiiError>;
    fn receive_report(&mut self, report: &mut [u8]) -> Result<usize, WiiError>;
}
