#![no_std]
//! Safe convenience wrappers for libogc's GameCube controller PAD API.
//!
//! The raw C-compatible symbols remain available through `sys` and the public
//! re-exports. The functions in this crate keep fixed-size buffers in Rust
//! types where the C API expects them.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_input_sys as sys;
pub use ogc_input_sys::*;
pub use sys::{
    Bool, DebugHook, PAD_BUTTON_A, PAD_BUTTON_B, PAD_BUTTON_DOWN, PAD_BUTTON_LEFT, PAD_BUTTON_MENU,
    PAD_BUTTON_RIGHT, PAD_BUTTON_START, PAD_BUTTON_UP, PAD_BUTTON_X, PAD_BUTTON_Y, PAD_CHAN0,
    PAD_CHAN0_BIT, PAD_CHAN1, PAD_CHAN1_BIT, PAD_CHAN2, PAD_CHAN2_BIT, PAD_CHAN3, PAD_CHAN3_BIT,
    PAD_CHANMAX, PAD_ERR_NO_CONTROLLER, PAD_ERR_NONE, PAD_ERR_NOT_READY, PAD_ERR_TRANSFER,
    PAD_MOTOR_RUMBLE, PAD_MOTOR_STOP, PAD_MOTOR_STOP_HARD, PAD_TRIGGER_L, PAD_TRIGGER_R,
    PAD_TRIGGER_Z, PadStatus, SamplingCallback,
};

pub fn init() -> u32 {
    sys::pad_init()
}

pub fn sync() -> u32 {
    sys::pad_sync()
}

/// Reads the state of all GameCube controller channels.
pub fn read(status: &mut [PadStatus; PAD_CHANMAX as usize]) -> u32 {
    unsafe { sys::pad_read(status.as_mut_ptr()) }
}

pub fn reset(mask: u32) -> u32 {
    sys::pad_reset(mask)
}

pub fn recalibrate(mask: u32) -> u32 {
    sys::pad_recalibrate(mask)
}

/// Applies libogc's analog-stick clamping to a full PAD status buffer.
pub fn clamp(status: &mut [PadStatus; PAD_CHANMAX as usize]) {
    unsafe { sys::pad_clamp(status.as_mut_ptr()) }
}

pub fn control_motor(chan: i32, cmd: u32) {
    sys::pad_control_motor(chan, cmd)
}

pub fn set_spec(spec: u32) {
    sys::pad_set_spec(spec)
}

pub fn scan_pads() -> u32 {
    sys::pad_scan_pads()
}

/// Scans once and saves the decoded PAD samples used by the button accessors.
pub fn scan_pads_with_status(status: &mut [PadStatus; PAD_CHANMAX as usize]) -> u32 {
    sys::pad_scan_pads_with_status(status)
}

pub fn buttons_up(pad: i32) -> u16 {
    sys::pad_buttons_up(pad)
}

pub fn buttons_down(pad: i32) -> u16 {
    sys::pad_buttons_down(pad)
}

pub fn buttons_held(pad: i32) -> u16 {
    sys::pad_buttons_held(pad)
}

pub fn substick_x(pad: i32) -> i8 {
    sys::pad_sub_stick_x(pad)
}

pub fn substick_y(pad: i32) -> i8 {
    sys::pad_sub_stick_y(pad)
}

pub fn stick_x(pad: i32) -> i8 {
    sys::pad_stick_x(pad)
}

pub fn stick_y(pad: i32) -> i8 {
    sys::pad_stick_y(pad)
}

pub fn trigger_l(pad: i32) -> u8 {
    sys::pad_trigger_l(pad)
}

pub fn trigger_r(pad: i32) -> u8 {
    sys::pad_trigger_r(pad)
}

pub fn set_sampling_callback(cb: SamplingCallback) -> SamplingCallback {
    sys::pad_set_sampling_callback(cb)
}

pub fn set_debug_hook(hook: DebugHook) {
    sys::set_debug_hook(hook)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn pad_status_layout_matches_c() {
        assert_eq!(core::mem::size_of::<PadStatus>(), 12);
        assert_eq!(core::mem::align_of::<PadStatus>(), 2);
    }

    #[test]
    fn c_style_api_is_visible() {
        let _ = pad_init as fn() -> u32;
        let _ = pad_scan_pads as fn() -> u32;
        let _ = pad_buttons_held as fn(i32) -> u16;
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> u32;
        let _ = sync as fn() -> u32;
        let _ = reset as fn(u32) -> u32;
        let _ = recalibrate as fn(u32) -> u32;
        let _ = control_motor as fn(i32, u32);
        let _ = set_spec as fn(u32);
        let _ = scan_pads as fn() -> u32;
        let _ = buttons_up as fn(i32) -> u16;
        let _ = buttons_down as fn(i32) -> u16;
        let _ = buttons_held as fn(i32) -> u16;
        let _ = substick_x as fn(i32) -> i8;
        let _ = substick_y as fn(i32) -> i8;
        let _ = stick_x as fn(i32) -> i8;
        let _ = stick_y as fn(i32) -> i8;
        let _ = trigger_l as fn(i32) -> u8;
        let _ = trigger_r as fn(i32) -> u8;
        let _ = set_sampling_callback as fn(SamplingCallback) -> SamplingCallback;
        let _ = set_debug_hook as fn(DebugHook);
    }
}
