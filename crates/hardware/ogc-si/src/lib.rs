#![no_std]
//! Serial Interface helpers for GameCube controller and accessory ports.
//!
//! This crate provides thin Rust names for libogc SI calls while keeping the
//! raw C ABI available through `sys` and public re-exports.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_si_sys as sys;
pub use sys::*;

pub fn init() {
    sys::init()
}

pub fn sync() -> u32 {
    sys::sync()
}

pub fn busy() -> u32 {
    sys::busy()
}

pub fn is_chan_busy(chan: i32) -> u32 {
    sys::is_chan_busy(chan)
}

pub fn set_xy(line: u16, cnt: u8) {
    sys::set_xy(line, cnt)
}

pub fn enable_polling(poll: u32) {
    sys::enable_polling(poll)
}

pub fn disable_polling(poll: u32) {
    sys::disable_polling(poll)
}

pub fn set_sampling_rate(rate: u32) {
    sys::set_sampling_rate(rate)
}

pub fn refresh_sampling_rate() {
    sys::refresh_sampling_rate()
}

pub fn get_status(chan: i32) -> u32 {
    sys::get_status(chan)
}

pub fn get_response_raw(chan: i32) -> u32 {
    sys::get_response_raw(chan)
}

/// # Safety
///
/// `buf` must be null or valid, properly aligned, and writable for two consecutive `u32` values.
pub unsafe fn get_response(chan: i32, buf: *mut c_void) -> u32 {
    unsafe { sys::get_response(chan, buf) }
}

pub fn set_command(chan: i32, cmd: u32) {
    sys::set_command(chan, cmd)
}

pub fn get_command(chan: i32) -> u32 {
    sys::get_command(chan)
}

/// # Safety
///
/// `out` and `in_` must be valid for the requested transfer sizes.
pub unsafe fn transfer(
    chan: i32,
    out: *const c_void,
    out_len: u32,
    in_: *mut c_void,
    in_len: u32,
    cb: SiCallback,
    us_delay: u32,
) -> u32 {
    unsafe { sys::transfer(chan, out, out_len, in_, in_len, cb, us_delay) }
}

pub fn get_type(chan: i32) -> u32 {
    sys::get_type(chan)
}

pub fn get_type_async(chan: i32, cb: SiCallback) -> u32 {
    sys::get_type_async(chan, cb)
}

pub fn decode_type(type_: u32) -> u32 {
    sys::decode_type(type_)
}

pub fn probe(chan: i32) -> u32 {
    sys::probe(chan)
}

pub fn transfer_commands() {
    sys::transfer_commands()
}

pub fn register_polling_handler(handler: RdstHandler) -> u32 {
    sys::register_polling_handler(handler)
}

pub fn unregister_polling_handler(handler: RdstHandler) -> u32 {
    sys::unregister_polling_handler(handler)
}

pub fn enable_polling_interrupt(enable: i32) -> u32 {
    sys::enable_polling_interrupt(enable)
}
