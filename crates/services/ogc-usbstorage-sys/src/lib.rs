#![no_std]

#[cfg(target_arch = "powerpc")]
extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32};

pub use ogc_filesystem_sys::DiscInterface;
pub use ogc_sys::Bool;

#[cfg(target_arch = "powerpc")]
mod wii_backend;

mod startup;
pub use startup::{ProbeStage, UsbProbeStatus, probe_status};

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.USBStorage_ioctl,"ax",@progbits
    .balign 4
    .globl USBStorage_ioctl
    .type USBStorage_ioctl,@function
USBStorage_ioctl:
    b __ogc_usbstorage_ioctl_bridge
    .size USBStorage_ioctl,.-USBStorage_ioctl
"#
);

pub const USBSTORAGE_OK: i32 = 0;
pub const USBSTORAGE_ENOINTERFACE: i32 = -10000;
pub const USBSTORAGE_ESENSE: i32 = -10001;
pub const USBSTORAGE_ESHORTWRITE: i32 = -10002;
pub const USBSTORAGE_ESHORTREAD: i32 = -10003;
pub const USBSTORAGE_ESIGNATURE: i32 = -10004;
pub const USBSTORAGE_ETAG: i32 = -10005;
pub const USBSTORAGE_ESTATUS: i32 = -10006;
pub const USBSTORAGE_EDATARESIDUE: i32 = -10007;
pub const USBSTORAGE_ETIMEDOUT: i32 = -10008;
pub const USBSTORAGE_EINIT: i32 = -10009;
pub const USBSTORAGE_PROCESSING: i32 = ogc_usbstorage_lifecycle::bulk::PROCESSING;

#[cfg(target_arch = "powerpc")]
static LAST_DEINITIALIZE_STATUS: AtomicI32 = AtomicI32::new(USBSTORAGE_OK);

/// Returns the result of the latest call to the C `USBStorage_Deinitialize`
/// entry point. C callers can use this because the compatible deinitialize
/// entry point itself returns `void`.
#[cfg(target_arch = "powerpc")]
pub fn last_deinitialize_status() -> i32 {
    ogc_usbstorage_lifecycle::deinitialize::latest_status(&LAST_DEINITIALIZE_STATUS)
}

/// Returns whether libogc would remount a LUN before I/O after a long idle period.
#[doc(hidden)]
pub const fn idle_remount_required(now_secs: u32, last_used_secs: u32) -> bool {
    now_secs.wrapping_sub(last_used_secs) > 60
}

/// Applies libogc's retry stop rule while preserving the failed BOT result.
#[doc(hidden)]
pub const fn bot_cycle_retry_error(attempt_result: i32, reset_result: i32) -> Option<i32> {
    if attempt_result == USBSTORAGE_ETIMEDOUT || reset_result == USBSTORAGE_ETIMEDOUT {
        Some(USBSTORAGE_ETIMEDOUT)
    } else {
        None
    }
}

/// Copies bytes returned by a short read before reporting the C-compatible error.
#[doc(hidden)]
pub fn copy_read_result(source: &[u8], destination: &mut [u8], received: i32) -> i32 {
    if received > 0 {
        let count = (received as usize).min(source.len()).min(destination.len());
        destination[..count].copy_from_slice(&source[..count]);
    }
    transfer_result(received, destination.len())
}

/// Maps a completed bulk transfer to the Rust result for one data chunk.
///
/// Negative transport errors stay visible to the caller. This follows the
/// repository error-propagation policy; C maps non-timeout errors to residue.
#[doc(hidden)]
pub const fn transfer_result(received: i32, expected: usize) -> i32 {
    if received == expected as i32 {
        USBSTORAGE_OK
    } else if received == USBSTORAGE_ETIMEDOUT || received < 0 {
        received
    } else {
        USBSTORAGE_EDATARESIDUE
    }
}

/// Converts the USB GetMaxLUN response into the number of LUNs.
///
/// The response is the highest LUN index, so a value of 254 represents 255
/// LUNs. A value of 255 would require 256 LUNs, which the C ABI's `u8`
/// `max_lun` count cannot represent.
#[doc(hidden)]
pub const fn lun_count_from_reported_max(reported_max: u8) -> Option<u8> {
    reported_max.checked_add(1)
}

