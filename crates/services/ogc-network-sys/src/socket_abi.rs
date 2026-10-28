//! Shared std network ABI; copied unchanged into the pinned std overlay.
use core::ffi::c_void;

#[repr(C)]
pub struct NetRequest {
    pub operation: u32,
    pub handle: *mut c_void,
    pub data: *mut u8,
    pub length: usize,
    pub ip: u32,
    pub port: u16,
    pub flags: u16,
    pub seconds: u64,
    pub nanos: u32,
}

impl NetRequest {
    pub fn new(operation: u32, handle: *mut c_void) -> Self {
        Self {
            operation,
            handle,
            data: core::ptr::null_mut(),
            length: 0,
            ip: 0,
            port: 0,
            flags: 0,
            seconds: 0,
            nanos: 0,
        }
    }
}

#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(core::mem::size_of::<NetRequest>() == 40);
    assert!(core::mem::offset_of!(NetRequest, seconds) == 24);
};
