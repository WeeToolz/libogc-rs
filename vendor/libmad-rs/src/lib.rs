#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

extern crate alloc;

pub mod bit;
pub mod decoder;
pub mod fixed;
pub mod frame;
pub mod layer;
pub mod stream;
pub mod synth;
pub mod tables;
pub mod timer;

pub use frame::{Emphasis, Header, Layer, Mode};
pub use stream::{Error, MAD_BUFFER_GUARD, MAD_BUFFER_MDLEN, Stream};

#[cfg(test)]
mod tests;
