#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_exi_sys as sys;
pub use sys::*;

pub use ogc_sys::{ErrorCode, Result};

pub fn init() {
    sys::init()
}

pub fn initialize() {
    init();
}

pub fn probe(chn: i32) -> i32 {
    sys::probe(chn)
}

pub fn try_probe(channel: i32) -> Result<bool> {
    ogc_sys::bool_from_status(probe(channel))
}

pub fn probe_ex(chn: i32) -> i32 {
    sys::probe_ex(chn)
}

pub fn try_probe_ex(channel: i32) -> Result<bool> {
    ogc_sys::bool_from_status(probe_ex(channel))
}

/// # Safety
///
/// `unlock_cb` must remain valid while the lock is held.
pub unsafe fn lock(chn: i32, dev: i32, unlock_cb: ExiCallback) -> i32 {
    sys::lock(chn, dev, unlock_cb)
}

/// # Safety
///
/// `unlock_callback` must remain valid while the lock is held.
pub unsafe fn try_lock(channel: i32, device: i32, unlock_callback: ExiCallback) -> Result<()> {
    unsafe { ogc_sys::unit_from_bool_status(lock(channel, device, unlock_callback)) }
}

pub fn unlock(chn: i32) -> i32 {
    sys::unlock(chn)
}

pub fn try_unlock(channel: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(unlock(channel))
}

pub fn select(chn: i32, dev: i32, frq: i32) -> i32 {
    sys::select(chn, dev, frq)
}

pub fn try_select(channel: i32, device: i32, frequency: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(select(channel, device, frequency))
}

pub fn select_sd(chn: i32, dev: i32, frq: i32) -> i32 {
    sys::select_sd(chn, dev, frq)
}

pub fn try_select_sd(channel: i32, device: i32, frequency: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(select_sd(channel, device, frequency))
}

pub fn deselect(chn: i32) -> i32 {
    sys::deselect(chn)
}

pub fn try_deselect(channel: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(deselect(channel))
}

pub fn sync(chn: i32) -> i32 {
    sys::sync(chn)
}

pub fn try_sync(channel: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(sync(channel))
}

/// # Safety
///
/// `p_data` must be valid for `n_len` bytes and remain live for the transfer.
pub unsafe fn imm(
    chn: i32,
    p_data: *mut c_void,
    n_len: u32,
    n_mode: i32,
    tc_cb: ExiCallback,
) -> i32 {
    unsafe { sys::imm(chn, p_data, n_len, n_mode, tc_cb) }
}

/// # Safety
///
/// `data` must be valid for `len` bytes and remain live for the transfer.
/// `callback`, when present, must remain valid until completion.
pub unsafe fn try_imm(
    channel: i32,
    data: *mut c_void,
    len: u32,
    mode: i32,
    callback: ExiCallback,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_bool_status(imm(channel, data, len, mode, callback)) }
}

/// # Safety
///
/// `p_data` must be valid for `n_len` bytes and remain live for the transfer.
pub unsafe fn imm_ex(chn: i32, p_data: *mut c_void, n_len: u32, n_mode: i32) -> i32 {
    unsafe { sys::imm_ex(chn, p_data, n_len, n_mode) }
}

/// # Safety
///
/// `data` must be valid for `len` bytes and remain live for the transfer.
pub unsafe fn try_imm_ex(channel: i32, data: *mut c_void, len: u32, mode: i32) -> Result<()> {
    unsafe { ogc_sys::unit_from_bool_status(imm_ex(channel, data, len, mode)) }
}

/// # Safety
///
/// `p_data` must be valid for `n_len` bytes and remain live for the transfer.
pub unsafe fn dma(
    chn: i32,
    p_data: *mut c_void,
    n_len: u32,
    n_mode: i32,
    tc_cb: ExiCallback,
) -> i32 {
    unsafe { sys::dma(chn, p_data, n_len, n_mode, tc_cb) }
}

/// # Safety
///
/// `data` must satisfy the EXI DMA alignment contract for `len` bytes and remain
/// live until completion. `callback`, when present, must remain valid until then.
pub unsafe fn try_dma(
    channel: i32,
    data: *mut c_void,
    len: u32,
    mode: i32,
    callback: ExiCallback,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_bool_status(dma(channel, data, len, mode, callback)) }
}

pub fn get_state(chn: i32) -> i32 {
    sys::get_state(chn)
}

/// # Safety
///
/// `n_id` must point to writable memory for one `u32`.
pub unsafe fn get_id(chn: i32, dev: i32, n_id: *mut u32) -> i32 {
    unsafe { sys::get_id(chn, dev, n_id) }
}

pub fn read_id(channel: i32, device: i32) -> Result<u32> {
    let mut id = 0;
    ogc_sys::unit_from_bool_status(unsafe { get_id(channel, device, &mut id) }).map(|()| id)
}

pub fn attach(chn: i32, ext_cb: ExiCallback) -> i32 {
    sys::attach(chn, ext_cb)
}

pub fn try_attach(channel: i32, callback: ExiCallback) -> Result<()> {
    ogc_sys::unit_from_bool_status(attach(channel, callback))
}

pub fn detach(chn: i32) -> i32 {
    sys::detach(chn)
}

pub fn try_detach(channel: i32) -> Result<()> {
    ogc_sys::unit_from_bool_status(detach(channel))
}

pub fn probe_reset() {
    sys::probe_reset()
}

pub fn register_exi_callback(chn: i32, exi_cb: ExiCallback) -> ExiCallback {
    sys::register_exi_callback(chn, exi_cb)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helpers_exist() {
        let _ = init as fn();
        let _ = probe as fn(i32) -> i32;
        let _ = probe_ex as fn(i32) -> i32;
        let _ = lock as unsafe fn(i32, i32, ExiCallback) -> i32;
        let _ = unlock as fn(i32) -> i32;
        let _ = select as fn(i32, i32, i32) -> i32;
        let _ = select_sd as fn(i32, i32, i32) -> i32;
        let _ = deselect as fn(i32) -> i32;
        let _ = sync as fn(i32) -> i32;
        let _ = imm as unsafe fn(i32, *mut c_void, u32, i32, ExiCallback) -> i32;
        let _ = imm_ex as unsafe fn(i32, *mut c_void, u32, i32) -> i32;
        let _ = dma as unsafe fn(i32, *mut c_void, u32, i32, ExiCallback) -> i32;
        let _ = get_state as fn(i32) -> i32;
        let _ = get_id as unsafe fn(i32, i32, *mut u32) -> i32;
        let _ = attach as fn(i32, ExiCallback) -> i32;
        let _ = detach as fn(i32) -> i32;
        let _ = probe_reset as fn();
        let _ = register_exi_callback as fn(i32, ExiCallback) -> ExiCallback;
    }

    #[test]
    fn result_helpers_preserve_raw_contract() {
        init();
        assert_eq!(try_probe(EXI_CHANNEL_MAX), Ok(false));
        assert_eq!(try_select(EXI_CHANNEL_MAX, 0, 0), Err(ErrorCode::new(-1)));
        assert_eq!(try_deselect(EXI_CHANNEL_MAX), Err(ErrorCode::new(-1)));
        assert_eq!(try_sync(EXI_CHANNEL_MAX), Err(ErrorCode::new(-1)));
        assert_eq!(read_id(EXI_CHANNEL_MAX, 0), Err(ErrorCode::new(-1)));
    }
}
