//! Tests the same GU math source that the PowerPC crates use.
extern crate self as ogc_gu_sys;

#[path = "../../../crates/hardware/ogc-gu-sys/src/math.rs"]
mod scalar;
pub use scalar::*;

#[path = "../../../crates/hardware/ogc-gu/src/math.rs"]
pub mod math;

#[cfg(test)]
mod tests;
