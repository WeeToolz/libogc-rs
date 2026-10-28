#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub use ogc_usbstorage_sys as sys;
pub use sys::*;

pub use ogc_sys::{ErrorCode, Result};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Capacity {
    pub sector_size: u32,
    pub sectors: u32,
}

pub fn initialize() -> i32 {
    sys::initialize()
}

pub fn try_initialize() -> Result<()> {
    ogc_sys::unit_from_status(initialize())
}

pub fn deinitialize() -> i32 {
    sys::deinitialize()
}

pub fn try_deinitialize() -> Result<()> {
    ogc_sys::unit_from_status(deinitialize())
}

/// # Safety
///
/// `dev` must be null or a valid writable, closed handle. Do not reuse a handle
/// after an operation times out until `close` succeeds.
pub unsafe fn open(dev: *mut sys::UsbStorageHandle, device_id: i32, vid: u16, pid: u16) -> i32 {
    unsafe { sys::open(dev, device_id, vid, pid) }
}

/// # Safety
///
/// `dev` must be null or a valid writable, closed storage handle. Do not reuse
/// a handle after an operation times out until `close` succeeds.
pub unsafe fn try_open(
    dev: *mut sys::UsbStorageHandle,
    device_id: i32,
    vid: u16,
    pid: u16,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(open(dev, device_id, vid, pid)) }
}

/// # Safety
///
/// `dev` must be null or a valid writable handle. If close returns a timeout or
/// initialization error, keep the handle alive and retry after the pending
/// callback has drained.
pub unsafe fn close(dev: *mut sys::UsbStorageHandle) -> i32 {
    unsafe { sys::close(dev) }
}

/// Flushes a mounted LUN's volatile write cache before removal or shutdown.
///
/// # Safety
///
/// `dev` must point to a valid open storage handle and `lun` must be mounted.
pub unsafe fn flush(dev: *mut sys::UsbStorageHandle, lun: u8) -> i32 {
    unsafe { sys::flush(dev, lun) }
}

/// # Safety
///
/// `dev` must point to a valid open storage handle and `lun` must be mounted.
pub unsafe fn try_flush(dev: *mut sys::UsbStorageHandle, lun: u8) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(flush(dev, lun)) }
}

/// Flushes the storage device currently mounted through the global disc interface.
pub fn flush_default() -> i32 {
    sys::flush_default()
}

pub fn try_flush_default() -> Result<()> {
    ogc_sys::unit_from_status(flush_default())
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_close(dev: *mut sys::UsbStorageHandle) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(close(dev)) }
}

/// # Safety
///
/// `dev` must be null or a valid writable handle.
pub unsafe fn reset(dev: *mut sys::UsbStorageHandle) -> i32 {
    unsafe { sys::reset(dev) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_reset(dev: *mut sys::UsbStorageHandle) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(reset(dev)) }
}

/// # Safety
///
/// `dev` must be null or a valid writable handle.
pub unsafe fn get_max_lun(dev: *mut sys::UsbStorageHandle) -> i32 {
    unsafe { sys::get_max_lun(dev) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_get_max_lun(dev: *mut sys::UsbStorageHandle) -> Result<i32> {
    unsafe { ogc_sys::result_from_status(get_max_lun(dev)) }
}

/// # Safety
///
/// `dev` must be null or a valid writable handle.
pub unsafe fn mount_lun(dev: *mut sys::UsbStorageHandle, lun: u8) -> i32 {
    unsafe { sys::mount_lun(dev, lun) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_mount_lun(dev: *mut sys::UsbStorageHandle, lun: u8) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(mount_lun(dev, lun)) }
}

/// # Safety
///
/// `dev` must be null or a valid writable handle.
pub unsafe fn suspend(dev: *mut sys::UsbStorageHandle) -> i32 {
    unsafe { sys::suspend(dev) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_suspend(dev: *mut sys::UsbStorageHandle) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(suspend(dev)) }
}

pub fn is_dvd() -> i32 {
    sys::is_dvd()
}

pub fn try_is_dvd() -> Result<bool> {
    ogc_sys::bool_from_status(is_dvd())
}

/// # Safety
///
/// `dev` and buffers must satisfy the storage transfer contract.
pub unsafe fn read_capacity(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    unsafe { sys::read_capacity(dev, lun, sector_size, n_sectors) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_read_capacity(dev: *mut sys::UsbStorageHandle, lun: u8) -> Result<Capacity> {
    unsafe {
        let mut sector_size = 0;
        let mut sectors = 0;
        ogc_sys::unit_from_status(read_capacity(dev, lun, &mut sector_size, &mut sectors)).map(
            |()| Capacity {
                sector_size,
                sectors,
            },
        )
    }
}

/// # Safety
///
/// `dev` and `buffer` must satisfy the storage transfer contract.
pub unsafe fn read(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> i32 {
    unsafe { sys::read(dev, lun, sector, n_sectors, buffer) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle. `buffer` must be valid
/// for `n_sectors` sectors and satisfy the device transfer alignment contract.
pub unsafe fn try_read(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(read(dev, lun, sector, n_sectors, buffer)) }
}

/// # Safety
///
/// `dev` and `buffer` must satisfy the storage transfer contract.
pub unsafe fn write(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> i32 {
    unsafe { sys::write(dev, lun, sector, n_sectors, buffer) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle. `buffer` must be valid
/// for `n_sectors` sectors and satisfy the device transfer alignment contract.
pub unsafe fn try_write(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(write(dev, lun, sector, n_sectors, buffer)) }
}

/// # Safety
///
/// `dev` must be a valid writable handle.
pub unsafe fn start_stop(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    lo_ej: u8,
    start: u8,
    imm: u8,
) -> i32 {
    unsafe { sys::start_stop(dev, lun, lo_ej, start, imm) }
}

/// # Safety
///
/// `dev` must be null or a valid writable storage handle.
pub unsafe fn try_start_stop(
    dev: *mut sys::UsbStorageHandle,
    lun: u8,
    load_eject: u8,
    start: u8,
    immediate: u8,
) -> Result<()> {
    unsafe { ogc_sys::unit_from_status(start_stop(dev, lun, load_eject, start, immediate)) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn result_helpers_convert_storage_status() {
        assert_eq!(try_is_dvd(), Ok(false));

        let mut handle = UsbStorageHandle::default();
        unsafe {
            assert_eq!(try_initialize(), Err(ErrorCode::new(USBSTORAGE_EINIT)));
            assert_eq!(
                try_open(&mut handle, 0, 0x1234, 0x5678),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
            assert_eq!(
                try_get_max_lun(&mut handle),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
            assert_eq!(
                try_read_capacity(&mut handle, 0),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
            assert_eq!(
                try_mount_lun(&mut handle, 0),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
            assert_eq!(
                try_close(&mut handle),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
            assert_eq!(
                try_read_capacity(&mut handle, 0),
                Err(ErrorCode::new(USBSTORAGE_ENOINTERFACE))
            );
        }
    }
}
