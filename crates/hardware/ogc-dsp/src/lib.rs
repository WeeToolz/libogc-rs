#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_dsp_sys as sys;
pub use sys::*;

pub fn init() {
    sys::init()
}

pub fn register_callback(cb: DspCallback) -> DspCallback {
    sys::register_callback(cb)
}

pub fn check_mail_to() -> u32 {
    sys::check_mail_to()
}

pub fn check_mail_from() -> u32 {
    sys::check_mail_from()
}

pub fn read_mail_from() -> u32 {
    sys::read_mail_from()
}

pub fn send_mail_to(mail: u32) {
    sys::send_mail_to(mail)
}

pub fn read_cpu_to_dsp() -> u32 {
    sys::read_cp_uto_dsp()
}

pub fn assert_int() {
    sys::assert_int()
}

pub fn reset() {
    sys::reset()
}

/// Stop DSP activity before a system reset.
pub fn shutdown() {
    sys::shutdown()
}

pub fn halt() {
    sys::halt()
}

pub fn unhalt() {
    sys::unhalt()
}

pub fn get_dma_status() -> u32 {
    sys::get_dma_status()
}

/// # Safety
///
/// `task` must point to a valid `DsptaskT` for the duration of the call.
pub unsafe fn add_task(task: *mut DsptaskT) -> *mut DsptaskT {
    unsafe { sys::add_task(task) }
}

/// # Safety
///
/// `task` must point to a valid `DsptaskT`.
pub unsafe fn cancel_task(task: *mut DsptaskT) {
    unsafe { sys::cancel_task(task) }
}

/// # Safety
///
/// `task` must point to a valid `DsptaskT`.
pub unsafe fn assert_task(task: *mut DsptaskT) -> *mut DsptaskT {
    unsafe { sys::assert_task(task) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helpers_exist() {
        let _ = init as fn();
        let _ = register_callback as fn(DspCallback) -> DspCallback;
        let _ = check_mail_to as fn() -> u32;
        let _ = check_mail_from as fn() -> u32;
        let _ = read_mail_from as fn() -> u32;
        let _ = send_mail_to as fn(u32);
        let _ = read_cpu_to_dsp as fn() -> u32;
        let _ = assert_int as fn();
        let _ = reset as fn();
        let _ = halt as fn();
        let _ = unhalt as fn();
        let _ = get_dma_status as fn() -> u32;
        let _ = add_task as unsafe fn(*mut DsptaskT) -> *mut DsptaskT;
        let _ = cancel_task as unsafe fn(*mut DsptaskT);
        let _ = assert_task as unsafe fn(*mut DsptaskT) -> *mut DsptaskT;
    }
}