/// Applies C's GetMaxLUN error policy and converts a successful highest LUN
/// index to a count. A timeout aborts open; other request errors use LUN 0.
#[doc(hidden)]
pub const fn lun_count_after_get_max_lun(result: Result<u8, i32>) -> Result<u8, i32> {
    match result {
        Ok(reported_max) => match lun_count_from_reported_max(reported_max) {
            Some(count) => Ok(count),
            None => Err(USBSTORAGE_ENOINTERFACE),
        },
        Err(USBSTORAGE_ETIMEDOUT) => Err(USBSTORAGE_ETIMEDOUT),
        Err(_) => Ok(1),
    }
}

/// Result of decoding a successful SCSI READ CAPACITY cycle.
#[doc(hidden)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ReadCapacityResult {
    pub result: i32,
    pub sector_size: u32,
    pub sectors: u32,
}

/// Applies libogc's READ CAPACITY result handling to one returned response.
///
/// C uses the BOT cycle result and ignores CSW status for this command.
#[doc(hidden)]
pub fn decode_read_capacity(
    cycle_result: i32,
    _csw_status: u8,
    response: [u8; 8],
) -> ReadCapacityResult {
    if cycle_result < 0 {
        return ReadCapacityResult {
            result: cycle_result,
            sector_size: 0,
            sectors: 0,
        };
    }

    let last_lba = u32::from_be_bytes([response[0], response[1], response[2], response[3]]);
    let sector_size = u32::from_be_bytes([response[4], response[5], response[6], response[7]]);
    ReadCapacityResult {
        result: USBSTORAGE_OK,
        sector_size,
        sectors: last_lba,
    }
}

/// Preserves a BOT cycle result regardless of CSW residue.
///
/// The C USB-storage call sites pass a null residue output pointer and do not
/// convert a nonzero CSW residue into a cycle error.
#[doc(hidden)]
pub fn bot_csw_cycle_result(cycle_result: i32, _residue: u32) -> i32 {
    cycle_result
}

/// Applies libogc's READ/WRITE CSW-status rule.
///
/// C reports `USBSTORAGE_ESTATUS` only when the BOT cycle result is positive
/// and the CSW status is nonzero. A zero cycle result is returned unchanged.
#[doc(hidden)]
pub fn read_write_cycle_result(cycle_result: i32, csw_status: u8) -> i32 {
    if cycle_result > 0 && csw_status != 0 {
        USBSTORAGE_ESTATUS
    } else {
        cycle_result
    }
}

/// Returns whether C's USB-mode check requires a BOT reset during open.
#[doc(hidden)]
pub fn open_requires_bot_reset(device_fd: i32, bulk_out_max_packet_size: u16) -> bool {
    !(bulk_out_max_packet_size > 64 && !(-1..0x20).contains(&device_fd))
}

pub const USBSTORAGE_EINVAL: i32 = -4;
pub const USBSTORAGE_INVALID_LUN: i32 = -2;

pub const FEATURE_MEDIUM_CANREAD: u32 = 0x0000_0001;
pub const FEATURE_MEDIUM_CANWRITE: u32 = 0x0000_0002;
pub const FEATURE_WII_USB: u32 = 0x0000_0200;

pub const DEVICE_TYPE_WII_USB: u32 =
    ((b'W' as u32) << 24) | ((b'U' as u32) << 16) | ((b'S' as u32) << 8) | (b'B' as u32);

