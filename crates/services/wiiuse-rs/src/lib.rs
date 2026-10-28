#![no_std]
#![forbid(unsafe_code)]

//! Portable Wii Remote behavior ported from Wiiuse.
//!
//! This crate contains shared, transport-independent Wiiuse behavior: the
//! Bluetooth adapter patch state machine, initial Remote handshake, memory
//! read assembly, and accelerometer calibration decoder. The target crate
//! supplies Bluetooth HCI/HID transport and performs Remote reads through
//! libogc-rs.

mod adapter_patch;
mod balance_board;
mod buttons;
mod calibration;
mod classic;
mod command_queue;
mod guitar_hero_3;
mod initial_handshake;
mod motion_plus;
mod nunchuk;
mod read_assembly;
mod status;

pub use adapter_patch::{AdapterPatch, AdapterPatchAction, AdapterPatchError};
pub use balance_board::{BalanceBoardCalibration, BalanceBoardReport};
pub use buttons::{ButtonState, WIIMOTE_BUTTON_ALL};
pub use calibration::AccelCalibration;
pub use classic::{ClassicCalibration, ClassicReport};
pub use command_queue::{
    AckCompletion, AckReport, CommandEnqueue, CommandQueueError, CommandTag, ExpansionSetup,
    ExpansionSetupAction, WII_COMMAND_MAX_BYTES, WII_COMMAND_QUEUE_CAPACITY, WiiCommand,
    WiiCommandQueue, decode_ack_report, encode_wii_read_command, encode_wii_write_command,
};
pub use guitar_hero_3::{GuitarHero3AnalogState, GuitarHero3Report};
pub use motion_plus::MotionPlusReport;
pub use nunchuk::{NunchukCalibration, NunchukReport};
pub use read_assembly::{ReadAssembly, ReadFragmentResult};
pub use status::RemoteStatusReport;

pub use initial_handshake::{
    HandshakeAction, InitialHandshake, InitialHandshakeError, WII_U_PRO_EXPANSION_ID,
};
