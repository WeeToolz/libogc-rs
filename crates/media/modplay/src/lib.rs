#![no_std]

pub use ogc_sys as sys;

mod tables;

pub use tables::{FREQTAB, FREQTAB_LEN, SEMITONETAB, SEMITONETAB_LEN};

pub const REVIEW_STATUS: &str = "phase-1-tables";
pub const TARGET_ENDIANNESS: u32 = sys::TARGET_BYTE_ORDER;