pub const B_RAW_DEVICE_DATA_IN: u8 = 0x01;
pub const B_RAW_DEVICE_COMMAND: u8 = 0x00;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct UsbStorageHandle {
    pub configuration: u8,
    pub interface: u32,
    pub alt_interface: u32,
    pub b_interface_sub_class: u8,
    pub ep_in: u8,
    pub ep_out: u8,
    pub max_lun: u8,
    pub sector_size: *mut u32,
    pub usb_fd: i32,
    pub lock: u32,
    pub alarm: u32,
    pub retval: i32,
    pub tag: u32,
    pub suspended: u8,
    pub buffer: *mut u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct RawDeviceCommand {
    pub command: [u8; 16],
    pub command_length: u8,
    pub flags: u8,
    pub scsi_status: u8,
    pub data: *mut core::ffi::c_void,
    pub data_length: usize,
}

#[cfg(target_arch = "powerpc")]
static USBSTORAGE_INITED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_MOUNTED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_CAPACITY_VALID: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_LAST_SECTOR_SIZE: AtomicU32 = AtomicU32::new(512);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_LAST_NUM_SECTORS: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_LAST_LUN: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_LAST_DEVICE_ID: AtomicI32 = AtomicI32::new(-1);
#[cfg(target_arch = "powerpc")]
static USBSTORAGE_LAST_RAW_WRITE: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
pub fn initialize() -> i32 {
    wii_backend::initialize()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn initialize() -> i32 {
    USBSTORAGE_EINIT
}

#[cfg(target_arch = "powerpc")]
pub fn deinitialize() -> i32 {
    wii_backend::deinitialize()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn deinitialize() -> i32 {
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable, closed `UsbStorageHandle`. Do not
/// reopen a handle after an operation times out until `close` succeeds.
#[cfg(target_arch = "powerpc")]
pub unsafe fn open(dev: *mut UsbStorageHandle, device_id: i32, vid: u16, pid: u16) -> i32 {
    wii_backend::open(dev, device_id, vid, pid)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn open(dev: *mut UsbStorageHandle, device_id: i32, vid: u16, pid: u16) -> i32 {
    let _ = (vid, pid);
    let _ = (dev, device_id);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable handle. If close returns
/// `USBSTORAGE_ETIMEDOUT` or `USBSTORAGE_EINIT`, keep the handle alive and retry
/// after the pending callback has drained.
#[cfg(target_arch = "powerpc")]
pub unsafe fn close(dev: *mut UsbStorageHandle) -> i32 {
    wii_backend::close(dev)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn close(dev: *mut UsbStorageHandle) -> i32 {
    let _ = dev;
    USBSTORAGE_ENOINTERFACE
}

/// Flushes a mounted LUN's volatile cache before removal or shutdown.
///
/// # Safety
///
/// `dev` must point to a live handle exclusively borrowed for this call.
#[cfg(target_arch = "powerpc")]
pub unsafe fn flush(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { wii_backend::flush(dev, lun) }
}

#[cfg(not(target_arch = "powerpc"))]
pub fn flush(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    let _ = (dev, lun);
    USBSTORAGE_ENOINTERFACE
}

/// Flushes the LUN currently owned by [`USB_STORAGE_INTERFACE`].
#[cfg(target_arch = "powerpc")]
pub fn flush_default() -> i32 {
    wii_backend::flush_default()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn flush_default() -> i32 {
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn reset(dev: *mut UsbStorageHandle) -> i32 {
    wii_backend::reset(dev)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn reset(dev: *mut UsbStorageHandle) -> i32 {
    let _ = dev;
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_max_lun(dev: *mut UsbStorageHandle) -> i32 {
    wii_backend::get_max_lun(dev)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn get_max_lun(dev: *mut UsbStorageHandle) -> i32 {
    let _ = dev;
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn mount_lun(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    wii_backend::mount_lun(dev, lun)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn mount_lun(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    let _ = (dev, lun);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn suspend(dev: *mut UsbStorageHandle) -> i32 {
    wii_backend::suspend(dev)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn suspend(dev: *mut UsbStorageHandle) -> i32 {
    let _ = dev;
    USBSTORAGE_ENOINTERFACE
}

#[cfg(target_arch = "powerpc")]
pub fn is_dvd() -> i32 {
    wii_backend::is_dvd()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn is_dvd() -> i32 {
    0
}

/// # Safety
///
/// `rdc` must be null or a valid writable `RawDeviceCommand`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn ioctl(request: i32, rdc: *mut RawDeviceCommand) -> i32 {
    wii_backend::ioctl(request, rdc)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn ioctl(request: i32, rdc: *mut RawDeviceCommand) -> i32 {
    let _ = (request, rdc);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_capacity(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    wii_backend::read_capacity(dev, lun, sector_size, n_sectors)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn read_capacity(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    let _ = (dev, lun, sector_size, n_sectors);
    USBSTORAGE_ENOINTERFACE
}

/// Runs the SCSI INQUIRY command for one LUN and returns the peripheral-device type.
///
/// This matches libogc's exported `USBStorage_Inquiry` implementation symbol,
/// which exists even though it is not declared in the public C header.
///
/// # Safety
///
/// `dev` must be null or a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn inquiry(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { wii_backend::inquiry(dev, lun) }
}

#[cfg(not(target_arch = "powerpc"))]
pub fn inquiry(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    let _ = (dev, lun);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` and `buffer` must be valid for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> i32 {
    wii_backend::read(dev, lun, sector, n_sectors, buffer)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn read(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> i32 {
    let _ = (dev, lun, sector, n_sectors, buffer);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` and `buffer` must be valid for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> i32 {
    wii_backend::write(dev, lun, sector, n_sectors, buffer)
}

#[cfg(not(target_arch = "powerpc"))]
pub fn write(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> i32 {
    let _ = (dev, lun, sector, n_sectors, buffer);
    USBSTORAGE_ENOINTERFACE
}

/// # Safety
///
/// `dev` must be a valid writable `UsbStorageHandle`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn start_stop(
    dev: *mut UsbStorageHandle,
    lun: u8,
    lo_ej: u8,
    start: u8,
    imm: u8,
) -> i32 {
    wii_backend::start_stop(dev, lun, lo_ej, start, imm)
}

/// Returns the number of timed-out USB control buffers retained because LWP
/// could not schedule their deferred cleanup.
#[doc(hidden)]
#[cfg(target_arch = "powerpc")]
pub fn async_control_cleanup_failures() -> u32 {
    wii_backend::async_control_cleanup_failures()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn start_stop(dev: *mut UsbStorageHandle, lun: u8, lo_ej: u8, start: u8, imm: u8) -> i32 {
    let _ = (dev, lun, lo_ej, start, imm);
    USBSTORAGE_ENOINTERFACE
}

#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Initialize")]
pub extern "C" fn c_initialize() -> i32 {
    initialize()
}

#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Deinitialize")]
pub extern "C" fn c_deinitialize() {
    ogc_usbstorage_lifecycle::deinitialize::record_status(
        &LAST_DEINITIALIZE_STATUS,
        deinitialize(),
    );
}

/// Returns the result of the latest `USBStorage_Deinitialize` call.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_GetLastDeinitializeStatus")]
pub extern "C" fn c_last_deinitialize_status() -> i32 {
    last_deinitialize_status()
}

/// # Safety
/// `dev` must be a valid writable closed handle. The caller must not reopen or
/// release it while a timed-out request still has a callback pending.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Open")]
pub unsafe extern "C" fn c_open(
    dev: *mut UsbStorageHandle,
    device_id: i32,
    vid: u16,
    pid: u16,
) -> i32 {
    unsafe { open(dev, device_id, vid, pid) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. If this returns `USBSTORAGE_ETIMEDOUT` or
/// `USBSTORAGE_EINIT`, keep `dev` alive and retry after the pending callback
/// has drained.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Close")]
pub unsafe extern "C" fn c_close(dev: *mut UsbStorageHandle) -> i32 {
    unsafe { close(dev) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Reset")]
pub unsafe extern "C" fn c_reset(dev: *mut UsbStorageHandle) -> i32 {
    unsafe { reset(dev) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_GetMaxLUN")]
pub unsafe extern "C" fn c_get_max_lun(dev: *mut UsbStorageHandle) -> i32 {
    unsafe { get_max_lun(dev) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_MountLUN")]
pub unsafe extern "C" fn c_mount_lun(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { mount_lun(dev, lun) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Suspend")]
pub unsafe extern "C" fn c_suspend(dev: *mut UsbStorageHandle) -> i32 {
    unsafe { suspend(dev) }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_IsDVD")]
pub extern "C" fn c_is_dvd() -> i32 {
    is_dvd()
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __ogc_usbstorage_ioctl_bridge(
    request: i32,
    rdc: *mut RawDeviceCommand,
) -> i32 {
    unsafe { ioctl(request, rdc) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_ReadCapacity")]
pub unsafe extern "C" fn c_read_capacity(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    unsafe { read_capacity(dev, lun, sector_size, n_sectors) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Inquiry")]
pub unsafe extern "C" fn c_inquiry(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { inquiry(dev, lun) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Read")]
pub unsafe extern "C" fn c_read(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> i32 {
    unsafe { read(dev, lun, sector, n_sectors, buffer) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_Write")]
pub unsafe extern "C" fn c_write(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> i32 {
    unsafe { write(dev, lun, sector, n_sectors, buffer) }
}

/// # Safety
/// The caller must meet the pointer, buffer, and exclusive handle requirements
/// of the corresponding Rust API. Handles and buffers must stay valid until
/// the synchronous operation completes.
#[cfg(target_arch = "powerpc")]
#[unsafe(export_name = "USBStorage_StartStop")]
pub unsafe extern "C" fn c_start_stop(
    dev: *mut UsbStorageHandle,
    lun: u8,
    lo_ej: u8,
    start: u8,
    imm: u8,
) -> i32 {
    unsafe { start_stop(dev, lun, lo_ej, start, imm) }
}

/// First and second independent drives in IOS discovery order. The slots do
/// not identify fixed physical USB ports.
#[unsafe(export_name = "__io_usbstorage")]
pub static USB_STORAGE_INTERFACE: DiscInterface = disc_interface::<0>();
#[unsafe(export_name = "__io_usbstorage2")]
pub static USB_STORAGE2_INTERFACE: DiscInterface = disc_interface::<1>();

const fn disc_interface<const SLOT: usize>() -> DiscInterface {
    DiscInterface {
        io_type: DEVICE_TYPE_WII_USB,
        features: FEATURE_MEDIUM_CANREAD | FEATURE_MEDIUM_CANWRITE | FEATURE_WII_USB,
        startup: Some(usbstorage_startup),
        is_inserted: Some(disc_is_inserted::<SLOT>),
        read_sectors: Some(disc_read_sectors::<SLOT>),
        write_sectors: Some(disc_write_sectors::<SLOT>),
        clear_status: Some(usbstorage_clear_status),
        shutdown: Some(disc_shutdown::<SLOT>),
    }
}

/// Flush the independent drive selected by its zero-based discovery slot.
pub fn flush_drive(slot: usize) -> i32 {
    wii_backend::flush_drive(slot)
}

unsafe extern "C" fn disc_is_inserted<const SLOT: usize>() -> bool {
    unsafe { wii_backend::disc_is_inserted(SLOT) }
}
unsafe extern "C" fn disc_read_sectors<const SLOT: usize>(
    sector: u32,
    count: u32,
    buffer: *mut core::ffi::c_void,
) -> bool {
    unsafe { wii_backend::disc_read_sectors(SLOT, sector, count, buffer) }
}
unsafe extern "C" fn disc_write_sectors<const SLOT: usize>(
    sector: u32,
    count: u32,
    buffer: *const core::ffi::c_void,
) -> bool {
    unsafe { wii_backend::disc_write_sectors(SLOT, sector, count, buffer) }
}
unsafe extern "C" fn disc_shutdown<const SLOT: usize>() -> bool {
    unsafe { wii_backend::disc_shutdown(SLOT) }
}

#[unsafe(export_name = "__io_usbstorage_sector_size")]
pub static USB_STORAGE_SECTOR_SIZE: u32 = 512;

unsafe extern "C" fn usbstorage_startup() -> bool {
    // Match C's USB_Initialize startup. Device selection still requests only
    // mass-storage interfaces, so HID devices never enter storage setup.
    startup::start_storage(ogc_usb_sys::initialize, initialize)
}

unsafe extern "C" fn usbstorage_clear_status() -> bool {
    unsafe { wii_backend::clear_status() }
}

#[cfg(all(test, not(target_arch = "powerpc")))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn exports_exist() {
        let _ = initialize as fn() -> i32;
        let _ = deinitialize as fn();
        let _ = open as unsafe fn(*mut UsbStorageHandle, i32, u16, u16) -> i32;
        let _ = close as unsafe fn(*mut UsbStorageHandle) -> i32;
        let _ = reset as unsafe fn(*mut UsbStorageHandle) -> i32;
        let _ = get_max_lun as unsafe fn(*mut UsbStorageHandle) -> i32;
        let _ = mount_lun as unsafe fn(*mut UsbStorageHandle, u8) -> i32;
        let _ = suspend as unsafe fn(*mut UsbStorageHandle) -> i32;
        let _ = is_dvd as fn() -> i32;
        let _ = ioctl as unsafe fn(i32, *mut RawDeviceCommand) -> i32;
        let _ = read_capacity as unsafe fn(*mut UsbStorageHandle, u8, *mut u32, *mut u32) -> i32;
        let _ = read as unsafe fn(*mut UsbStorageHandle, u8, u32, u16, *mut u8) -> i32;
        let _ = write as unsafe fn(*mut UsbStorageHandle, u8, u32, u16, *const u8) -> i32;
        let _ = start_stop as unsafe fn(*mut UsbStorageHandle, u8, u8, u8, u8) -> i32;
    }

    #[test]
    fn handle_and_command_layouts_are_c_compatible() {
        assert!(size_of::<UsbStorageHandle>() >= 48);
        assert_eq!(align_of::<UsbStorageHandle>(), align_of::<*mut u8>());
        assert_eq!(offset_of!(UsbStorageHandle, configuration), 0);
        assert!(
            offset_of!(UsbStorageHandle, interface) > offset_of!(UsbStorageHandle, configuration)
        );
        assert!(offset_of!(UsbStorageHandle, sector_size) > offset_of!(UsbStorageHandle, max_lun));
        assert!(offset_of!(UsbStorageHandle, buffer) > offset_of!(UsbStorageHandle, suspended));

        assert!(size_of::<RawDeviceCommand>() >= 32);
        assert_eq!(offset_of!(RawDeviceCommand, command), 0);
        assert_eq!(offset_of!(RawDeviceCommand, command_length), 16);
        assert!(offset_of!(RawDeviceCommand, data) > offset_of!(RawDeviceCommand, scsi_status));
        assert!(offset_of!(RawDeviceCommand, data_length) > offset_of!(RawDeviceCommand, data));
    }

    #[test]
    fn idle_remount_matches_c_threshold() {
        assert!(!idle_remount_required(160, 100));
        assert!(idle_remount_required(161, 100));
        assert!(idle_remount_required(100, 0));
    }

    #[test]
    fn short_read_copies_prefix_and_returns_data_residue() {
        let mut output = [0xaa; 4];
        assert_eq!(
            copy_read_result(&[0x11, 0x22, 0x33, 0x44], &mut output, 2),
            USBSTORAGE_EDATARESIDUE
        );
        assert_eq!(output, [0x11, 0x22, 0xaa, 0xaa]);
    }

    #[test]
    fn zero_byte_short_transfer_returns_data_residue() {
        let mut output = [0xaa; 4];
        assert_eq!(
            copy_read_result(&[0; 4], &mut output, 0),
            USBSTORAGE_EDATARESIDUE
        );
        assert_eq!(output, [0xaa; 4]);
        assert_eq!(transfer_result(0, 4), USBSTORAGE_EDATARESIDUE);
        assert_eq!(transfer_result(-41, 4), -41);
        assert_eq!(
            transfer_result(USBSTORAGE_ETIMEDOUT, 4),
            USBSTORAGE_ETIMEDOUT
        );
    }

    #[test]
    fn bot_retry_stops_only_for_timeout_and_preserves_cycle_error() {
        assert_eq!(bot_cycle_retry_error(-41, -42), None);
        assert_eq!(bot_cycle_retry_error(-41, USBSTORAGE_OK), None);
        assert_eq!(
            bot_cycle_retry_error(-41, USBSTORAGE_ETIMEDOUT),
            Some(USBSTORAGE_ETIMEDOUT)
        );
        assert_eq!(
            bot_cycle_retry_error(USBSTORAGE_ETIMEDOUT, USBSTORAGE_OK),
            Some(USBSTORAGE_ETIMEDOUT)
        );
    }

    #[test]
    fn host_runtime_surface_fails_closed() {
        let mut rdc = RawDeviceCommand::default();
        let mut handle = UsbStorageHandle::default();
        let mut sector_size = 0;
        let mut sectors = 0;
        let mut buffer = [0u8; 512];

        assert_eq!(initialize(), USBSTORAGE_EINIT);
        unsafe {
            assert_eq!(is_dvd(), 0);
            assert_eq!(is_dvd(), 0);
            assert_eq!(
                open(&mut handle, 7, 0x1234, 0xabcd),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(close(&mut handle), USBSTORAGE_ENOINTERFACE);
            assert_eq!(reset(&mut handle), USBSTORAGE_ENOINTERFACE);
            assert_eq!(get_max_lun(&mut handle), USBSTORAGE_ENOINTERFACE);
            assert_eq!(mount_lun(&mut handle, 0), USBSTORAGE_ENOINTERFACE);
            assert_eq!(suspend(&mut handle), USBSTORAGE_ENOINTERFACE);
            assert_eq!(
                ioctl(B_RAW_DEVICE_COMMAND as i32, &mut rdc),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                read_capacity(&mut handle, 0, &mut sector_size, &mut sectors),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                read(&mut handle, 0, 0, 1, buffer.as_mut_ptr()),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                write(&mut handle, 0, 0, 1, buffer.as_ptr()),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(start_stop(&mut handle, 0, 0, 1, 0), USBSTORAGE_ENOINTERFACE);
            assert_eq!(
                open(core::ptr::null_mut(), 0, 0, 0),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(close(core::ptr::null_mut()), USBSTORAGE_ENOINTERFACE);
            assert_eq!(reset(core::ptr::null_mut()), USBSTORAGE_ENOINTERFACE);
            assert_eq!(get_max_lun(core::ptr::null_mut()), USBSTORAGE_ENOINTERFACE);
            assert_eq!(mount_lun(core::ptr::null_mut(), 0), USBSTORAGE_ENOINTERFACE);
            assert_eq!(suspend(core::ptr::null_mut()), USBSTORAGE_ENOINTERFACE);
            assert_eq!(
                read_capacity(
                    core::ptr::null_mut(),
                    0,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                read(core::ptr::null_mut(), 0, 0, 1, core::ptr::null_mut()),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                write(core::ptr::null_mut(), 0, 0, 1, core::ptr::null()),
                USBSTORAGE_ENOINTERFACE
            );
            assert_eq!(
                start_stop(core::ptr::null_mut(), 0, 0, 0, 0),
                USBSTORAGE_ENOINTERFACE
            );
        }
    }

    #[test]
    fn disc_interface_callbacks_fail_closed_on_host() {
        assert_eq!(USB_STORAGE_SECTOR_SIZE, 512);
        assert_eq!(USB_STORAGE_INTERFACE.io_type, DEVICE_TYPE_WII_USB);
        assert_eq!(USB_STORAGE_INTERFACE.io_type, DEVICE_TYPE_WII_USB);
        assert_eq!(
            USB_STORAGE_INTERFACE.features,
            FEATURE_MEDIUM_CANREAD | FEATURE_MEDIUM_CANWRITE | FEATURE_WII_USB
        );
        assert!(USB_STORAGE_INTERFACE.startup.is_some());
        assert!(USB_STORAGE_INTERFACE.is_inserted.is_some());
        assert!(USB_STORAGE_INTERFACE.read_sectors.is_some());
        assert!(USB_STORAGE_INTERFACE.write_sectors.is_some());
        assert!(USB_STORAGE_INTERFACE.clear_status.is_some());
        assert!(USB_STORAGE_INTERFACE.shutdown.is_some());

        unsafe {
            assert!(
                !USB_STORAGE_INTERFACE
                    .startup
                    .is_some_and(|callback| callback())
            );
            assert!(
                !USB_STORAGE_INTERFACE
                    .is_inserted
                    .is_some_and(|callback| callback())
            );
            let mut sector = [0u8; 512];
            assert!(
                !USB_STORAGE_INTERFACE
                    .read_sectors
                    .is_some_and(|callback| callback(0, 1, sector.as_mut_ptr().cast()))
            );
            assert!(
                !USB_STORAGE_INTERFACE
                    .write_sectors
                    .is_some_and(|callback| callback(0, 1, sector.as_ptr().cast()))
            );
            assert!(
                !USB_STORAGE_INTERFACE
                    .read_sectors
                    .is_some_and(|callback| callback(0, 1, core::ptr::null_mut()))
            );
            assert!(
                !USB_STORAGE_INTERFACE
                    .clear_status
                    .is_some_and(|callback| callback())
            );
            assert!(
                !USB_STORAGE_INTERFACE
                    .shutdown
                    .is_some_and(|callback| callback())
            );
        }
    }
}
