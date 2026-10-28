#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

pub use ogc_filesystem_sys::DiscInterface;
pub use u32;

pub const DEVICE_TYPE_GC_SD: u32 =
    (b'G' as u32) << 24 | (b'C' as u32) << 16 | (b'S' as u32) << 8 | b'D' as u32;

mod timeout;

const SLOT_A: usize = 0;
const SLOT_B: usize = 1;
const SLOT_2: usize = 2;
const MAX_DRIVE: usize = 3;

pub const CARD_IO_SECTOR_ADDRESSING: u32 = 0;
pub const CARD_IO_BYTE_ADDRESSING: u32 = 1;

pub const MAX_MI_NUM: usize = 1;
pub const MAX_DI_NUM: usize = 5;
const CARDIO_ERROR_NOCARD: i32 = -3;
const BUFFER_POOL_COUNT: usize = 3;
const BUFFER_BYTES: usize = 514;

#[unsafe(no_mangle)]
pub static mut g_CID: [[u8; 16]; MAX_DRIVE] = [[0; 16]; MAX_DRIVE];
#[unsafe(no_mangle)]
pub static mut g_CSD: [[u8; 16]; MAX_DRIVE] = [[0; 16]; MAX_DRIVE];
#[unsafe(no_mangle)]
pub static mut g_CardStatus: [[u8; 64]; MAX_DRIVE] = [[0; 64]; MAX_DRIVE];
#[unsafe(no_mangle)]
pub static mut g_mCode: [u8; MAX_MI_NUM] = [0x03];
#[unsafe(no_mangle)]
pub static mut g_dCode: [[u16; MAX_DI_NUM]; MAX_MI_NUM] =
    [[0x033f, 0x0383, 0x074b, 0x0edf, 0x0f03]];

static mut BUFFER_POOL: [[u8; BUFFER_BYTES]; BUFFER_POOL_COUNT] =
    [[0; BUFFER_BYTES]; BUFFER_POOL_COUNT];

// Two bits per value: head, next[0], next[1], next[2]. Values are encoded
// as index + 1, with zero meaning the end of the free list.
const BUFFER_STACK_INITIAL: u32 = 1 | (2 << 2) | (3 << 4);
static BUFFER_STACK: AtomicU32 = AtomicU32::new(BUFFER_STACK_INITIAL);

fn stack_field(state: u32, field: usize) -> u32 {
    (state >> (field * 2)) & 0x3
}

fn with_stack_field(state: u32, field: usize, value: u32) -> u32 {
    let shift = field * 2;
    (state & !(0x3 << shift)) | ((value & 0x3) << shift)
}

pub(crate) fn init_buffer_pool() {
    BUFFER_STACK.store(BUFFER_STACK_INITIAL, Ordering::Release);
}

fn alloc_buffer() -> *mut u8 {
    let mut state = BUFFER_STACK.load(Ordering::Acquire);
    loop {
        let head = stack_field(state, 0);
        if head == 0 {
            return core::ptr::null_mut();
        }
        let index = (head - 1) as usize;
        let next = stack_field(state, index + 1);
        let updated = with_stack_field(state, 0, next);
        match BUFFER_STACK.compare_exchange_weak(
            state,
            updated,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => unsafe {
                return core::ptr::addr_of_mut!(BUFFER_POOL)
                    .cast::<[u8; BUFFER_BYTES]>()
                    .add(index)
                    .cast::<u8>();
            },
            Err(current) => state = current,
        }
    }
}

fn free_buffer(buffer: *mut u8) {
    if buffer.is_null() {
        return;
    }
    let base = core::ptr::addr_of_mut!(BUFFER_POOL).cast::<u8>();
    let base_addr = base as usize;
    let buffer_addr = buffer as usize;
    let end_addr = base_addr + BUFFER_POOL_COUNT * BUFFER_BYTES;
    if buffer_addr < base_addr || buffer_addr >= end_addr {
        return;
    }
    let offset = buffer_addr - base_addr;
    if !offset.is_multiple_of(BUFFER_BYTES) {
        return;
    }
    let index = offset / BUFFER_BYTES;
    if index >= BUFFER_POOL_COUNT {
        return;
    }

    let encoded = (index + 1) as u32;
    let mut state = BUFFER_STACK.load(Ordering::Acquire);
    loop {
        let head = stack_field(state, 0);
        let with_next = with_stack_field(state, index + 1, head);
        let updated = with_stack_field(with_next, 0, encoded);
        match BUFFER_STACK.compare_exchange_weak(
            state,
            updated,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return,
            Err(current) => state = current,
        }
    }
}

