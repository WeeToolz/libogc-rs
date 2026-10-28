#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_sys::Bool;

pub const ISO_MAXPATHLEN: usize = 128;

#[repr(C)]
pub struct DiscInterface {
    pub io_type: u32,
    pub features: u32,
    pub startup: Option<unsafe extern "C" fn() -> bool>,
    pub is_inserted: Option<unsafe extern "C" fn() -> bool>,
    pub read_sectors: Option<
        unsafe extern "C" fn(sector: u32, num_sectors: u32, buffer: *mut core::ffi::c_void) -> bool,
    >,
    pub write_sectors: Option<
        unsafe extern "C" fn(
            sector: u32,
            num_sectors: u32,
            buffer: *const core::ffi::c_void,
        ) -> bool,
    >,
    pub clear_status: Option<unsafe extern "C" fn() -> bool>,
    pub shutdown: Option<unsafe extern "C" fn() -> bool>,
}
