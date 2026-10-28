#![no_std]

//! Shared contracts for Wii runtime drivers.
//!
//! This crate deliberately contains no emulated devices, global subsystem state,
//! or compatibility return-code translation. Driver crates implement these
//! interfaces; ABI and safe-wrapper crates translate at their boundaries.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "ogc-runtime is a Wii runtime interface crate; build it with the powerpc-nintendo-none target"
);

pub mod driver;
pub mod error;
pub mod scheduler;

pub use driver::{
    AsyncOperation, BlockDevice, DebugTransport, HidTransport, Lifecycle, SubsystemState,
};
pub use error::{ExiError, NetworkError, UsbError, WiiError};
pub use scheduler::{
    CallbackContext, Deadline, DeferredCallback, DeferredExecutor, InterruptWake, MonotonicClock,
    ThreadId, TimerService, WaitOutcome, WakeToken,
};