fn drive_index(drv_no: i32) -> Option<usize> {
    usize::try_from(drv_no)
        .ok()
        .filter(|slot| *slot < MAX_DRIVE)
}

/// Return a drive's EXI speed. Invalid drive numbers return `None`.
pub fn sdgecko_get_speed(drv_no: i32) -> Option<u32> {
    drive_index(drv_no).and_then(wii_backend::get_speed)
}

/// Set a drive's EXI speed. Return `false` when the drive number is invalid.
pub fn sdgecko_set_speed(drv_no: i32, frequency: u32) -> bool {
    drive_index(drv_no).is_some_and(|slot| wii_backend::set_speed(slot, frequency))
}

/// Return a drive's current block size. Invalid drive numbers return `None`.
pub fn sdgecko_get_page_size(drv_no: i32) -> Option<u32> {
    drive_index(drv_no).and_then(wii_backend::get_page_size)
}

/// Store a drive's block size, then issue CMD16. Invalid drive numbers return `None`.
pub fn sdgecko_set_page_size(drv_no: i32, size: u32) -> Option<i32> {
    drive_index(drv_no).and_then(|slot| wii_backend::set_page_size(slot, size))
}

/// Return a drive's addressing mode. Invalid drive numbers return `None`.
pub fn sdgecko_get_addressing_type(drv_no: i32) -> Option<u32> {
    drive_index(drv_no).and_then(wii_backend::get_addressing_type)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_initIODefault() {
    wii_backend::init_io_default();
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_initIO(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::init_io)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_preIO(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::pre_io)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_readCID(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::read_cid_api)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_readCSD(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::read_csd_api)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_readStatus(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::read_status_api)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
/// # Safety
/// The output buffer must be writable for num_sectors times 512 bytes and stay valid for the synchronous transfer.
pub unsafe extern "C" fn sdgecko_readSectors(
    drv_no: i32,
    sector_no: u32,
    num_sectors: u32,
    buffer: *mut c_void,
) -> i32 {
    let Some(slot) = drive_index(drv_no) else {
        return CARDIO_ERROR_NOCARD;
    };
    wii_backend::read_sectors_result(slot, sector_no, num_sectors, buffer.cast())
}

#[unsafe(no_mangle)]
/// # Safety
/// The input buffer must be readable for num_sectors times 512 bytes and stay valid for the synchronous transfer.
pub unsafe extern "C" fn sdgecko_writeSectors(
    drv_no: i32,
    sector_no: u32,
    num_sectors: u32,
    buffer: *const c_void,
) -> i32 {
    let Some(slot) = drive_index(drv_no) else {
        return CARDIO_ERROR_NOCARD;
    };
    wii_backend::write_sectors_result(slot, sector_no, num_sectors, buffer.cast())
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_doUnmount(drv_no: i32) -> i32 {
    drive_index(drv_no)
        .map(wii_backend::do_unmount)
        .unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_insertedCB(_drv_no: i32) {
    // The reference callback table has no public setter and is initialized to null.
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_ejectedCB(_drv_no: i32) {
    // The reference callback table has no public setter and is initialized to null.
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_getSpeed(drv_no: i32) -> u32 {
    sdgecko_get_speed(drv_no).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_setSpeed(drv_no: i32, frequency: u32) {
    let _ = sdgecko_set_speed(drv_no, frequency);
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_getPageSize(drv_no: i32) -> u32 {
    sdgecko_get_page_size(drv_no).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_setPageSize(drv_no: i32, size: u32) -> i32 {
    sdgecko_set_page_size(drv_no, size).unwrap_or(CARDIO_ERROR_NOCARD)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_getAddressingType(drv_no: i32) -> u32 {
    sdgecko_get_addressing_type(drv_no).unwrap_or(CARD_IO_SECTOR_ADDRESSING)
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_initBufferPool() {
    init_buffer_pool();
}

#[unsafe(no_mangle)]
pub extern "C" fn sdgecko_allocBuffer() -> *mut u8 {
    alloc_buffer()
}

#[unsafe(no_mangle)]
/// # Safety
/// The pointer must be null or a live buffer returned by the matching SD Gecko allocator, with no remaining users.
pub unsafe extern "C" fn sdgecko_freeBuffer(buffer: *mut u8) {
    free_buffer(buffer);
}

#[cfg(target_arch = "powerpc")]
mod wii_backend;

#[cfg(not(target_arch = "powerpc"))]
mod wii_backend {
    pub fn get_speed(_slot: usize) -> Option<u32> {
        None
    }
    pub fn set_speed(_slot: usize, _frequency: u32) -> bool {
        false
    }
    pub fn get_page_size(_slot: usize) -> Option<u32> {
        None
    }
    pub fn set_page_size(_slot: usize, _size: u32) -> Option<i32> {
        None
    }
    pub fn get_addressing_type(_slot: usize) -> Option<u32> {
        None
    }
    pub fn startup(_slot: usize) -> bool {
        false
    }
    pub fn is_inserted(_slot: usize) -> bool {
        false
    }
    pub fn read_sectors(_slot: usize, _sector: u32, _num_sectors: u32, _buffer: *mut u8) -> bool {
        false
    }
    pub fn write_sectors(
        _slot: usize,
        _sector: u32,
        _num_sectors: u32,
        _buffer: *const u8,
    ) -> bool {
        false
    }
    pub fn clear_status(_slot: usize) -> bool {
        false
    }
    pub fn shutdown(_slot: usize) -> bool {
        false
    }
}

macro_rules! define_slot_interface {
    ($name:ident, $startup_fn:ident, $is_inserted_fn:ident, $read_fn:ident, $write_fn:ident, $clear_fn:ident, $shutdown_fn:ident, $slot:expr_2021, $features:expr_2021) => {
        unsafe extern "C" fn $startup_fn() -> bool {
            wii_backend::startup($slot)
        }

        unsafe extern "C" fn $is_inserted_fn() -> bool {
            wii_backend::is_inserted($slot)
        }

        unsafe extern "C" fn $read_fn(sector: u32, num_sectors: u32, buffer: *mut c_void) -> bool {
            wii_backend::read_sectors($slot, sector, num_sectors, buffer.cast())
        }

        unsafe extern "C" fn $write_fn(
            sector: u32,
            num_sectors: u32,
            buffer: *const c_void,
        ) -> bool {
            wii_backend::write_sectors($slot, sector, num_sectors, buffer.cast())
        }

        unsafe extern "C" fn $clear_fn() -> bool {
            wii_backend::clear_status($slot)
        }

        unsafe extern "C" fn $shutdown_fn() -> bool {
            wii_backend::shutdown($slot)
        }

        pub static $name: DiscInterface = DiscInterface {
            io_type: DEVICE_TYPE_GC_SD,
            features: $features,
            startup: Some($startup_fn),
            is_inserted: Some($is_inserted_fn),
            read_sectors: Some($read_fn),
            write_sectors: Some($write_fn),
            clear_status: Some($clear_fn),
            shutdown: Some($shutdown_fn),
        };
    };
}

define_slot_interface!(
    GAMECUBE_SD_A_INTERFACE,
    startup_a,
    is_inserted_a,
    read_sectors_a,
    write_sectors_a,
    clear_status_a,
    shutdown_a,
    SLOT_A,
    0x0000_0013
);
define_slot_interface!(
    GAMECUBE_SD_B_INTERFACE,
    startup_b,
    is_inserted_b,
    read_sectors_b,
    write_sectors_b,
    clear_status_b,
    shutdown_b,
    SLOT_B,
    0x0000_0023
);
define_slot_interface!(
    GAMECUBE_SD_2_INTERFACE,
    startup_2,
    is_inserted_2,
    read_sectors_2,
    write_sectors_2,
    clear_status_2,
    shutdown_2,
    SLOT_2,
    0x0000_0083
);

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn operating_condition_timeout_makes_and_checks_one_final_attempt() {
        use core::cell::Cell;

        let attempts = Cell::new(0);
        let final_attempts = Cell::new(0);
        let result = wii_backend::send_opcond_with_attempts(
            |is_final| {
                attempts.set(attempts.get() + 1);
                if is_final {
                    final_attempts.set(final_attempts.get() + 1);
                    Ok(false)
                } else {
                    Ok(true)
                }
            },
            || true,
        );

        assert_eq!(result, 0);
        assert_eq!(attempts.get(), 2);
        assert_eq!(final_attempts.get(), 1);
    }

    #[test]
    fn operating_condition_final_attempt_reports_idle_card_as_error() {
        use core::cell::Cell;

        let attempts = Cell::new(0);
        let result = wii_backend::send_opcond_with_attempts(
            |_| {
                attempts.set(attempts.get() + 1);
                Ok(true)
            },
            || true,
        );

        assert_eq!(result, -5);
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn operating_condition_final_attempt_propagates_response_error() {
        use core::cell::Cell;

        let attempts = Cell::new(0);
        let result = wii_backend::send_opcond_with_attempts(
            |is_final| {
                attempts.set(attempts.get() + 1);
                if is_final { Err(-7) } else { Ok(true) }
            },
            || true,
        );

        assert_eq!(result, -7);
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn disc_interface_layout_is_host_stable() {
        assert_eq!(offset_of!(DiscInterface, io_type), 0);
        assert_eq!(offset_of!(DiscInterface, features), 4);
        assert!(offset_of!(DiscInterface, startup) > offset_of!(DiscInterface, features));
        assert!(offset_of!(DiscInterface, shutdown) > offset_of!(DiscInterface, clear_status));
        assert_eq!(align_of::<DiscInterface>(), align_of::<usize>());
        assert!(size_of::<DiscInterface>() >= 32);
    }

    #[test]
    fn exported_interfaces_have_slot_features_and_callbacks() {
        assert_eq!(GAMECUBE_SD_A_INTERFACE.io_type, DEVICE_TYPE_GC_SD);
        assert_eq!(GAMECUBE_SD_B_INTERFACE.io_type, DEVICE_TYPE_GC_SD);
        assert_eq!(GAMECUBE_SD_2_INTERFACE.io_type, DEVICE_TYPE_GC_SD);
        assert_eq!(GAMECUBE_SD_A_INTERFACE.features, 0x0000_0013);
        assert_eq!(GAMECUBE_SD_B_INTERFACE.features, 0x0000_0023);
        assert_eq!(GAMECUBE_SD_2_INTERFACE.features, 0x0000_0083);

        for interface in [
            &GAMECUBE_SD_A_INTERFACE,
            &GAMECUBE_SD_B_INTERFACE,
            &GAMECUBE_SD_2_INTERFACE,
        ] {
            assert!(interface.startup.is_some());
            assert!(interface.is_inserted.is_some());
            assert!(interface.read_sectors.is_some());
            assert!(interface.write_sectors.is_some());
            assert!(interface.clear_status.is_some());
            assert!(interface.shutdown.is_some());
        }
    }

    #[test]
    fn public_card_io_abi_and_buffer_pool_are_present() {
        let _: extern "C" fn() = sdgecko_initIODefault;
        let _: extern "C" fn(i32) -> i32 = sdgecko_initIO;
        let _: extern "C" fn(i32) -> i32 = sdgecko_preIO;
        let _: extern "C" fn(i32) -> i32 = sdgecko_readCID;
        let _: extern "C" fn(i32) -> i32 = sdgecko_readCSD;
        let _: extern "C" fn(i32) -> i32 = sdgecko_readStatus;
        let _: extern "C" fn(i32) -> i32 = sdgecko_doUnmount;
        let _: extern "C" fn(i32) -> u32 = sdgecko_getSpeed;
        let _: extern "C" fn(i32, u32) = sdgecko_setSpeed;
        let _: extern "C" fn(i32) -> u32 = sdgecko_getPageSize;
        let _: extern "C" fn(i32, u32) -> i32 = sdgecko_setPageSize;
        let _: extern "C" fn(i32) -> u32 = sdgecko_getAddressingType;

        sdgecko_initBufferPool();
        let first = sdgecko_allocBuffer();
        let second = sdgecko_allocBuffer();
        let third = sdgecko_allocBuffer();
        assert!(!first.is_null());
        assert!(!second.is_null());
        assert!(!third.is_null());
        assert!(sdgecko_allocBuffer().is_null());
        unsafe { sdgecko_freeBuffer(second) };
        assert_eq!(sdgecko_allocBuffer(), second);
    }

    #[test]
    fn public_card_io_accessors_report_invalid_drive_numbers_safely() {
        assert_eq!(sdgecko_get_speed(-1), None);
        assert!(!sdgecko_set_speed(-1, 1));
        assert_eq!(sdgecko_get_page_size(3), None);
        assert_eq!(sdgecko_set_page_size(3, 256), None);
        assert_eq!(sdgecko_get_addressing_type(3), None);
    }

    #[test]
    fn unsupported_backend_never_reports_success() {
        let mut sector = [0u8; 512];
        for interface in [
            &GAMECUBE_SD_A_INTERFACE,
            &GAMECUBE_SD_B_INTERFACE,
            &GAMECUBE_SD_2_INTERFACE,
        ] {
            unsafe {
                assert!(!interface.startup.is_some_and(|callback| callback()));
                assert!(!interface.is_inserted.is_some_and(|callback| callback()));
                assert!(!interface.write_sectors.is_some_and(|callback| callback(
                    0,
                    1,
                    sector.as_ptr().cast()
                )));
                assert!(!interface.read_sectors.is_some_and(|callback| callback(
                    0,
                    1,
                    sector.as_mut_ptr().cast()
                )));
                assert!(!interface.clear_status.is_some_and(|callback| callback()));
                assert!(!interface.shutdown.is_some_and(|callback| callback()));
            }
        }
    }
}
