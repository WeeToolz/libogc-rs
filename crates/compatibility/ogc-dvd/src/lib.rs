#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_dvd_sys as sys;
pub use sys::*;

pub fn init() {
    sys::init()
}

pub fn pause() {
    sys::pause()
}

pub fn reset(reset_mode: u32) {
    sys::reset(reset_mode)
}

pub fn mount() -> i32 {
    sys::mount()
}

pub fn get_drive_status() -> i32 {
    sys::get_drive_status()
}

/// # Safety
///
/// The caller must uphold the pointer validity, exclusivity, and asynchronous
/// lifetime requirements documented by [`sys::mount_async`].
pub unsafe fn mount_async(block: *mut sys::DvdCmdBlk, cb: sys::DvdCbCallback) -> i32 {
    unsafe { sys::mount_async(block, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::control_drive_async`].
pub unsafe fn control_drive_async(
    block: *mut sys::DvdCmdBlk,
    cmd: u32,
    cb: sys::DvdCbCallback,
) -> i32 {
    unsafe { sys::control_drive_async(block, cmd, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::control_drive`].
pub unsafe fn control_drive(block: *mut sys::DvdCmdBlk, cmd: u32) -> i32 {
    unsafe { sys::control_drive(block, cmd) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::set_gcm_offset_async`].
pub unsafe fn set_gcm_offset_async(
    block: *mut sys::DvdCmdBlk,
    offset: i64,
    cb: sys::DvdCbCallback,
) -> i32 {
    unsafe { sys::set_gcm_offset_async(block, offset, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::set_gcm_offset`].
pub unsafe fn set_gcm_offset(block: *mut sys::DvdCmdBlk, offset: i64) -> i32 {
    unsafe { sys::set_gcm_offset(block, offset) }
}

/// # Safety
///
/// The caller must uphold the validity and lifetime requirements documented by
/// [`sys::get_cmd_block_status`].
pub unsafe fn get_cmd_block_status(block: *mut sys::DvdCmdBlk) -> i32 {
    unsafe { sys::get_cmd_block_status(block) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::spin_up_drive_async`].
pub unsafe fn spin_up_drive_async(block: *mut sys::DvdCmdBlk, cb: sys::DvdCbCallback) -> i32 {
    unsafe { sys::spin_up_drive_async(block, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::spin_up_drive`].
pub unsafe fn spin_up_drive(block: *mut sys::DvdCmdBlk) -> i32 {
    unsafe { sys::spin_up_drive(block) }
}

/// # Safety
///
/// The caller must uphold the pointer and asynchronous lifetime requirements
/// documented by [`sys::inquiry_async`].
pub unsafe fn inquiry_async(
    block: *mut sys::DvdCmdBlk,
    info: *mut sys::DvdDrvInfo,
    cb: sys::DvdCbCallback,
) -> i32 {
    unsafe { sys::inquiry_async(block, info, cb) }
}

/// # Safety
///
/// The caller must uphold the pointer and non-aliasing requirements documented
/// by [`sys::inquiry`].
pub unsafe fn inquiry(block: *mut sys::DvdCmdBlk, info: *mut sys::DvdDrvInfo) -> i32 {
    unsafe { sys::inquiry(block, info) }
}

/// # Safety
///
/// The caller must uphold the pointer and asynchronous lifetime requirements
/// documented by [`sys::read_disk_id`].
pub unsafe fn read_disk_id(
    block: *mut sys::DvdCmdBlk,
    id: *mut sys::DvdDiskId,
    cb: sys::DvdCbCallback,
) -> i32 {
    unsafe { sys::read_disk_id(block, id, cb) }
}

/// # Safety
///
/// The caller must uphold the pointer validity, 32-byte buffer alignment,
/// non-aliasing, and lifetime requirements documented by
/// [`sys::read_abs_async_prio`].
pub unsafe fn read_abs_async_prio(
    block: *mut sys::DvdCmdBlk,
    buf: *mut core::ffi::c_void,
    len: u32,
    offset: i64,
    cb: sys::DvdCbCallback,
    prio: i32,
) -> i32 {
    unsafe { sys::read_abs_async_prio(block, buf, len, offset, cb, prio) }
}

/// # Safety
///
/// The caller must uphold the requirements documented by
/// [`sys::read_abs_async_for_bs`].
pub unsafe fn read_abs_async_for_bs(
    block: *mut sys::DvdCmdBlk,
    buf: *mut core::ffi::c_void,
    len: u32,
    offset: i64,
    cb: sys::DvdCbCallback,
) -> i32 {
    unsafe { sys::read_abs_async_for_bs(block, buf, len, offset, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::seek_abs_async_prio`].
pub unsafe fn seek_abs_async_prio(
    block: *mut sys::DvdCmdBlk,
    offset: i64,
    cb: sys::DvdCbCallback,
    prio: i32,
) -> i32 {
    unsafe { sys::seek_abs_async_prio(block, offset, cb, prio) }
}

/// # Safety
///
/// The caller must uphold the command-block and output-buffer requirements
/// documented by [`sys::read_prio`].
pub unsafe fn read_prio(
    block: *mut sys::DvdCmdBlk,
    buf: *mut core::ffi::c_void,
    len: u32,
    offset: i64,
    prio: i32,
) -> i32 {
    unsafe { sys::read_prio(block, buf, len, offset, prio) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::seek_prio`].
pub unsafe fn seek_prio(block: *mut sys::DvdCmdBlk, offset: i64, prio: i32) -> i32 {
    unsafe { sys::seek_prio(block, offset, prio) }
}

pub fn cancel_all_async(cb: sys::DvdCbCallback) -> i32 {
    sys::cancel_all_async(cb)
}

/// # Safety
///
/// The caller must uphold the asynchronous command-block requirements
/// documented by [`sys::stop_stream_at_end_async`].
pub unsafe fn stop_stream_at_end_async(block: *mut sys::DvdCmdBlk, cb: sys::DvdCbCallback) -> i32 {
    unsafe { sys::stop_stream_at_end_async(block, cb) }
}

/// # Safety
///
/// The caller must uphold the command-block requirements documented by
/// [`sys::stop_stream_at_end`].
pub unsafe fn stop_stream_at_end(block: *mut sys::DvdCmdBlk) -> i32 {
    unsafe { sys::stop_stream_at_end(block) }
}

pub fn set_auto_invalidation(auto_inv: u32) -> u32 {
    sys::set_auto_invalidation(auto_inv)
}

pub fn get_current_disk_id() -> *mut sys::DvdDiskId {
    sys::get_current_disk_id()
}

pub fn get_drive_info() -> *mut sys::DvdDrvInfo {
    sys::get_drive_info()
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helpers_exist() {
        let _ = init as fn();
        let _ = pause as fn();
        let _ = reset as fn(u32);
        let _ = mount as fn() -> i32;
        let _ = get_drive_status as fn() -> i32;
        let _ = mount_async as unsafe fn(*mut sys::DvdCmdBlk, sys::DvdCbCallback) -> i32;
        let _ =
            control_drive_async as unsafe fn(*mut sys::DvdCmdBlk, u32, sys::DvdCbCallback) -> i32;
        let _ = control_drive as unsafe fn(*mut sys::DvdCmdBlk, u32) -> i32;
        let _ =
            set_gcm_offset_async as unsafe fn(*mut sys::DvdCmdBlk, i64, sys::DvdCbCallback) -> i32;
        let _ = set_gcm_offset as unsafe fn(*mut sys::DvdCmdBlk, i64) -> i32;
        let _ = get_cmd_block_status as unsafe fn(*mut sys::DvdCmdBlk) -> i32;
        let _ = spin_up_drive_async as unsafe fn(*mut sys::DvdCmdBlk, sys::DvdCbCallback) -> i32;
        let _ = spin_up_drive as unsafe fn(*mut sys::DvdCmdBlk) -> i32;
        let _ = inquiry_async
            as unsafe fn(*mut sys::DvdCmdBlk, *mut sys::DvdDrvInfo, sys::DvdCbCallback) -> i32;
        let _ = inquiry as unsafe fn(*mut sys::DvdCmdBlk, *mut sys::DvdDrvInfo) -> i32;
        let _ = read_disk_id
            as unsafe fn(*mut sys::DvdCmdBlk, *mut sys::DvdDiskId, sys::DvdCbCallback) -> i32;
        let _ = read_abs_async_prio
            as unsafe fn(
                *mut sys::DvdCmdBlk,
                *mut core::ffi::c_void,
                u32,
                i64,
                sys::DvdCbCallback,
                i32,
            ) -> i32;
        let _ = read_abs_async_for_bs
            as unsafe fn(
                *mut sys::DvdCmdBlk,
                *mut core::ffi::c_void,
                u32,
                i64,
                sys::DvdCbCallback,
            ) -> i32;
        let _ = seek_abs_async_prio
            as unsafe fn(*mut sys::DvdCmdBlk, i64, sys::DvdCbCallback, i32) -> i32;
        let _ = read_prio
            as unsafe fn(*mut sys::DvdCmdBlk, *mut core::ffi::c_void, u32, i64, i32) -> i32;
        let _ = seek_prio as unsafe fn(*mut sys::DvdCmdBlk, i64, i32) -> i32;
        let _ = cancel_all_async as fn(sys::DvdCbCallback) -> i32;
        let _ =
            stop_stream_at_end_async as unsafe fn(*mut sys::DvdCmdBlk, sys::DvdCbCallback) -> i32;
        let _ = stop_stream_at_end as unsafe fn(*mut sys::DvdCmdBlk) -> i32;
        let _ = set_auto_invalidation as fn(u32) -> u32;
        let _ = get_current_disk_id as fn() -> *mut sys::DvdDiskId;
        let _ = get_drive_info as fn() -> *mut sys::DvdDrvInfo;
    }
}
