//! Host checks for the production libmad ABI implementation.
extern crate alloc;
pub use crate as imp;
#[path = "../../../crates/media/mad-sys/src/imp/abi.rs"]
pub mod abi;
#[path = "../../../crates/media/mad-sys/src/imp/bit.rs"]
pub mod bit;
#[path = "../../../crates/media/mad-sys/src/imp/callbacks.rs"]
pub mod callbacks;
#[path = "../../../crates/media/mad-sys/src/imp/decoder.rs"]
pub mod decoder;
#[path = "../../../crates/media/mad-sys/src/imp/error.rs"]
pub mod error;
#[path = "../../../crates/media/mad-sys/src/imp/fixed.rs"]
pub mod fixed;
#[path = "../../../crates/media/mad-sys/src/imp/frame.rs"]
pub mod frame;
#[path = "../../../crates/media/mad-sys/src/imp/stream.rs"]
pub mod stream;
#[path = "../../../crates/media/mad-sys/src/imp/synth.rs"]
pub mod synth;
#[path = "../../../crates/media/mad-sys/src/imp/timer.rs"]
pub mod timer;
