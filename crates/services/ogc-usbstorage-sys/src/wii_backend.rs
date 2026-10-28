#![cfg(target_arch = "powerpc")]

use alloc::alloc::{Layout, alloc_zeroed, dealloc};
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};

use ogc_lwp_sys as lwp;
use ogc_system_sys::{
    TimeSpec, sys_cancel_alarm, sys_create_alarm, sys_get_hollywood_revision, sys_remove_alarm,
    sys_set_alarm,
};
use ogc_usb_sys as usb;

use super::*;
use ogc_usbstorage_lifecycle::transport::{self, BotResetStep, reset as run_bot_reset_sequence};
use ogc_usbstorage_lifecycle::{self as close_policy, CloseOperations};

const TAG_START: u32 = 0x0bad_c0de;
const CBW_SIZE: usize = 31;
const CBW_SIGNATURE: u32 = 0x4342_5355;
const CBW_IN: u8 = 1 << 7;
const CBW_OUT: u8 = 0;
const CSW_SIZE: usize = 13;
const CSW_SIGNATURE: u32 = 0x5342_5355;

const SCSI_TEST_UNIT_READY: u8 = 0x00;
const SCSI_REQUEST_SENSE: u8 = 0x03;
const SCSI_INQUIRY: u8 = 0x12;
const SCSI_START_STOP: u8 = 0x1b;
const SCSI_READ_CAPACITY: u8 = 0x25;
const SCSI_READ_10: u8 = 0x28;
const SCSI_WRITE_10: u8 = 0x2a;
const SCSI_SYNCHRONIZE_CACHE: u8 = 0x35;

const SCSI_SENSE_REPLY_SIZE: usize = 18;
const SCSI_SENSE_NOT_READY: u8 = 0x02;
const SCSI_SENSE_MEDIUM_ERROR: u8 = 0x03;
const SCSI_SENSE_HARDWARE_ERROR: u8 = 0x04;

const USB_CLASS_MASS_STORAGE: u8 = 0x08;
const MASS_STORAGE_BULK_ONLY: u8 = 0x50;
const USBSTORAGE_GET_MAX_LUN: u8 = 0xfe;
const USBSTORAGE_RESET: u8 = 0xff;
const USB_ENDPOINT_BULK: u8 = 0x02;

const USBSTORAGE_CYCLE_RETRIES: usize = 3;
const BOT_RESET_CONTROL_TIMEOUT_TICKS: u64 = lwp::TICKS_PER_SECOND;
const USBSTORAGE_CONTROL_TIMEOUT_TICKS: u64 = 2 * lwp::TICKS_PER_SECOND;
const USBSTORAGE_START_STOP_TIMEOUT_TICKS: u64 = 10 * lwp::TICKS_PER_SECOND;
const CONTROL_ACTIVE: u8 = 1;
const CONTROL_COMPLETE: u8 = 2;
const CONTROL_TIMED_OUT: u8 = 3;
const MAX_TRANSFER_SIZE_V0: usize = 4 * 1024;
const MAX_TRANSFER_SIZE_V5: usize = 16 * 1024;
const MAX_TRANSFER_SIZE: usize = MAX_TRANSFER_SIZE_V5;
// `UsbStorageHandle::max_lun` is a u8 count. A count of 255 represents LUN
// indices 0 through 254; the 256-entry case cannot be represented by the C ABI.
const MAX_DEVICE_LUNS: usize = u8::MAX as usize;
const DEVLIST_MAXSIZE: usize = 8;
const STORAGE_PROBE_CLASSES: [u8; 1] = [USB_CLASS_MASS_STORAGE];

static USBSTORAGE_LAST_USED_SECS: AtomicU32 = AtomicU32::new(0);
static USBSTORAGE_LAST_IDLE_REMOUNT_RESULT: AtomicI32 = AtomicI32::new(USBSTORAGE_OK);
static ASYNC_CONTROL_DEFER_FAILURES: AtomicU32 = AtomicU32::new(0);

#[repr(align(32))]
struct AsyncPayload<T>(UnsafeCell<T>);

unsafe impl<T: Send> Sync for AsyncPayload<T> {}

#[repr(C, align(32))]
struct AsyncControl<T: Copy> {
    state: AtomicU8,
    result: AtomicI32,
    allocation_size: usize,
    allocation_align: usize,
    data: AsyncPayload<T>,
}

unsafe impl<T: Copy + Send> Sync for AsyncControl<T> {}

impl<T: Copy> AsyncControl<T> {
    fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(CONTROL_ACTIVE),
            result: AtomicI32::new(USBSTORAGE_PROCESSING),
            allocation_size: Layout::new::<Self>().size(),
            allocation_align: Layout::new::<Self>().align(),
            data: AsyncPayload(UnsafeCell::new(data)),
        }
    }
}

fn allocate_async_control<T: Copy>(data: T) -> Result<*mut AsyncControl<T>, i32> {
    let layout = Layout::new::<AsyncControl<T>>();
    let control = unsafe { alloc_zeroed(layout).cast::<AsyncControl<T>>() };
    if control.is_null() {
        return Err(USBSTORAGE_EINIT);
    }
    unsafe { control.write(AsyncControl::new(data)) };
    Ok(control)
}

unsafe fn deallocate_async_control<T: Copy>(control: *mut AsyncControl<T>) {
    unsafe { dealloc(control.cast(), Layout::new::<AsyncControl<T>>()) };
}

unsafe fn deallocate_async_control_deferred(control: *mut core::ffi::c_void) {
    let header = control.cast::<AsyncControl<()>>();
    let size = unsafe { (*header).allocation_size };
    let align = unsafe { (*header).allocation_align };
    if let Ok(layout) = Layout::from_size_align(size, align) {
        unsafe { dealloc(control.cast(), layout) };
    }
}

unsafe fn complete_async_control<T: Copy>(control: *mut AsyncControl<T>, result: i32) {
    if control.is_null() {
        return;
    }
    let control_ref = unsafe { &*control };
    control_ref.result.store(result, Ordering::Relaxed);
    if control_ref
        .state
        .compare_exchange(
            CONTROL_ACTIVE,
            CONTROL_COMPLETE,
            Ordering::Release,
            Ordering::Acquire,
        )
        .is_err()
        && control_ref
            .state
            .compare_exchange(CONTROL_TIMED_OUT, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let deferred = unsafe {
            lwp::deferred_call_from_interrupt(
                Some(deallocate_async_control_deferred),
                control.cast(),
            )
        };
        if deferred < 0 {
            ASYNC_CONTROL_DEFER_FAILURES.fetch_add(1, Ordering::AcqRel);
        }
    }
}

pub(super) fn async_control_cleanup_failures() -> u32 {
    ASYNC_CONTROL_DEFER_FAILURES.load(Ordering::Acquire)
}

fn pending_bulk_error(result: &AtomicI32) -> Option<i32> {
    match close_policy::bulk::classify(result.load(Ordering::Acquire)) {
        close_policy::bulk::State::TimedOut {
            request_pending: true,
        } => Some(USBSTORAGE_ETIMEDOUT),
        close_policy::bulk::State::AlarmCancelFailed {
            alarm_pending: true,
        } => Some(USBSTORAGE_EINIT),
        _ => None,
    }
}

fn bulk_result(result: &AtomicI32) -> Option<i32> {
    match close_policy::bulk::classify(result.load(Ordering::Acquire)) {
        close_policy::bulk::State::Waiting => None,
        close_policy::bulk::State::Complete(value) => Some(value),
        close_policy::bulk::State::TimedOut { .. } => Some(USBSTORAGE_ETIMEDOUT),
        close_policy::bulk::State::AlarmCancelFailed { .. } => Some(USBSTORAGE_EINIT),
    }
}

unsafe fn wait_async_control<T: Copy>(
    control: *mut AsyncControl<T>,
    timeout_ticks: u64,
) -> Result<(i32, T), i32> {
    let started = lwp::now_ticks();
    loop {
        let control_ref = unsafe { &*control };
        if control_ref.state.load(Ordering::Acquire) == CONTROL_COMPLETE {
            let result = control_ref.result.load(Ordering::Acquire);
            let data = unsafe { *control_ref.data.0.get() };
            unsafe { deallocate_async_control(control) };
            return Ok((result, data));
        }
        if lwp::now_ticks().wrapping_sub(started) >= timeout_ticks
            && control_ref
                .state
                .compare_exchange(
                    CONTROL_ACTIVE,
                    CONTROL_TIMED_OUT,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        {
            return Err(USBSTORAGE_ETIMEDOUT);
        }
        lwp::yield_thread();
    }
}

struct TransferGuard {
    mutex: lwp::Mutex,
}

impl TransferGuard {
    fn acquire(mutex: lwp::Mutex) -> Result<Self, i32> {
        if mutex == lwp::LWP_MUTEX_NULL || lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            return Err(USBSTORAGE_EINIT);
        }
        Ok(Self { mutex })
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        let result = lwp::mutex_unlock(self.mutex);
        if result != lwp::LWP_SUCCESSFUL {
            super::startup::record(super::ProbeStage::Reset, result);
        }
    }
}

fn storage_now_seconds() -> u32 {
    (lwp::now_ticks() / lwp::TICKS_PER_SECOND) as u32
}

fn transfer_chunk_size(usb2_mode: bool) -> usize {
    if usb2_mode {
        MAX_TRANSFER_SIZE_V5
    } else {
        MAX_TRANSFER_SIZE_V0
    }
}

fn io_timeout_ticks(idle_remount: bool) -> u64 {
    if idle_remount {
        USBSTORAGE_START_STOP_TIMEOUT_TICKS
    } else {
        USBSTORAGE_CONTROL_TIMEOUT_TICKS
    }
}

fn with_transfer_guard<G>(
    acquire: impl FnOnce() -> Result<G, i32>,
    operation: impl FnOnce() -> i32,
) -> i32 {
    let _guard = match acquire() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    operation()
}

// The handle address is stable for IOS callbacks. Each disc owns its transfer
// resources, media state, idle clock, and transport mode.
struct Drive {
    handle: UnsafeCell<UsbStorageHandle>,
    state: close_policy::drives::DriveState,
}

// Public raw-handle APIs require an exclusive caller. Disc operations are
// serialized by the shared disc lock below; callbacks change atomics only.
unsafe impl Sync for Drive {}

impl Drive {
    const fn new() -> Self {
        Self {
            handle: UnsafeCell::new(UsbStorageHandle {
                configuration: 0,
                interface: 0,
                alt_interface: 0,
                b_interface_sub_class: 0,
                ep_in: 0,
                ep_out: 0,
                max_lun: 0,
                sector_size: ptr::null_mut(),
                usb_fd: -1,
                lock: 0,
                alarm: 0,
                retval: 0,
                tag: 0,
                suspended: 0,
                buffer: ptr::null_mut(),
            }),
            state: close_policy::drives::DriveState::new(),
        }
    }
}
impl core::ops::Deref for Drive {
    type Target = close_policy::drives::DriveState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

static DRIVES: [Drive; 2] = [const { Drive::new() }; 2];
static USB2_MODE: AtomicBool = AtomicBool::new(false);
static DISC_MUTEX: AtomicU32 = AtomicU32::new(lwp::LWP_MUTEX_NULL);

// A caller can have a higher priority than the current storage worker. An LWP
// mutex blocks that caller while the worker finishes; a yield-only spin lock
// can keep selecting the higher-priority waiter and prevent progress.
fn lock_discs() -> Result<TransferGuard, i32> {
    let result = (|| {
        let mut mutex = DISC_MUTEX.load(Ordering::Acquire);
        if mutex == lwp::LWP_MUTEX_NULL {
            let mut created = lwp::LWP_MUTEX_NULL;
            let initialized = unsafe { lwp::mutex_init(&mut created, 0) };
            if initialized != lwp::LWP_SUCCESSFUL {
                return Err(initialized);
            }
            mutex = match DISC_MUTEX.compare_exchange(
                lwp::LWP_MUTEX_NULL,
                created,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => created,
                Err(existing) => {
                    let destroyed = lwp::mutex_destroy(created);
                    if destroyed != lwp::LWP_SUCCESSFUL {
                        return Err(destroyed);
                    }
                    existing
                }
            };
        }
        TransferGuard::acquire(mutex)
    })();
    if let Err(error) = result {
        super::startup::record(super::ProbeStage::Reset, error);
    }
    result
}

fn drive_for_handle(dev: *const UsbStorageHandle) -> Option<&'static Drive> {
    DRIVES
        .iter()
        .find(|drive| core::ptr::eq(drive.handle.get(), dev))
}

// Keep primary-disc diagnostics compatible with the existing public API.
const DISC_MOUNTED: &AtomicBool = &DRIVES[0].state.mounted;
const DISC_LUN: &AtomicU32 = &DRIVES[0].state.lun;
#[cfg(test)]
const DISC_DEVICE_ID: &AtomicI32 = &DRIVES[0].state.device_id;
#[cfg(test)]
const DISC_VID: &AtomicU32 = &DRIVES[0].state.vid;
#[cfg(test)]
const DISC_PID: &AtomicU32 = &DRIVES[0].state.pid;

#[inline]
fn is_wii_hardware() -> bool {
    (unsafe { sys_get_hollywood_revision() }) != 0
}

fn reset_global_disc_state() {
    DRIVES[0].reset();
}

#[cfg(test)]
fn record_storage_device_removed() {
    DISC_MOUNTED.store(false, Ordering::Release);
}

unsafe extern "C" fn storage_device_removed(_result: i32, userdata: *mut c_void) -> i32 {
    if let Some(drive) = drive_for_handle(userdata.cast()) {
        drive.remove();
    }
    USBSTORAGE_OK
}

fn clear_handle_state(dev: *const UsbStorageHandle) {
    if let Some(drive) = drive_for_handle(dev) {
        drive.reset();
    }
    if core::ptr::eq(dev, DRIVES[0].handle.get()) {
        USBSTORAGE_MOUNTED.store(false, Ordering::Release);
        USBSTORAGE_CAPACITY_VALID.store(false, Ordering::Release);
        USBSTORAGE_LAST_DEVICE_ID.store(-1, Ordering::Release);
    }
}

fn handle_last_used(dev: *const UsbStorageHandle) -> &'static AtomicU32 {
    drive_for_handle(dev).map_or(&USBSTORAGE_LAST_USED_SECS, |drive| &drive.last_used)
}

fn same_usb_device_entry(a: usb::UsbDeviceEntry, b: usb::UsbDeviceEntry) -> bool {
    a.device_id == b.device_id && a.vid == b.vid && a.pid == b.pid
}

fn append_device_candidates(
    buffer: &mut [usb::UsbDeviceEntry; DEVLIST_MAXSIZE],
    count: &mut usize,
    interface_class: u8,
) -> i32 {
    let mut scratch = [usb::UsbDeviceEntry::default(); DEVLIST_MAXSIZE];
    let mut scratch_count = 0u8;
    let rc = unsafe {
        usb::get_device_list(
            scratch.as_mut_ptr(),
            DEVLIST_MAXSIZE as u8,
            interface_class,
            &mut scratch_count,
        )
    };
    if rc < 0 {
        return rc;
    }

    append_storage_candidates(
        buffer,
        count,
        interface_class,
        scratch.iter().copied().take(scratch_count as usize),
    );
    USBSTORAGE_OK
}

fn append_storage_candidates(
    buffer: &mut [usb::UsbDeviceEntry; DEVLIST_MAXSIZE],
    count: &mut usize,
    interface_class: u8,
    entries: impl Iterator<Item = usb::UsbDeviceEntry>,
) {
    if !STORAGE_PROBE_CLASSES.contains(&interface_class) {
        return;
    }

    for entry in entries {
        if entry.vid == 0 || entry.pid == 0 || *count >= buffer.len() {
            continue;
        }
        if buffer
            .iter()
            .take(*count)
            .any(|seen| same_usb_device_entry(*seen, entry))
        {
            continue;
        }
        buffer[*count] = entry;
        *count += 1;
    }
}

fn storage_device_candidates(
    buffer: &mut [usb::UsbDeviceEntry; DEVLIST_MAXSIZE],
) -> Result<usize, i32> {
    let mut count = 0usize;
    // Do not query the any-class list here. In the current USB API that list
    // includes devices from the HID host. Trying to open a controller as a
    // storage device can delay or interfere with controller input.
    let mut first_error = USBSTORAGE_OK;
    for interface_class in STORAGE_PROBE_CLASSES {
        let rc = append_device_candidates(buffer, &mut count, interface_class);
        if rc < 0 && first_error == USBSTORAGE_OK {
            first_error = rc;
        }
    }
    if count > 0 || first_error == USBSTORAGE_OK {
        Ok(count)
    } else {
        Err(first_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asynchronous_completion_state_is_owned_per_request() {
        let first = allocate_async_control(7u8).unwrap();
        let second = allocate_async_control(9u8).unwrap();
        assert_ne!(first, second);
        assert_eq!((first as usize) % 32, 0);
        assert_eq!((unsafe { (*first).data.0.get() as usize }) % 32, 0);
        unsafe {
            complete_async_control(first, -41);
            complete_async_control(second, -42);
        }

        assert_eq!(
            unsafe { (*first).state.load(Ordering::Acquire) },
            CONTROL_COMPLETE
        );
        assert_eq!(unsafe { (*first).result.load(Ordering::Acquire) }, -41);
        assert_eq!(unsafe { *(*first).data.0.get() }, 7);
        assert_eq!(
            unsafe { (*second).state.load(Ordering::Acquire) },
            CONTROL_COMPLETE
        );
        assert_eq!(unsafe { (*second).result.load(Ordering::Acquire) }, -42);
        assert_eq!(unsafe { *(*second).data.0.get() }, 9);
        unsafe {
            deallocate_async_control(first);
            deallocate_async_control(second);
        }
    }

    #[test]
    fn timed_out_bulk_request_blocks_handle_reuse_until_callback_drains() {
        let result = AtomicI32::new(USBSTORAGE_PROCESSING);
        assert_eq!(pending_bulk_error(&result), None);

        close_policy::bulk::timeout(&result);
        assert_eq!(pending_bulk_error(&result), Some(USBSTORAGE_ETIMEDOUT));
        assert!(close_policy::bulk::begin(&result).is_err());

        close_policy::bulk::complete(&result, 512, || {
            panic!("the alarm already fired before the late bulk callback")
        });
        assert_eq!(pending_bulk_error(&result), None);
        assert!(close_policy::bulk::begin(&result).is_ok());
    }

    #[test]
    fn idle_remount_threshold_matches_c() {
        assert!(!super::super::idle_remount_required(160, 100));
        assert!(super::super::idle_remount_required(161, 100));
        assert!(super::super::idle_remount_required(100, 0));
    }

    #[test]
    fn idle_io_uses_c_ten_second_timeout() {
        assert_eq!(io_timeout_ticks(false), 2 * lwp::TICKS_PER_SECOND);
        assert_eq!(io_timeout_ticks(true), 10 * lwp::TICKS_PER_SECOND);
        assert_eq!(timeout_timespec(io_timeout_ticks(false)).tv_sec, 2);
        assert_eq!(timeout_timespec(io_timeout_ticks(true)).tv_sec, 10);
    }

    #[test]
    fn transfer_chunk_size_matches_c_usb_mode() {
        assert_eq!(transfer_chunk_size(false), 4 * 1024);
        assert_eq!(transfer_chunk_size(true), 16 * 1024);
    }

    #[test]
    fn storage_probe_rejects_hid_candidates_and_accepts_mass_storage() {
        let hid = usb::UsbDeviceEntry {
            device_id: 7,
            vid: 0x1234,
            pid: 0xabcd,
            token: 0,
        };
        let storage = usb::UsbDeviceEntry {
            device_id: 8,
            vid: 0x5678,
            pid: 0xef01,
            token: 0,
        };
        let mut candidates = [usb::UsbDeviceEntry::default(); DEVLIST_MAXSIZE];
        let mut count = 0;

        append_storage_candidates(
            &mut candidates,
            &mut count,
            usb::USB_CLASS_HID,
            [hid].into_iter(),
        );
        assert_eq!(
            count, 0,
            "HID-only attachment must not be a storage candidate"
        );

        append_storage_candidates(
            &mut candidates,
            &mut count,
            USB_CLASS_MASS_STORAGE,
            [storage].into_iter(),
        );
        assert_eq!(count, 1);
        assert!(same_usb_device_entry(candidates[0], storage));
    }

    #[test]
    fn suspend_returns_usb_error_without_marking_handle_suspended() {
        let mut dev = UsbStorageHandle::default();
        dev.usb_fd = 23;
        let mut calls = 0;

        let rc = suspend_with(&mut dev, |fd| {
            calls += 1;
            assert_eq!(fd, 23);
            -55
        });

        assert_eq!(rc, -55);
        assert_eq!(dev.suspended, 0);
        assert_eq!(calls, 1);

        let rc = suspend_with(&mut dev, |_| {
            calls += 1;
            USBSTORAGE_OK
        });
        assert_eq!(rc, USBSTORAGE_OK);
        assert_eq!(dev.suspended, 1);
        assert_eq!(calls, 2, "a failed suspend must allow another USB request");
    }

    #[test]
    fn alarm_setup_failure_waits_for_submitted_request_completion() {
        let mut polls = 0;
        let mut yields = 0;
        let result = wait_for_submitted_request_after_alarm_error(
            -77,
            || {
                polls += 1;
                if polls < 3 {
                    USBSTORAGE_PROCESSING
                } else {
                    USBSTORAGE_OK
                }
            },
            || yields += 1,
        );

        assert_eq!(result, -77);
        assert_eq!(polls, 3);
        assert_eq!(yields, 2);
    }

    #[test]
    fn clear_status_matches_c_unconditional_success_result() {
        assert!(clear_status_result());
    }

    #[test]
    fn close_cleanup_releases_storage_and_clears_handle_and_global_state() {
        let mut dev = UsbStorageHandle::default();
        dev.usb_fd = 17;
        dev.max_lun = 1;
        assert_eq!(unsafe { allocate_handle_storage(&mut dev) }, USBSTORAGE_OK);
        unsafe { set_handle_sector_size(&mut dev, 0, 512) };
        DISC_MOUNTED.store(true, Ordering::Release);
        DISC_LUN.store(2, Ordering::Release);
        DISC_DEVICE_ID.store(17, Ordering::Release);
        DISC_VID.store(0x1234, Ordering::Release);
        DISC_PID.store(0x5678, Ordering::Release);
        USBSTORAGE_MOUNTED.store(true, Ordering::Release);
        USBSTORAGE_CAPACITY_VALID.store(true, Ordering::Release);
        USBSTORAGE_LAST_DEVICE_ID.store(17, Ordering::Release);

        unsafe { cleanup_closed_handle(&mut dev) };

        assert_eq!(dev.usb_fd, -1);
        assert_eq!(dev.max_lun, 0);
        assert!(dev.sector_size.is_null());
        assert!(dev.buffer.is_null());
        assert!(!DISC_MOUNTED.load(Ordering::Acquire));
        assert_eq!(DISC_LUN.load(Ordering::Acquire), 0);
        assert_eq!(DISC_DEVICE_ID.load(Ordering::Acquire), -1);
        assert_eq!(DISC_VID.load(Ordering::Acquire), 0);
        assert_eq!(DISC_PID.load(Ordering::Acquire), 0);
        assert!(!USBSTORAGE_MOUNTED.load(Ordering::Acquire));
        assert!(!USBSTORAGE_CAPACITY_VALID.load(Ordering::Acquire));
        assert_eq!(USBSTORAGE_LAST_DEVICE_ID.load(Ordering::Acquire), -1);
    }

    #[test]
    fn bot_reset_orders_delays_and_clears_both_halts_after_reset_error() {
        let mut calls = [BotResetStep::ClassReset; 6];
        let mut count = 0;

        let result = run_bot_reset_sequence(|step| {
            calls[count] = step;
            count += 1;
            match step {
                BotResetStep::ClassReset => -41,
                BotResetStep::ClearInHalt => -42,
                BotResetStep::ClearOutHalt => -43,
                BotResetStep::DelayMs(_) => USBSTORAGE_OK,
            }
        });

        assert_eq!(result, -41);
        assert_eq!(
            calls,
            [
                BotResetStep::ClassReset,
                BotResetStep::DelayMs(60),
                BotResetStep::ClearInHalt,
                BotResetStep::DelayMs(10),
                BotResetStep::ClearOutHalt,
                BotResetStep::DelayMs(10),
            ]
        );
    }

    #[test]
    fn bot_reset_returns_endpoint_clear_failure() {
        let result = run_bot_reset_sequence(|step| match step {
            BotResetStep::ClassReset => USBSTORAGE_OK,
            BotResetStep::ClearInHalt | BotResetStep::ClearOutHalt => -42,
            BotResetStep::DelayMs(_) => USBSTORAGE_OK,
        });

        assert_eq!(result, -42);
    }

    #[test]
    fn read_capacity_accepts_nonnegative_cycle_result_with_nonzero_csw_status() {
        let outcome = super::decode_read_capacity(USBSTORAGE_OK, 1, [0, 0, 0, 9, 0, 0, 2, 0]);
        assert_eq!(outcome.result, USBSTORAGE_OK);
        assert_eq!(outcome.sector_size, 512);
        assert_eq!(outcome.sectors, 9);
    }

    #[test]
    fn inquiry_runs_two_cycles_after_failures_and_returns_the_second_result() {
        let mut calls = 0;
        let result = super::inquiry_with_cycles(|response| {
            calls += 1;
            assert_eq!(response, &[0; 36]);
            if calls == 1 {
                -41
            } else {
                response[0] = 0xe5;
                USBSTORAGE_OK
            }
        });

        assert_eq!(calls, 2);
        assert_eq!(result, 5);
    }

    #[test]
    fn inquiry_stops_after_first_successful_cycle() {
        let mut calls = 0;
        let result = super::inquiry_with_cycles(|response| {
            calls += 1;
            response[0] = 0x83;
            USBSTORAGE_OK
        });

        assert_eq!(calls, 1);
        assert_eq!(result, 3);
    }

    #[test]
    fn read_capacity_writes_each_optional_output_independently() {
        let capacity = super::ReadCapacityResult {
            result: USBSTORAGE_OK,
            sector_size: 2048,
            sectors: 1234,
        };
        let mut sector_size = 1;
        let mut sectors = 2;

        super::write_capacity_outputs(capacity, &mut sector_size, core::ptr::null_mut());
        assert_eq!(sector_size, 2048);
        assert_eq!(sectors, 2);

        super::write_capacity_outputs(capacity, core::ptr::null_mut(), &mut sectors);
        assert_eq!(sector_size, 2048);
        assert_eq!(sectors, 1234);

        super::write_capacity_outputs(capacity, core::ptr::null_mut(), core::ptr::null_mut());
    }

    #[test]
    fn read_capacity_preserves_negative_cycle_result() {
        assert_eq!(
            super::decode_read_capacity(USBSTORAGE_ETIMEDOUT, 1, [0; 8]).result,
            USBSTORAGE_ETIMEDOUT
        );
    }

    #[test]
    fn nonzero_csw_residue_does_not_change_cycle_result() {
        assert_eq!(super::bot_csw_cycle_result(USBSTORAGE_OK, 1), USBSTORAGE_OK);
        assert_eq!(super::bot_csw_cycle_result(-41, 1), -41);
    }

    #[test]
    fn read_write_status_matches_c_positive_result_rule() {
        assert_eq!(super::read_write_cycle_result(0, 1), 0);
        assert_eq!(super::read_write_cycle_result(1, 1), USBSTORAGE_ESTATUS);
        assert_eq!(super::read_write_cycle_result(1, 0), 1);
        assert_eq!(super::read_write_cycle_result(-41, 1), -41);
    }

    #[test]
    fn open_reset_decision_matches_c_fd_and_packet_size_condition() {
        assert!(!super::open_requires_bot_reset(0x20, 512));
        assert!(super::open_requires_bot_reset(0x20, 64));
        assert!(super::open_requires_bot_reset(0x1f, 512));
        assert!(!super::open_requires_bot_reset(-2, 512));
        assert!(super::open_requires_bot_reset(-1, 512));
    }

    #[test]
    fn max_lun_response_maps_to_representable_count() {
        assert_eq!(super::lun_count_from_reported_max(0), Some(1));
        assert_eq!(super::lun_count_from_reported_max(7), Some(8));
        assert_eq!(super::lun_count_from_reported_max(8), Some(9));
        assert_eq!(super::lun_count_from_reported_max(254), Some(255));
        assert_eq!(super::lun_count_from_reported_max(255), None);
    }

    #[test]
    fn abort_open_ignores_close_failure_and_preserves_original_error() {
        let mut handle = UsbStorageHandle::default();
        handle.usb_fd = 42;
        handle.configuration = 1;
        handle.max_lun = 2;
        let mut close_calls = 0;

        let result = unsafe {
            abort_open_with(&mut handle, USBSTORAGE_ETIMEDOUT, |fd| {
                close_calls += 1;
                assert_eq!(*fd, 42);
                -41
            })
        };

        assert_eq!(result, USBSTORAGE_ETIMEDOUT);
        assert_eq!(close_calls, 1);
        assert_eq!(handle.usb_fd, -1);
        assert_eq!(handle.configuration, 0);
        assert_eq!(handle.max_lun, 0);
    }

    #[test]
    fn mount_lun_uses_read_capacity_result_after_clear_reset_and_inquiry_errors() {
        use core::cell::Cell;

        let delay_calls = Cell::new(0);
        let clear_calls = Cell::new(0);
        let reset_calls = Cell::new(0);
        let inquiry_calls = Cell::new(0);
        let capacity_calls = Cell::new(0);

        let (result, sector_size, sectors) = mount_lun_with(
            || delay_calls.set(delay_calls.get() + 1),
            || {
                clear_calls.set(clear_calls.get() + 1);
                -41
            },
            || {
                reset_calls.set(reset_calls.get() + 1);
                -42
            },
            || {
                inquiry_calls.set(inquiry_calls.get() + 1);
                -43
            },
            |sector_size, sectors| {
                capacity_calls.set(capacity_calls.get() + 1);
                *sector_size = 512;
                *sectors = 100;
                USBSTORAGE_OK
            },
        );

        assert_eq!(result, USBSTORAGE_OK);
        assert_eq!(sector_size, 512);
        assert_eq!(sectors, 100);
        assert_eq!(delay_calls.get(), 1);
        assert_eq!(clear_calls.get(), 2);
        assert_eq!(reset_calls.get(), 1);
        assert_eq!(inquiry_calls.get(), 1);
        assert_eq!(capacity_calls.get(), 1);
    }

    #[test]
    fn mount_lun_returns_final_capacity_error_and_invalid_lun_result() {
        let (error, _, _) = mount_lun_with(
            || {},
            || USBSTORAGE_OK,
            || USBSTORAGE_OK,
            || USBSTORAGE_OK,
            |_, _| -44,
        );
        assert_eq!(error, -44);

        let (invalid, sector_size, sectors) = mount_lun_with(
            || {},
            || USBSTORAGE_OK,
            || USBSTORAGE_OK,
            || USBSTORAGE_OK,
            |sector_size, sectors| {
                *sector_size = 256;
                *sectors = 1;
                USBSTORAGE_OK
            },
        );
        assert_eq!(invalid, USBSTORAGE_INVALID_LUN);
        assert_eq!(sector_size, 256);
        assert_eq!(sectors, 1);
    }

    #[test]
    fn get_max_lun_timeout_aborts_but_other_errors_keep_lun_zero() {
        assert_eq!(
            super::lun_count_after_get_max_lun(Err(USBSTORAGE_ETIMEDOUT)),
            Err(USBSTORAGE_ETIMEDOUT)
        );
        assert_eq!(super::lun_count_after_get_max_lun(Err(-1)), Ok(1));
        assert_eq!(super::lun_count_after_get_max_lun(Ok(0)), Ok(1));
        assert_eq!(
            super::lun_count_after_get_max_lun(Ok(255)),
            Err(USBSTORAGE_ENOINTERFACE)
        );
    }

    #[test]
    fn start_stop_uses_c_timeout_selection() {
        assert_eq!(
            start_stop_csw_timeout_ticks(1),
            USBSTORAGE_CONTROL_TIMEOUT_TICKS
        );
        assert_eq!(
            start_stop_csw_timeout_ticks(0),
            USBSTORAGE_START_STOP_TIMEOUT_TICKS
        );
        assert_eq!(
            USBSTORAGE_START_STOP_TIMEOUT_TICKS,
            10 * lwp::TICKS_PER_SECOND
        );
    }

    #[test]
    fn bot_reset_control_uses_one_second_timeout() {
        assert_eq!(BOT_RESET_CONTROL_TIMEOUT_TICKS, lwp::TICKS_PER_SECOND);
    }

    #[test]
    fn mounted_non_dvd_does_not_probe_media_again() {
        use core::cell::Cell;

        let probed = Cell::new(false);
        assert!(mounted_media_is_inserted(512, || {
            probed.set(true);
            false
        }));
        assert!(!probed.get());
    }

    #[test]
    fn mounted_dvd_uses_capacity_probe_result() {
        assert!(mounted_media_is_inserted(2048, || true));
        assert!(!mounted_media_is_inserted(2048, || false));
    }

    #[test]
    fn removal_callback_clears_disc_mounted_state() {
        DISC_MOUNTED.store(true, Ordering::Release);
        record_storage_device_removed();
        assert!(!DISC_MOUNTED.load(Ordering::Acquire));
    }

    #[test]
    fn production_deinitialization_preserves_state_when_close_fails() {
        use core::cell::Cell;

        let initialized = Cell::new(true);
        let mounted = Cell::new(true);
        let capacity_valid = Cell::new(true);
        let cleared = Cell::new(false);
        let mut clear_calls = 0;

        assert_eq!(
            deinitialize_with(
                || {
                    clear_calls += 1;
                    -41
                },
                || {
                    initialized.set(false);
                    mounted.set(false);
                    capacity_valid.set(false);
                    cleared.set(true);
                },
            ),
            -41
        );
        assert_eq!(clear_calls, 1);
        assert!(initialized.get());
        assert!(mounted.get());
        assert!(capacity_valid.get());
        assert!(!cleared.get());
    }

    #[test]
    fn production_deinitialization_clears_state_after_close_succeeds() {
        use core::cell::Cell;

        let initialized = Cell::new(true);
        let mounted = Cell::new(true);
        let capacity_valid = Cell::new(true);
        let cleared = Cell::new(false);

        assert_eq!(
            deinitialize_with(
                || USBSTORAGE_OK,
                || {
                    initialized.set(false);
                    mounted.set(false);
                    capacity_valid.set(false);
                    cleared.set(true);
                },
            ),
            USBSTORAGE_OK
        );
        assert!(!initialized.get());
        assert!(!mounted.get());
        assert!(!capacity_valid.get());
        assert!(cleared.get());
    }

    #[test]
    fn shutdown_reports_close_failure_after_clearing_disc_state() {
        DISC_MOUNTED.store(true, Ordering::Release);
        USBSTORAGE_CAPACITY_VALID.store(true, Ordering::Release);
        let mut close_calls = 0;

        assert!(!shutdown_with(|| {
            close_calls += 1;
            -41
        }));
        assert_eq!(close_calls, 1);
        assert!(!DISC_MOUNTED.load(Ordering::Acquire));
        assert!(!USBSTORAGE_CAPACITY_VALID.load(Ordering::Acquire));
    }

    #[test]
    fn transfer_guard_covers_reset_operation_and_releases_afterward() {
        use core::cell::Cell;

        struct TestGuard<'a>(&'a Cell<bool>);
        impl Drop for TestGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }

        let held = Cell::new(false);
        let result = with_transfer_guard(
            || {
                held.set(true);
                Ok(TestGuard(&held))
            },
            || {
                assert!(
                    held.get(),
                    "reset must run while the transfer guard is held"
                );
                -41
            },
        );

        assert_eq!(result, -41);
        assert!(!held.get(), "transfer guard must be released after reset");
    }
}

unsafe fn allocate_handle_storage(dev: &mut UsbStorageHandle) -> i32 {
    if dev.max_lun == 0 {
        return USBSTORAGE_EINVAL;
    }
    let sector_layout = match Layout::array::<u32>(dev.max_lun as usize) {
        Ok(layout) => layout,
        Err(_) => return USBSTORAGE_EINIT,
    };
    let buffer_layout = match Layout::from_size_align(MAX_TRANSFER_SIZE, 32) {
        Ok(layout) => layout,
        Err(_) => return USBSTORAGE_EINIT,
    };

    let sector_size = unsafe { alloc_zeroed(sector_layout) }.cast::<u32>();
    if sector_size.is_null() {
        return USBSTORAGE_EINIT;
    }
    let buffer = unsafe { alloc_zeroed(buffer_layout) };
    if buffer.is_null() {
        unsafe { dealloc(sector_size.cast(), sector_layout) };
        return USBSTORAGE_EINIT;
    }

    dev.sector_size = sector_size;
    dev.buffer = buffer;
    USBSTORAGE_OK
}

unsafe fn release_handle_storage(dev: &mut UsbStorageHandle) {
    if !dev.sector_size.is_null()
        && dev.max_lun != 0
        && let Ok(layout) = Layout::array::<u32>(dev.max_lun as usize)
    {
        unsafe { dealloc(dev.sector_size.cast(), layout) };
    }
    if !dev.buffer.is_null()
        && let Ok(layout) = Layout::from_size_align(MAX_TRANSFER_SIZE, 32)
    {
        unsafe { dealloc(dev.buffer, layout) };
    }
    dev.sector_size = ptr::null_mut();
    dev.buffer = ptr::null_mut();
}

unsafe fn release_handle_resources(dev: &mut UsbStorageHandle) {
    unsafe { release_handle_storage(dev) };
    if dev.alarm != 0 {
        let _ = unsafe { sys_remove_alarm(dev.alarm) };
        dev.alarm = 0;
    }
    if dev.lock != lwp::LWP_MUTEX_NULL {
        let _ = lwp::mutex_destroy(dev.lock);
        dev.lock = lwp::LWP_MUTEX_NULL;
    }
}

unsafe fn cleanup_closed_handle(dev: *mut UsbStorageHandle) {
    unsafe {
        release_handle_resources(&mut *dev);
        *dev = UsbStorageHandle::default();
        (*dev).usb_fd = -1;
    }
    clear_handle_state(dev);
}

unsafe fn handle_sector_size(dev: *const UsbStorageHandle, lun: u8) -> u32 {
    if dev.is_null() || unsafe { (*dev).sector_size.is_null() || lun >= (*dev).max_lun } {
        0
    } else {
        unsafe { (*dev).sector_size.add(lun as usize).read() }
    }
}

unsafe fn set_handle_sector_size(dev: &mut UsbStorageHandle, lun: u8, value: u32) {
    if !dev.sector_size.is_null() && lun < dev.max_lun {
        unsafe { dev.sector_size.add(lun as usize).write(value) };
    }
}

unsafe fn set_last_capacity(lun: u8, sector_size: u32, sectors: u32) {
    USBSTORAGE_LAST_SECTOR_SIZE.store(sector_size, Ordering::Release);
    USBSTORAGE_LAST_NUM_SECTORS.store(sectors, Ordering::Release);
    USBSTORAGE_LAST_LUN.store(lun as u32, Ordering::Release);
    USBSTORAGE_CAPACITY_VALID.store(true, Ordering::Release);
}

pub fn initialize() -> i32 {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if !is_wii_hardware() {
        return USBSTORAGE_EINIT;
    }
    if USBSTORAGE_INITED.load(Ordering::Acquire) {
        return USBSTORAGE_OK;
    }
    USBSTORAGE_INITED.store(true, Ordering::Release);
    USBSTORAGE_MOUNTED.store(false, Ordering::Release);
    USBSTORAGE_CAPACITY_VALID.store(false, Ordering::Release);
    USBSTORAGE_LAST_SECTOR_SIZE.store(512, Ordering::Release);
    USBSTORAGE_LAST_NUM_SECTORS.store(0, Ordering::Release);
    USBSTORAGE_LAST_LUN.store(0, Ordering::Release);
    USBSTORAGE_LAST_DEVICE_ID.store(-1, Ordering::Release);
    USBSTORAGE_LAST_RAW_WRITE.store(false, Ordering::Release);
    reset_global_disc_state();
    USBSTORAGE_OK
}

fn deinitialize_with(mut close_device: impl FnMut() -> i32, clear_state: impl FnOnce()) -> i32 {
    let close_result = close_device();
    // Keep the subsystem initialized while a failed close leaves a live
    // transport handle. The caller can retry deinitialization without losing
    // the state needed to close that handle safely.
    if close_result < 0 {
        return close_result;
    }
    clear_state();
    USBSTORAGE_OK
}

pub fn deinitialize() -> i32 {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    deinitialize_with(
        || {
            let mut first_error = USBSTORAGE_OK;
            for drive in &DRIVES {
                let result = unsafe {
                    if (*drive.handle.get()).usb_fd >= 0 {
                        close(drive.handle.get())
                    } else {
                        USBSTORAGE_OK
                    }
                };
                if result < 0 && first_error == USBSTORAGE_OK {
                    first_error = result;
                }
            }
            first_error
        },
        || {
            USBSTORAGE_INITED.store(false, Ordering::Release);
            USBSTORAGE_MOUNTED.store(false, Ordering::Release);
            USBSTORAGE_CAPACITY_VALID.store(false, Ordering::Release);
            reset_global_disc_state();
        },
    )
}

unsafe fn find_mass_storage_interface(dev: &mut UsbStorageHandle, fd: i32) -> Result<bool, i32> {
    unsafe {
        let mut udd = usb::UsbDevDesc::default();
        let rc = usb::get_descriptors(fd, &mut udd);
        if rc < 0 {
            return Err(rc);
        }

        if !udd.configurations.is_null() {
            for conf_idx in 0..udd.b_num_configurations as usize {
                let ucd = &*udd.configurations.add(conf_idx);
                if ucd.interfaces.is_null() {
                    continue;
                }
                for if_idx in 0..ucd.b_num_interfaces as usize {
                    let uid = &*ucd.interfaces.add(if_idx);
                    if uid.b_interface_class != USB_CLASS_MASS_STORAGE
                        || uid.b_interface_protocol != MASS_STORAGE_BULK_ONLY
                        || uid.b_num_endpoints < 2
                        || uid.endpoints.is_null()
                    {
                        continue;
                    }

                    let mut ep_in = 0u8;
                    let mut ep_out = 0u8;
                    let mut bulk_out_max_packet_size = 0u16;
                    for ep_idx in 0..uid.b_num_endpoints as usize {
                        let ued = &*uid.endpoints.add(ep_idx);
                        if ued.bm_attributes != USB_ENDPOINT_BULK {
                            continue;
                        }
                        if (ued.b_endpoint_address & usb::USB_ENDPOINT_IN) != 0 {
                            ep_in = ued.b_endpoint_address;
                        } else {
                            ep_out = ued.b_endpoint_address;
                            bulk_out_max_packet_size = ued.w_max_packet_size;
                        }
                    }

                    if ep_in != 0 && ep_out != 0 {
                        dev.configuration = ucd.b_configuration_value;
                        dev.interface = uid.b_interface_number as u32;
                        dev.alt_interface = uid.b_alternate_setting as u32;
                        dev.b_interface_sub_class = uid.b_interface_sub_class;
                        dev.ep_in = ep_in;
                        dev.ep_out = ep_out;
                        let reset_required =
                            super::open_requires_bot_reset(fd, bulk_out_max_packet_size);
                        if let Some(drive) = drive_for_handle(dev) {
                            drive.usb2_mode.store(!reset_required, Ordering::Release);
                        } else {
                            USB2_MODE.store(!reset_required, Ordering::Release);
                        }
                        usb::free_descriptors(&mut udd);
                        return Ok(reset_required);
                    }
                }
            }
        }

        usb::free_descriptors(&mut udd);
        Err(USBSTORAGE_ENOINTERFACE)
    }
}

unsafe fn class_get_max_lun(dev: &mut UsbStorageHandle) -> Result<u8, i32> {
    unsafe {
        let _transfer_guard = TransferGuard::acquire(dev.lock)?;
        let control = allocate_async_control(0u8)?;
        let rc = usb::read_ctrl_msg_async(
            usb::UsbControlRequest {
                fd: dev.usb_fd,
                request_type: usb::USB_CTRLTYPE_DIR_DEVICE2HOST
                    | usb::USB_CTRLTYPE_TYPE_CLASS
                    | usb::USB_CTRLTYPE_REC_INTERFACE,
                request: USBSTORAGE_GET_MAX_LUN,
                value: 0,
                index: dev.interface as u16,
                length: 1,
                data: (*control).data.0.get().cast(),
            },
            usb::UsbAsyncCompletion {
                callback: Some(get_max_lun_control_complete),
                userdata: control.cast(),
            },
        );
        if rc < 0 {
            deallocate_async_control(control);
            return Err(rc);
        }

        let (result, value) = wait_async_control(control, USBSTORAGE_CONTROL_TIMEOUT_TICKS)?;
        if result < 0 { Err(result) } else { Ok(value) }
    }
}

unsafe extern "C" fn reset_control_complete(result: i32, userdata: *mut c_void) -> i32 {
    unsafe { complete_async_control(userdata.cast::<AsyncControl<()>>(), result) };
    0
}

unsafe extern "C" fn get_max_lun_control_complete(result: i32, userdata: *mut c_void) -> i32 {
    unsafe { complete_async_control(userdata.cast::<AsyncControl<u8>>(), result) };
    0
}

fn start_stop_csw_timeout_ticks(immediate: u8) -> u64 {
    if immediate & 1 != 0 {
        USBSTORAGE_CONTROL_TIMEOUT_TICKS
    } else {
        USBSTORAGE_START_STOP_TIMEOUT_TICKS
    }
}

unsafe extern "C" fn start_stop_csw_complete(result: i32, userdata: *mut c_void) -> i32 {
    unsafe { complete_async_control(userdata.cast::<AsyncControl<[u8; CSW_SIZE]>>(), result) };
    0
}

unsafe fn read_start_stop_csw(
    dev: &mut UsbStorageHandle,
    status: &mut u8,
    timeout_ticks: u64,
) -> i32 {
    let control = match allocate_async_control([0u8; CSW_SIZE]) {
        Ok(control) => control,
        Err(error) => return error,
    };

    let rc = unsafe {
        usb::read_blk_msg_async(
            dev.usb_fd,
            dev.ep_in,
            CSW_SIZE as u16,
            (*control).data.0.get().cast(),
            Some(start_stop_csw_complete),
            control.cast(),
        )
    };
    if rc < 0 {
        unsafe { deallocate_async_control(control) };
        return rc;
    }

    let (rc, csw) = match unsafe { wait_async_control(control, timeout_ticks) } {
        Ok(result) => result,
        Err(error) => {
            let clear_result = usb::clear_halt(dev.usb_fd, dev.ep_in);
            return if clear_result < 0 {
                clear_result
            } else {
                error
            };
        }
    };

    if rc > 0 && rc != CSW_SIZE as i32 {
        return USBSTORAGE_ESHORTREAD;
    }
    if rc < 0 {
        let clear_result = usb::clear_halt(dev.usb_fd, dev.ep_in);
        return if clear_result < 0 { clear_result } else { rc };
    }

    let signature = u32::from_le_bytes(csw[0..4].try_into().unwrap());
    let tag = u32::from_le_bytes(csw[4..8].try_into().unwrap());
    *status = csw[12];
    if signature != CSW_SIGNATURE {
        return USBSTORAGE_ESIGNATURE;
    }
    if tag != dev.tag {
        return USBSTORAGE_ETAG;
    }
    USBSTORAGE_OK
}

unsafe fn bot_reset_control_with_timeout(dev: &UsbStorageHandle, timeout_ticks: u64) -> i32 {
    let control = match allocate_async_control(()) {
        Ok(control) => control,
        Err(error) => return error,
    };
    let rc = unsafe {
        usb::write_ctrl_msg_async(
            usb::UsbControlRequest {
                fd: dev.usb_fd,
                request_type: usb::USB_CTRLTYPE_DIR_HOST2DEVICE
                    | usb::USB_CTRLTYPE_TYPE_CLASS
                    | usb::USB_CTRLTYPE_REC_INTERFACE,
                request: USBSTORAGE_RESET,
                value: 0,
                index: dev.interface as u16,
                length: 0,
                data: ptr::null_mut(),
            },
            usb::UsbAsyncCompletion {
                callback: Some(reset_control_complete),
                userdata: control.cast(),
            },
        )
    };
    if rc < 0 {
        unsafe { deallocate_async_control(control) };
        return rc;
    }
    match unsafe { wait_async_control(control, timeout_ticks) } {
        Ok((result, ())) => result,
        Err(error) => error,
    }
}

unsafe fn mass_storage_reset(dev: &mut UsbStorageHandle) -> i32 {
    if let Some(error) = pending_bulk_error(unsafe { handle_retval_atomic(dev) }) {
        return error;
    }
    run_bot_reset_sequence(|step| match step {
        BotResetStep::ClassReset => unsafe {
            bot_reset_control_with_timeout(dev, BOT_RESET_CONTROL_TIMEOUT_TICKS)
        },
        BotResetStep::ClearInHalt => usb::clear_halt(dev.usb_fd, dev.ep_in),
        BotResetStep::ClearOutHalt => usb::clear_halt(dev.usb_fd, dev.ep_out),
        BotResetStep::DelayMs(milliseconds) => {
            let ticks = lwp::TICKS_PER_SECOND.saturating_mul(milliseconds) / 1000;
            lwp::sleep_ticks(ticks);
            USBSTORAGE_OK
        }
    })
}

unsafe fn handle_retval_atomic(dev: *mut UsbStorageHandle) -> &'static AtomicI32 {
    unsafe { &*core::ptr::addr_of_mut!((*dev).retval).cast::<AtomicI32>() }
}

unsafe extern "C" fn bulk_message_complete(result: i32, userdata: *mut c_void) -> i32 {
    let dev = userdata.cast::<UsbStorageHandle>();
    if dev.is_null() {
        return USBSTORAGE_EINVAL;
    }
    unsafe {
        close_policy::bulk::complete(handle_retval_atomic(dev), result, || {
            let cancel_result = sys_cancel_alarm((*dev).alarm);
            if cancel_result < 0 {
                super::startup::record(ProbeStage::AsyncCleanup, cancel_result);
            }
            cancel_result
        });
    }
    USBSTORAGE_OK
}

unsafe extern "C" fn bulk_message_timeout(_alarm: u32, userdata: *mut c_void) {
    let dev = userdata.cast::<UsbStorageHandle>();
    if !dev.is_null() {
        unsafe { close_policy::bulk::timeout(handle_retval_atomic(dev)) };
    }
}

fn timeout_timespec(timeout_ticks: u64) -> TimeSpec {
    let seconds = timeout_ticks / lwp::TICKS_PER_SECOND;
    let remainder = timeout_ticks % lwp::TICKS_PER_SECOND;
    TimeSpec {
        tv_sec: seconds as i64,
        tv_nsec: ((remainder * 1_000_000_000) / lwp::TICKS_PER_SECOND) as i32,
    }
}

fn wait_for_submitted_request_after_alarm_error(
    alarm_error: i32,
    mut result: impl FnMut() -> i32,
    mut yield_thread: impl FnMut(),
) -> i32 {
    while result() == USBSTORAGE_PROCESSING {
        yield_thread();
    }
    alarm_error
}

unsafe fn bulk_message_with_timeout(
    dev: &mut UsbStorageHandle,
    endpoint: u8,
    length: u16,
    data: *mut c_void,
    write: bool,
    timeout_ticks: u64,
) -> i32 {
    let dev_ptr = dev as *mut UsbStorageHandle;
    let retval = unsafe { handle_retval_atomic(dev_ptr) };
    if close_policy::bulk::begin(retval).is_err() {
        return pending_bulk_error(retval).unwrap_or(USBSTORAGE_EINIT);
    }
    let userdata = dev_ptr.cast::<c_void>();
    let submit = unsafe {
        if write {
            usb::write_blk_msg_async(
                dev.usb_fd,
                endpoint,
                length,
                data,
                Some(bulk_message_complete),
                userdata,
            )
        } else {
            usb::read_blk_msg_async(
                dev.usb_fd,
                endpoint,
                length,
                data,
                Some(bulk_message_complete),
                userdata,
            )
        }
    };
    if submit < 0 {
        retval.store(submit, Ordering::Release);
        return submit;
    }

    let timeout = timeout_timespec(timeout_ticks);
    let alarm_result =
        unsafe { sys_set_alarm(dev.alarm, &timeout, Some(bulk_message_timeout), userdata) };
    if alarm_result < 0 {
        if !close_policy::bulk::mark_no_alarm(retval) {
            if let Some(result) = bulk_result(retval) {
                return result;
            }
        }
        return wait_for_submitted_request_after_alarm_error(
            alarm_result,
            || bulk_result(retval).unwrap_or(USBSTORAGE_PROCESSING),
            lwp::yield_thread,
        );
    }

    let alarm_state = close_policy::bulk::arm_alarm(retval, || unsafe {
        let cancel_result = sys_cancel_alarm(dev.alarm);
        if cancel_result < 0 {
            super::startup::record(ProbeStage::AsyncCleanup, cancel_result);
        }
        cancel_result
    });
    if matches!(
        alarm_state,
        close_policy::bulk::State::AlarmCancelFailed {
            alarm_pending: true
        }
    ) {
        return USBSTORAGE_EINIT;
    }

    loop {
        if let Some(result) = bulk_result(retval) {
            let state = close_policy::bulk::classify(retval.load(Ordering::Acquire));
            if matches!(state, close_policy::bulk::State::AlarmCancelFailed { .. }) {
                return USBSTORAGE_EINIT;
            }
            if matches!(state, close_policy::bulk::State::TimedOut { .. }) {
                let clear_result = usb::clear_halt(dev.usb_fd, endpoint);
                return if clear_result < 0 {
                    clear_result
                } else {
                    USBSTORAGE_ETIMEDOUT
                };
            }
            if result < 0 {
                let clear_result = usb::clear_halt(dev.usb_fd, endpoint);
                return if clear_result < 0 {
                    clear_result
                } else {
                    result
                };
            }
            return result;
        }
        lwp::yield_thread();
    }
}

unsafe fn send_cbw(
    dev: &mut UsbStorageHandle,
    lun: u8,
    len: u32,
    flags: u8,
    cb: &[u8],
    timeout_ticks: u64,
) -> i32 {
    unsafe {
        if cb.is_empty() || cb.len() > 16 {
            return USBSTORAGE_EINVAL;
        }
        if dev.buffer.is_null() {
            return USBSTORAGE_EINIT;
        }
        let cbw = { core::slice::from_raw_parts_mut(dev.buffer, CBW_SIZE) };
        cbw.fill(0);
        cbw[0..4].copy_from_slice(&CBW_SIGNATURE.to_le_bytes());
        dev.tag = dev.tag.wrapping_add(1);
        cbw[4..8].copy_from_slice(&dev.tag.to_le_bytes());
        cbw[8..12].copy_from_slice(&len.to_le_bytes());
        cbw[12] = flags;
        cbw[13] = lun;
        cbw[14] = if cb.len() > 6 { 10 } else { 6 };
        cbw[15..15 + cb.len()].copy_from_slice(cb);

        if dev.suspended == 1 {
            let _ = usb::resume_device(dev.usb_fd);
            dev.suspended = 0;
        }

        let rc = bulk_message_with_timeout(
            dev,
            dev.ep_out,
            CBW_SIZE as u16,
            dev.buffer.cast(),
            true,
            timeout_ticks,
        );
        if rc == CBW_SIZE as i32 {
            USBSTORAGE_OK
        } else if rc > 0 {
            USBSTORAGE_ESHORTWRITE
        } else {
            rc
        }
    }
}

unsafe fn read_csw(
    dev: &mut UsbStorageHandle,
    status: &mut u8,
    residue: &mut u32,
    timeout_ticks: u64,
) -> i32 {
    if dev.buffer.is_null() {
        return USBSTORAGE_EINIT;
    }
    unsafe { ptr::write_bytes(dev.buffer, 0, CSW_SIZE) };
    let rc = unsafe {
        bulk_message_with_timeout(
            dev,
            dev.ep_in,
            CSW_SIZE as u16,
            dev.buffer.cast(),
            false,
            timeout_ticks,
        )
    };
    if rc > 0 && rc != CSW_SIZE as i32 {
        return USBSTORAGE_ESHORTREAD;
    }
    if rc < 0 {
        return rc;
    }

    let csw = unsafe { core::slice::from_raw_parts(dev.buffer, CSW_SIZE) };
    let signature = u32::from_le_bytes(csw[0..4].try_into().unwrap());
    let tag = u32::from_le_bytes(csw[4..8].try_into().unwrap());
    *residue = u32::from_le_bytes(csw[8..12].try_into().unwrap());
    *status = csw[12];

    if signature != CSW_SIGNATURE {
        return USBSTORAGE_ESIGNATURE;
    }
    if tag != dev.tag {
        return USBSTORAGE_ETAG;
    }
    USBSTORAGE_OK
}

unsafe fn transfer_data(
    dev: &mut UsbStorageHandle,
    endpoint: u8,
    buf: *mut u8,
    len: usize,
    write: bool,
    timeout_ticks: u64,
) -> i32 {
    unsafe {
        if len == 0 {
            return USBSTORAGE_OK;
        }
        let mut offset = 0usize;
        let bounce = dev.buffer;
        while offset < len {
            let max_transfer_size = transfer_chunk_size(drive_for_handle(dev).map_or_else(
                || USB2_MODE.load(Ordering::Acquire),
                |drive| drive.usb2_mode.load(Ordering::Acquire),
            ));
            let chunk = (len - offset).min(max_transfer_size);
            if write {
                ptr::copy_nonoverlapping(buf.add(offset), bounce, chunk);
                let rc = {
                    bulk_message_with_timeout(
                        dev,
                        endpoint,
                        chunk as u16,
                        bounce.cast(),
                        true,
                        timeout_ticks,
                    )
                };
                let result = super::transfer_result(rc, chunk);
                if result != USBSTORAGE_OK {
                    return result;
                }
            } else {
                let rc = {
                    bulk_message_with_timeout(
                        dev,
                        endpoint,
                        chunk as u16,
                        bounce.cast(),
                        false,
                        timeout_ticks,
                    )
                };
                let source = core::slice::from_raw_parts(bounce, chunk);
                let destination = core::slice::from_raw_parts_mut(buf.add(offset), chunk);
                let result = copy_read_result(source, destination, rc);
                if result < 0 {
                    return result;
                }
            }
            offset += chunk;
        }
        USBSTORAGE_OK
    }
}

#[derive(Copy, Clone)]
enum ScsiDataDirection {
    DeviceToHost,
    HostToDevice,
}

impl ScsiDataDirection {
    const fn is_write(self) -> bool {
        matches!(self, Self::HostToDevice)
    }
}

struct CycleRequest<'a> {
    lun: u8,
    data: *mut u8,
    data_len: u32,
    command: &'a [u8],
    direction: ScsiDataDirection,
}

struct CycleOutcome {
    result: i32,
    status: u8,
    residue: u32,
}

unsafe fn cycle(dev: &mut UsbStorageHandle, request: CycleRequest<'_>) -> CycleOutcome {
    unsafe { cycle_with_timeout(dev, request, USBSTORAGE_CONTROL_TIMEOUT_TICKS) }
}

unsafe fn cycle_with_timeout(
    dev: &mut UsbStorageHandle,
    request: CycleRequest<'_>,
    timeout_ticks: u64,
) -> CycleOutcome {
    let _transfer_guard = match TransferGuard::acquire(dev.lock) {
        Ok(guard) => guard,
        Err(error) => {
            return CycleOutcome {
                result: error,
                status: 0,
                residue: 0,
            };
        }
    };
    unsafe { cycle_locked_with_timeout(dev, request, timeout_ticks) }
}

unsafe fn cycle_locked(dev: &mut UsbStorageHandle, request: CycleRequest<'_>) -> CycleOutcome {
    unsafe { cycle_locked_with_timeout(dev, request, USBSTORAGE_CONTROL_TIMEOUT_TICKS) }
}

unsafe fn cycle_locked_with_timeout(
    dev: &mut UsbStorageHandle,
    request: CycleRequest<'_>,
    timeout_ticks: u64,
) -> CycleOutcome {
    unsafe {
        let write = request.direction.is_write();
        let endpoint = if write { dev.ep_out } else { dev.ep_in };
        let mut outcome = CycleOutcome {
            result: USBSTORAGE_ESTATUS,
            status: 0,
            residue: 0,
        };
        for _ in 0..=USBSTORAGE_CYCLE_RETRIES {
            outcome.result = send_cbw(
                dev,
                request.lun,
                request.data_len,
                if write { CBW_OUT } else { CBW_IN },
                request.command,
                timeout_ticks,
            );
            if outcome.result < 0 {
                let attempt_result = outcome.result;
                let reset_rc = mass_storage_reset(dev);
                if let Some(error) = bot_cycle_retry_error(attempt_result, reset_rc) {
                    outcome.result = error;
                    return outcome;
                }
                outcome.result = attempt_result;
                continue;
            }

            outcome.result = transfer_data(
                dev,
                endpoint,
                request.data,
                request.data_len as usize,
                write,
                timeout_ticks,
            );
            if outcome.result < 0 {
                let attempt_result = outcome.result;
                let reset_rc = mass_storage_reset(dev);
                if let Some(error) = bot_cycle_retry_error(attempt_result, reset_rc) {
                    outcome.result = error;
                    return outcome;
                }
                outcome.result = attempt_result;
                continue;
            }

            outcome.result = read_csw(
                dev,
                &mut outcome.status,
                &mut outcome.residue,
                timeout_ticks,
            );
            if outcome.result < 0 {
                let attempt_result = outcome.result;
                let reset_rc = mass_storage_reset(dev);
                if let Some(error) = bot_cycle_retry_error(attempt_result, reset_rc) {
                    outcome.result = error;
                    return outcome;
                }
                outcome.result = attempt_result;
                continue;
            }
            outcome.result = super::bot_csw_cycle_result(outcome.result, outcome.residue);
            return outcome;
        }
        outcome
    }
}

unsafe fn clear_errors_with_timeout(
    dev: &mut UsbStorageHandle,
    lun: u8,
    timeout_ticks: u64,
) -> i32 {
    unsafe {
        let cmd = [SCSI_TEST_UNIT_READY];
        let outcome = cycle_with_timeout(
            dev,
            CycleRequest {
                lun,
                data: ptr::null_mut(),
                data_len: 0,
                command: &cmd,
                direction: ScsiDataDirection::DeviceToHost,
            },
            timeout_ticks,
        );
        if outcome.result < 0 {
            return outcome.result;
        }

        if outcome.status != 0 {
            let mut sense = [0u8; SCSI_SENSE_REPLY_SIZE];
            let sense_cmd = [
                SCSI_REQUEST_SENSE,
                lun << 5,
                0,
                0,
                SCSI_SENSE_REPLY_SIZE as u8,
                0,
            ];
            let outcome = cycle_with_timeout(
                dev,
                CycleRequest {
                    lun,
                    data: sense.as_mut_ptr(),
                    data_len: sense.len() as u32,
                    command: &sense_cmd,
                    direction: ScsiDataDirection::DeviceToHost,
                },
                timeout_ticks,
            );
            if outcome.result >= 0 {
                match sense[2] & 0x0f {
                    SCSI_SENSE_NOT_READY => return USBSTORAGE_EINIT,
                    SCSI_SENSE_MEDIUM_ERROR | SCSI_SENSE_HARDWARE_ERROR => {
                        return USBSTORAGE_ESENSE;
                    }
                    _ => {}
                }
            }
            return outcome.result;
        }

        USBSTORAGE_OK
    }
}

pub unsafe fn inquiry(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe {
        if !is_wii_hardware() {
            return USBSTORAGE_ENOINTERFACE;
        }
        if dev.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }
        let dev_ref = &mut *dev;
        if dev_ref.max_lun == 0 || lun >= dev_ref.max_lun {
            return USBSTORAGE_EINVAL;
        }
        if dev_ref.buffer.is_null() || dev_ref.sector_size.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }
        inquiry_inner(dev_ref, lun)
    }
}

unsafe fn inquiry_inner(dev: &mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { inquiry_inner_with_timeout(dev, lun, USBSTORAGE_CONTROL_TIMEOUT_TICKS) }
}

unsafe fn inquiry_inner_with_timeout(
    dev: &mut UsbStorageHandle,
    lun: u8,
    timeout_ticks: u64,
) -> i32 {
    let cmd = [SCSI_INQUIRY, lun << 5, 0, 0, 36, 0];
    inquiry_with_cycles(|response| {
        let outcome = unsafe {
            cycle_with_timeout(
                dev,
                CycleRequest {
                    lun,
                    data: response.as_mut_ptr(),
                    data_len: response.len() as u32,
                    command: &cmd,
                    direction: ScsiDataDirection::DeviceToHost,
                },
                timeout_ticks,
            )
        };
        outcome.result
    })
}

fn inquiry_with_cycles(mut attempt: impl FnMut(&mut [u8; 36]) -> i32) -> i32 {
    let mut response = [0u8; 36];
    let mut result = USBSTORAGE_ESTATUS;
    for _ in 0..2 {
        response.fill(0);
        result = attempt(&mut response);
        if result >= 0 {
            return (response[0] & 0x1f) as i32;
        }
    }
    result
}

unsafe fn read_capacity_inner(
    dev: &mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    unsafe {
        read_capacity_inner_with_timeout(
            dev,
            lun,
            sector_size,
            n_sectors,
            USBSTORAGE_CONTROL_TIMEOUT_TICKS,
        )
    }
}

unsafe fn read_capacity_inner_with_timeout(
    dev: &mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
    timeout_ticks: u64,
) -> i32 {
    unsafe {
        let cmd = [SCSI_READ_CAPACITY, lun << 5, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut response = [0u8; 8];
        let outcome = cycle_with_timeout(
            dev,
            CycleRequest {
                lun,
                data: response.as_mut_ptr(),
                data_len: response.len() as u32,
                command: &cmd,
                direction: ScsiDataDirection::DeviceToHost,
            },
            timeout_ticks,
        );
        let capacity = super::decode_read_capacity(outcome.result, outcome.status, response);
        if capacity.result < 0 {
            return capacity.result;
        }

        write_capacity_outputs(capacity, sector_size, n_sectors);
        set_handle_sector_size(dev, lun, capacity.sector_size);
        set_last_capacity(lun, capacity.sector_size, capacity.sectors);
        USBSTORAGE_OK
    }
}

fn write_capacity_outputs(
    capacity: ReadCapacityResult,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) {
    unsafe {
        if !sector_size.is_null() {
            *sector_size = capacity.sector_size;
        }
        if !n_sectors.is_null() {
            *n_sectors = capacity.sectors;
        }
    }
}

unsafe fn abort_open_with(
    dev: &mut UsbStorageHandle,
    error: i32,
    close: impl FnOnce(&mut i32) -> i32,
) -> i32 {
    let close_result = close(&mut dev.usb_fd);
    if close_result < 0 {
        super::startup::record(super::ProbeStage::Reset, error);
        return close_result;
    }
    unsafe { release_handle_resources(dev) };
    *dev = UsbStorageHandle::default();
    dev.usb_fd = -1;
    error
}

unsafe fn abort_open(dev: &mut UsbStorageHandle, error: i32) -> i32 {
    unsafe { abort_open_with(dev, error, |fd| usb::close_device(fd)) }
}

pub fn open(dev: *mut UsbStorageHandle, device_id: i32, vid: u16, pid: u16) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if drive_for_handle(dev).is_some() {
        if let Some(error) = unsafe { pending_bulk_error(handle_retval_atomic(dev)) } {
            return error;
        }
        unsafe {
            if (*dev).usb_fd >= 0 {
                return USBSTORAGE_EINIT;
            }
        }
    }
    unsafe {
        *dev = UsbStorageHandle::default();
        (*dev).usb_fd = -1;
        (*dev).tag = TAG_START;

        if lwp::mutex_init(&mut (*dev).lock, 0) != lwp::LWP_SUCCESSFUL {
            return USBSTORAGE_EINIT;
        }
        if sys_create_alarm(&mut (*dev).alarm) < 0 {
            let _ = lwp::mutex_destroy((*dev).lock);
            (*dev).lock = lwp::LWP_MUTEX_NULL;
            return USBSTORAGE_EINIT;
        }

        let mut fd = -1;
        let rc = usb::open_device(device_id, vid, pid, &mut fd);
        if rc < 0 {
            release_handle_resources(&mut *dev);
            *dev = UsbStorageHandle::default();
            (*dev).usb_fd = -1;
            return rc;
        }
        (*dev).usb_fd = fd;

        let open_reset_required = match find_mass_storage_interface(&mut *dev, fd) {
            Ok(reset_required) => reset_required,
            Err(rc) => return abort_open(&mut *dev, rc),
        };

        let mut conf = 0u8;
        let configuration_result = usb::get_configuration(fd, &mut conf);
        if configuration_result < 0 {
            return abort_open(&mut *dev, configuration_result);
        }
        if conf != (*dev).configuration {
            let set_result = usb::set_configuration(fd, (*dev).configuration);
            if set_result < 0 {
                return abort_open(&mut *dev, set_result);
            }
        }
        if (*dev).alt_interface != 0 {
            let alternative_result = usb::set_alternative_interface(
                fd,
                (*dev).interface as u8,
                (*dev).alt_interface as u8,
            );
            if alternative_result < 0 {
                return abort_open(&mut *dev, alternative_result);
            }
        }

        if open_reset_required {
            let reset_result = with_transfer_guard(
                || TransferGuard::acquire((*dev).lock),
                || mass_storage_reset(&mut *dev),
            );
            if reset_result < 0 {
                return abort_open(&mut *dev, reset_result);
            }
        }

        let max_lun = match super::lun_count_after_get_max_lun(class_get_max_lun(&mut *dev)) {
            Ok(count) => count,
            Err(error) => return abort_open(&mut *dev, error),
        };
        (*dev).max_lun = max_lun;

        let storage_result = allocate_handle_storage(&mut *dev);
        if storage_result < 0 {
            return abort_open(&mut *dev, storage_result);
        }

        let removal_notify_result =
            usb::device_removal_notify_async(fd, Some(storage_device_removed), dev.cast());
        if let Some(drive) = drive_for_handle(dev) {
            if removal_notify_result < 0 {
                super::startup::record(super::ProbeStage::Open, removal_notify_result);
                return abort_open(&mut *dev, removal_notify_result);
            }
            // A removal callback can run during registration.
            if drive.removed.load(Ordering::Acquire) {
                return abort_open(&mut *dev, USBSTORAGE_ENOINTERFACE);
            }
        }

        USBSTORAGE_LAST_DEVICE_ID.store(device_id, Ordering::Release);
        USBSTORAGE_LAST_LUN.store(0, Ordering::Release);
        USBSTORAGE_CAPACITY_VALID.store(false, Ordering::Release);
        USBSTORAGE_OK
    }
}

unsafe fn synchronize_cache_inner(dev: &mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { synchronize_cache(dev, lun, false) }
}

unsafe fn synchronize_cache_locked(dev: &mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { synchronize_cache(dev, lun, true) }
}

unsafe fn synchronize_cache(dev: &mut UsbStorageHandle, lun: u8, lock_held: bool) -> i32 {
    if lun >= dev.max_lun {
        return USBSTORAGE_EINVAL;
    }
    let cmd = [SCSI_SYNCHRONIZE_CACHE, lun << 5, 0, 0, 0, 0, 0, 0, 0, 0];
    let request = CycleRequest {
        lun,
        data: ptr::null_mut(),
        data_len: 0,
        command: &cmd,
        direction: ScsiDataDirection::DeviceToHost,
    };
    let outcome = unsafe {
        if lock_held {
            cycle_locked(dev, request)
        } else {
            cycle(dev, request)
        }
    };
    let result = if outcome.result >= 0 && outcome.status != 0 {
        let mut sense = [0u8; SCSI_SENSE_REPLY_SIZE];
        let command = [
            SCSI_REQUEST_SENSE,
            lun << 5,
            0,
            0,
            SCSI_SENSE_REPLY_SIZE as u8,
            0,
        ];
        let request = CycleRequest {
            lun,
            data: sense.as_mut_ptr(),
            data_len: sense.len() as u32,
            command: &command,
            direction: ScsiDataDirection::DeviceToHost,
        };
        let reply = unsafe {
            if lock_held {
                cycle_locked(dev, request)
            } else {
                cycle(dev, request)
            }
        };
        if reply.result >= 0
            && reply.status == 0
            && matches!(sense[0] & 0x7f, 0x70 | 0x71)
            && sense[7] >= 10
        {
            super::startup::record_sense(sense[2] & 15, sense[12], sense[13]);
        } else {
            super::startup::record_sense(0xff, 0xff, 0xff);
        }
        USBSTORAGE_ESTATUS
    } else {
        outcome.result
    };
    super::startup::record(super::ProbeStage::Flush, result);
    result
}

/// Flushes the selected LUN's volatile write cache using SCSI SYNCHRONIZE CACHE.
///
/// # Safety
///
/// `dev` must point to a live handle exclusively borrowed for this call.
pub unsafe fn flush(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    if !is_wii_hardware() || dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        if (*dev).buffer.is_null() || (*dev).sector_size.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }
        synchronize_cache_inner(&mut *dev, lun)
    }
}

/// Flushes the LUN currently owned by the global disc interface.
pub fn flush_default() -> i32 {
    flush_drive(0)
}

pub(super) fn flush_drive(slot: usize) -> i32 {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    let Some(drive) = DRIVES.get(slot) else {
        return USBSTORAGE_EINVAL;
    };
    if !drive.mounted.load(Ordering::Acquire) {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe { flush(drive.handle.get(), drive.lun.load(Ordering::Acquire) as u8) }
}

unsafe fn invalidate_removed_handle(dev: *mut UsbStorageHandle) {
    unsafe {
        if dev.is_null() {
            return;
        }
        if (*dev).usb_fd >= 0 {
            let result = usb::close_device(&mut (*dev).usb_fd);
            if result < 0 {
                super::startup::record(super::ProbeStage::Reset, result);
                // Keep the live handle and claim for a later close attempt.
                return;
            }
        }
        release_handle_resources(&mut *dev);
        *dev = UsbStorageHandle::default();
        (*dev).usb_fd = -1;
        clear_handle_state(dev);
    }
}

pub fn close(dev: *mut UsbStorageHandle) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if let Some(error) = unsafe { pending_bulk_error(handle_retval_atomic(dev)) } {
        return error;
    }
    unsafe {
        // A failed open can retain a live descriptor after close fails. Its
        // transfer buffers may be absent. The close engine already skips
        // unmounted LUNs, so it can retry transport close and release resources.
        if (*dev).usb_fd < 0 {
            return USBSTORAGE_ENOINTERFACE;
        }
        let transfer_guard = match TransferGuard::acquire((*dev).lock) {
            Ok(guard) => guard,
            Err(error) => return error,
        };
        if let Some(error) = pending_bulk_error(handle_retval_atomic(dev)) {
            drop(transfer_guard);
            return error;
        }
        if let Some(drive) = drive_for_handle(dev) {
            drive.mounted.store(false, Ordering::Release);
        }

        struct Operations {
            dev: *mut UsbStorageHandle,
        }

        impl CloseOperations for Operations {
            fn max_lun(&self) -> u8 {
                unsafe { (*self.dev).max_lun.min(MAX_DEVICE_LUNS as u8) }
            }

            fn lun_is_mounted(&self, lun: u8) -> bool {
                unsafe { handle_sector_size(self.dev, lun) != 0 }
            }

            fn flush_lun(&mut self, lun: u8) -> i32 {
                unsafe { synchronize_cache_locked(&mut *self.dev, lun) }
            }

            fn close_transport(&mut self) -> i32 {
                unsafe { usb::close_device(&mut (*self.dev).usb_fd) }
            }
        }

        let mut operations = Operations { dev };
        let result = close_policy::close(&mut operations);
        drop(transfer_guard);
        if (*dev).usb_fd < 0 {
            cleanup_closed_handle(dev);
        }
        result
    }
}

pub fn reset(dev: *mut UsbStorageHandle) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        if let Some(error) = pending_bulk_error(handle_retval_atomic(dev)) {
            return error;
        }
        with_transfer_guard(
            || TransferGuard::acquire((*dev).lock),
            || mass_storage_reset(&mut *dev),
        )
    }
}

pub fn get_max_lun(dev: *mut UsbStorageHandle) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe { (*dev).max_lun as i32 }
}

pub fn mount_lun(dev: *mut UsbStorageHandle, lun: u8) -> i32 {
    unsafe { mount_lun_with_timeout_ticks(dev, lun, USBSTORAGE_CONTROL_TIMEOUT_TICKS) }
}

unsafe fn mount_lun_with_timeout_ticks(
    dev: *mut UsbStorageHandle,
    lun: u8,
    timeout_ticks: u64,
) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        let dev_ref = &mut *dev;
        if dev_ref.max_lun == 0 || lun >= dev_ref.max_lun {
            return USBSTORAGE_EINVAL;
        }
        if dev_ref.buffer.is_null() || dev_ref.sector_size.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }

        let dev_ptr = dev_ref as *mut UsbStorageHandle;
        let (rc, _sector_size, _sectors) = mount_lun_with(
            || {
                let ticks = lwp::TICKS_PER_SECOND.saturating_mul(50) / 1_000_000;
                lwp::sleep_ticks(ticks.max(1));
            },
            || clear_errors_with_timeout(&mut *dev_ptr, lun, timeout_ticks),
            || {
                with_transfer_guard(
                    || TransferGuard::acquire((*dev_ptr).lock),
                    || mass_storage_reset(&mut *dev_ptr),
                )
            },
            || inquiry_inner_with_timeout(&mut *dev_ptr, lun, timeout_ticks),
            |sector_size, sectors| {
                read_capacity_inner_with_timeout(
                    &mut *dev_ptr,
                    lun,
                    sector_size,
                    sectors,
                    timeout_ticks,
                )
            },
        );
        if rc >= 0 {
            USBSTORAGE_MOUNTED.store(true, Ordering::Release);
            USBSTORAGE_LAST_LUN.store(lun as u32, Ordering::Release);
        }
        rc
    }
}

fn mount_lun_with<Delay, Clear, Reset, Inquiry, Capacity>(
    mut delay: Delay,
    mut clear_errors: Clear,
    mut reset: Reset,
    mut inquiry: Inquiry,
    mut read_capacity: Capacity,
) -> (i32, u32, u32)
where
    Delay: FnMut(),
    Clear: FnMut() -> i32,
    Reset: FnMut() -> i32,
    Inquiry: FnMut() -> i32,
    Capacity: FnMut(&mut u32, &mut u32) -> i32,
{
    delay();
    let clear_result = clear_errors();
    if clear_result < 0 {
        // C performs both operations but lets their results be overwritten by
        // Inquiry and then Read Capacity. Preserve that observable result flow.
        let _reset_result_ignored_by_c = reset();
        let _second_clear_result_ignored_by_c = clear_errors();
    }

    let _inquiry_result_ignored_by_c = inquiry();

    let mut sector_size = 0u32;
    let mut sectors = 0u32;
    let mut result = read_capacity(&mut sector_size, &mut sectors);
    if result >= 0 && (sector_size < 512 || sectors == 0) {
        result = USBSTORAGE_INVALID_LUN;
    }
    (result, sector_size, sectors)
}

fn suspend_with<F>(dev: &mut UsbStorageHandle, suspend_device: F) -> i32
where
    F: FnOnce(i32) -> i32,
{
    transport::suspend(&mut dev.suspended, dev.usb_fd, suspend_device)
}

pub fn suspend(dev: *mut UsbStorageHandle) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        with_transfer_guard(
            || TransferGuard::acquire((*dev).lock),
            || {
                if let Some(error) = pending_bulk_error(handle_retval_atomic(dev)) {
                    return error;
                }
                suspend_with(&mut *dev, usb::suspend_device)
            },
        )
    }
}

pub fn is_dvd() -> i32 {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    drive_is_dvd(&DRIVES[0])
}

fn drive_is_dvd(drive: &Drive) -> i32 {
    if !is_wii_hardware() {
        return 0;
    }
    if !drive.mounted.load(Ordering::Acquire) {
        return 0;
    }
    unsafe {
        let lun = drive.lun.load(Ordering::Acquire) as u8;
        if (*drive.handle.get()).buffer.is_null() || (*drive.handle.get()).sector_size.is_null() {
            return 0;
        }
        if handle_sector_size(drive.handle.get(), lun) != 2048 {
            return 0;
        }
        let mut sector_size = 0u32;
        let mut sectors = 0u32;
        if read_capacity_inner(
            &mut *drive.handle.get(),
            lun,
            &mut sector_size,
            &mut sectors,
        ) < 0
        {
            return 0;
        }
        i32::from(sector_size == 2048)
    }
}

pub fn ioctl(request: i32, rdc: *mut RawDeviceCommand) -> i32 {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if request != B_RAW_DEVICE_COMMAND as i32
        || rdc.is_null()
        || !DISC_MOUNTED.load(Ordering::Acquire)
    {
        return -1;
    }
    unsafe {
        if (*rdc).command_length == 0 || (*rdc).command_length > 16 {
            return USBSTORAGE_EINVAL;
        }
        let lun = DISC_LUN.load(Ordering::Acquire) as u8;
        if (*DRIVES[0].handle.get()).buffer.is_null()
            || (*DRIVES[0].handle.get()).sector_size.is_null()
        {
            return -1;
        }
        let outcome = cycle(
            &mut *DRIVES[0].handle.get(),
            CycleRequest {
                lun,
                data: (*rdc).data.cast(),
                data_len: (*rdc).data_length as u32,
                command: &(&(*rdc).command)[..(*rdc).command_length as usize],
                direction: if (*rdc).flags != B_RAW_DEVICE_DATA_IN {
                    ScsiDataDirection::HostToDevice
                } else {
                    ScsiDataDirection::DeviceToHost
                },
            },
        );
        (*rdc).scsi_status = outcome.status;
        USBSTORAGE_LAST_RAW_WRITE.store((*rdc).flags != B_RAW_DEVICE_DATA_IN, Ordering::Release);
        outcome.result
    }
}

pub fn read_capacity(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector_size: *mut u32,
    n_sectors: *mut u32,
) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        let dev_ref = &mut *dev;
        if dev_ref.max_lun == 0 || lun >= dev_ref.max_lun {
            return USBSTORAGE_EINVAL;
        }
        if dev_ref.buffer.is_null() || dev_ref.sector_size.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }
        read_capacity_inner(dev_ref, lun, sector_size, n_sectors)
    }
}

pub fn read(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
) -> i32 {
    transfer_blocks(
        dev,
        lun,
        sector,
        n_sectors,
        buffer,
        ScsiDataDirection::DeviceToHost,
    )
}

pub fn write(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *const u8,
) -> i32 {
    transfer_blocks(
        dev,
        lun,
        sector,
        n_sectors,
        buffer.cast_mut(),
        ScsiDataDirection::HostToDevice,
    )
}

fn transfer_blocks(
    dev: *mut UsbStorageHandle,
    lun: u8,
    sector: u32,
    n_sectors: u16,
    buffer: *mut u8,
    direction: ScsiDataDirection,
) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() || buffer.is_null() || n_sectors == 0 {
        return USBSTORAGE_EINVAL;
    }
    unsafe {
        let dev_ref = &mut *dev;
        if dev_ref.max_lun == 0 || lun >= dev_ref.max_lun || dev_ref.sector_size.is_null() {
            return USBSTORAGE_EINVAL;
        }
        if dev_ref.buffer.is_null() || dev_ref.sector_size.is_null() {
            return USBSTORAGE_ENOINTERFACE;
        }
        if handle_sector_size(ptr::from_ref(dev_ref), lun) == 0 {
            return USBSTORAGE_EINVAL;
        }
        if sector.checked_add(n_sectors as u32 - 1).is_none() {
            return USBSTORAGE_EINVAL;
        }
        let idle_remount = super::idle_remount_required(
            storage_now_seconds(),
            handle_last_used(dev).load(Ordering::Acquire),
        );
        let timeout_ticks = io_timeout_ticks(idle_remount);
        if idle_remount {
            let remount_result = mount_lun_with_timeout_ticks(dev, lun, timeout_ticks);
            USBSTORAGE_LAST_IDLE_REMOUNT_RESULT.store(remount_result, Ordering::Release);
            if remount_result < 0 {
                return remount_result;
            }
        }
        let sector_size = handle_sector_size(ptr::from_ref(dev_ref), lun);
        let Some(transfer_len) = (n_sectors as u32).checked_mul(sector_size) else {
            return USBSTORAGE_EINVAL;
        };

        let cmd = [
            match direction {
                ScsiDataDirection::DeviceToHost => SCSI_READ_10,
                ScsiDataDirection::HostToDevice => SCSI_WRITE_10,
            },
            lun << 5,
            (sector >> 24) as u8,
            (sector >> 16) as u8,
            (sector >> 8) as u8,
            sector as u8,
            0,
            (n_sectors >> 8) as u8,
            n_sectors as u8,
            0,
        ];
        let outcome = cycle_with_timeout(
            dev_ref,
            CycleRequest {
                lun,
                data: buffer,
                data_len: transfer_len,
                command: &cmd,
                direction,
            },
            timeout_ticks,
        );
        let result = super::read_write_cycle_result(outcome.result, outcome.status);
        handle_last_used(dev).store(storage_now_seconds(), Ordering::Release);
        result
    }
}

pub fn start_stop(dev: *mut UsbStorageHandle, lun: u8, lo_ej: u8, start: u8, imm: u8) -> i32 {
    if !is_wii_hardware() {
        return USBSTORAGE_ENOINTERFACE;
    }
    if dev.is_null() {
        return USBSTORAGE_ENOINTERFACE;
    }
    unsafe {
        let _transfer_guard = match TransferGuard::acquire((*dev).lock) {
            Ok(guard) => guard,
            Err(error) => return error,
        };
        let dev_ref = &mut *dev;
        if dev_ref.max_lun == 0 || lun >= dev_ref.max_lun {
            return USBSTORAGE_EINVAL;
        }
        let cmd = [
            SCSI_START_STOP,
            (lun << 5) | (imm & 1),
            0,
            0,
            ((lo_ej & 1) << 1) | (start & 1),
            0,
        ];
        let mut status = 0u8;
        let rc = send_cbw(
            dev_ref,
            lun,
            0,
            CBW_IN,
            &cmd,
            USBSTORAGE_CONTROL_TIMEOUT_TICKS,
        );
        if rc < 0 {
            return rc;
        }
        let rc = read_start_stop_csw(dev_ref, &mut status, start_stop_csw_timeout_ticks(imm));
        if rc >= 0 && status != 0 {
            USBSTORAGE_ESTATUS
        } else {
            rc
        }
    }
}

fn mounted_media_is_inserted(sector_size: u32, dvd_present: impl FnOnce() -> bool) -> bool {
    if sector_size != 2048 {
        true
    } else {
        dvd_present()
    }
}

unsafe fn drive_is_inserted(drive: &Drive) -> bool {
    unsafe {
        if !is_wii_hardware() {
            return false;
        }
        if drive.mounted.load(Ordering::Acquire) && !drive.removed.load(Ordering::Acquire) {
            let lun = drive.lun.load(Ordering::Acquire) as usize;
            if lun >= MAX_DEVICE_LUNS {
                invalidate_removed_handle(drive.handle.get());
                return false;
            }

            if (*drive.handle.get()).buffer.is_null() || (*drive.handle.get()).sector_size.is_null()
            {
                invalidate_removed_handle(drive.handle.get());
                return false;
            }
            return mounted_media_is_inserted(
                handle_sector_size(drive.handle.get(), lun as u8),
                || drive_is_dvd(drive) > 0,
            );
        }
        if !USBSTORAGE_INITED.load(Ordering::Acquire) {
            return false;
        }

        super::startup::record(super::ProbeStage::Enumerate, USBSTORAGE_OK);
        let mut buffer = [usb::UsbDeviceEntry::default(); DEVLIST_MAXSIZE];
        let device_count = match storage_device_candidates(&mut buffer) {
            Ok(count) => {
                super::startup::record_candidates(count);
                super::startup::record(super::ProbeStage::Enumerate, USBSTORAGE_OK);
                count
            }
            Err(error) => {
                super::startup::record_candidates(0);
                let stage = super::startup::enumeration_failure_stage(usb::v5_failure_status());
                super::startup::record(stage, error);
                if drive.vid.load(Ordering::Acquire) != 0 || drive.pid.load(Ordering::Acquire) != 0
                {
                    invalidate_removed_handle(drive.handle.get());
                }
                return false;
            }
        };
        if device_count == 0 {
            if drive.vid.load(Ordering::Acquire) != 0 || drive.pid.load(Ordering::Acquire) != 0 {
                invalidate_removed_handle(drive.handle.get());
            }
            return false;
        }

        if (*drive.handle.get()).usb_fd >= 0 {
            invalidate_removed_handle(drive.handle.get());
            if (*drive.handle.get()).usb_fd >= 0 {
                return false;
            }
        }

        let eligible = |entry: &usb::UsbDeviceEntry| {
            entry.vid != 0
                && entry.pid != 0
                && !(entry.vid == 0x0b95 && entry.pid == 0x7720)
                && DRIVES
                    .iter()
                    .all(|other| core::ptr::eq(other, drive) || other.can_select(entry.device_id))
        };
        let available = buffer
            .iter()
            .take(device_count)
            .filter(|entry| eligible(entry))
            .count();
        super::startup::record_candidates(available);
        for entry in buffer.iter().take(device_count) {
            if !eligible(entry) {
                continue;
            }
            drive.removed.store(false, Ordering::Release);
            // Retain ownership through open, mount failure, and failed close.
            drive.device_id.store(entry.device_id, Ordering::Release);
            let open_result = open(drive.handle.get(), entry.device_id, entry.vid, entry.pid);
            super::startup::record(super::ProbeStage::Open, open_result);
            if open_result < 0 {
                if (*drive.handle.get()).usb_fd >= 0 {
                    return false;
                }
                drive.reset();
                continue;
            }

            let max_lun = get_max_lun(drive.handle.get());
            super::startup::record(super::ProbeStage::MaxLun, max_lun);
            for lun in 0..max_lun.max(0) as u8 {
                let rc = mount_lun(drive.handle.get(), lun);
                super::startup::record(super::ProbeStage::MountLun, rc);
                if rc == USBSTORAGE_INVALID_LUN {
                    continue;
                }
                if rc < 0 {
                    let reset_result = reset(drive.handle.get());
                    if reset_result < 0 {
                        super::startup::record(super::ProbeStage::Reset, reset_result);
                        invalidate_removed_handle(drive.handle.get());
                        return false;
                    }
                    continue;
                }

                drive.lun.store(lun as u32, Ordering::Release);
                drive.device_id.store(entry.device_id, Ordering::Release);
                drive.vid.store(entry.vid as u32, Ordering::Release);
                drive.pid.store(entry.pid as u32, Ordering::Release);
                drive
                    .last_used
                    .store(storage_now_seconds().wrapping_sub(100), Ordering::Release);
                if !drive.publish() {
                    invalidate_removed_handle(drive.handle.get());
                    return false;
                }
                super::startup::record(super::ProbeStage::Ready, USBSTORAGE_OK);
                return true;
            }

            invalidate_removed_handle(drive.handle.get());
            if (*drive.handle.get()).usb_fd >= 0 {
                return false;
            }
        }

        false
    }
}

unsafe fn drive_read_sectors(
    drive: &Drive,
    sector: u32,
    num_sectors: u32,
    buffer: *mut c_void,
) -> bool {
    unsafe {
        if !is_wii_hardware() {
            return false;
        }
        if !drive.mounted.load(Ordering::Acquire)
            || drive.removed.load(Ordering::Acquire)
            || buffer.is_null()
            || num_sectors == 0
        {
            return false;
        }
        transfer_disc_sectors(drive, sector, num_sectors, buffer.cast(), false)
    }
}

unsafe fn drive_write_sectors(
    drive: &Drive,
    sector: u32,
    num_sectors: u32,
    buffer: *const c_void,
) -> bool {
    unsafe {
        if !is_wii_hardware() {
            return false;
        }
        if !drive.mounted.load(Ordering::Acquire)
            || drive.removed.load(Ordering::Acquire)
            || buffer.is_null()
            || num_sectors == 0
        {
            return false;
        }
        transfer_disc_sectors(drive, sector, num_sectors, buffer.cast_mut().cast(), true)
    }
}

unsafe fn transfer_disc_sectors(
    drive: &Drive,
    mut sector: u32,
    mut remaining: u32,
    mut buffer: *mut u8,
    write_transfer: bool,
) -> bool {
    unsafe {
        let lun = drive.lun.load(Ordering::Acquire) as usize;
        if lun >= MAX_DEVICE_LUNS {
            return false;
        }
        if (*drive.handle.get()).buffer.is_null() || (*drive.handle.get()).sector_size.is_null() {
            return false;
        }
        let sector_size = handle_sector_size(drive.handle.get(), lun as u8) as usize;
        if sector_size == 0 {
            return false;
        }

        while remaining != 0 {
            let chunk = remaining.min(u16::MAX as u32) as u16;
            let result = if write_transfer {
                write(
                    drive.handle.get(),
                    lun as u8,
                    sector,
                    chunk,
                    buffer.cast_const(),
                )
            } else {
                read(drive.handle.get(), lun as u8, sector, chunk, buffer)
            };
            if result < 0 {
                super::startup::record(
                    if write_transfer {
                        super::ProbeStage::TransferWrite
                    } else {
                        super::ProbeStage::TransferRead
                    },
                    result,
                );
                return false;
            }

            let Some(chunk_bytes) = (chunk as usize).checked_mul(sector_size) else {
                return false;
            };
            buffer = buffer.add(chunk_bytes);
            remaining -= chunk as u32;
            if remaining != 0 {
                let Some(next_sector) = sector.checked_add(chunk as u32) else {
                    return false;
                };
                sector = next_sector;
            }
        }

        true
    }
}

fn clear_status_result() -> bool {
    true
}

pub unsafe fn clear_status() -> bool {
    if !is_wii_hardware() {
        return false;
    }
    clear_status_result()
}

#[cfg(test)]
fn shutdown_with(mut close_device: impl FnMut() -> i32) -> bool {
    // Always clear mounted state after the close attempt, but report a failed
    // close to the caller so it cannot mistake an incomplete operation for
    // success.
    let close_result = close_device();
    reset_global_disc_state();
    close_result >= 0
}

pub(super) unsafe fn disc_shutdown(slot: usize) -> bool {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    let Some(drive) = DRIVES.get(slot) else {
        return false;
    };
    if !is_wii_hardware() {
        return false;
    }
    unsafe {
        if (*drive.handle.get()).usb_fd >= 0 {
            close(drive.handle.get()) >= 0
        } else {
            drive.reset();
            true
        }
    }
}

pub(super) unsafe fn disc_is_inserted(slot: usize) -> bool {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    DRIVES
        .get(slot)
        .is_some_and(|drive| unsafe { drive_is_inserted(drive) })
}
pub(super) unsafe fn disc_read_sectors(
    slot: usize,
    sector: u32,
    count: u32,
    buffer: *mut c_void,
) -> bool {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    DRIVES
        .get(slot)
        .is_some_and(|drive| unsafe { drive_read_sectors(drive, sector, count, buffer) })
}
pub(super) unsafe fn disc_write_sectors(
    slot: usize,
    sector: u32,
    count: u32,
    buffer: *const c_void,
) -> bool {
    let _guard = match lock_discs() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    DRIVES
        .get(slot)
        .is_some_and(|drive| unsafe { drive_write_sectors(drive, sector, count, buffer) })
}
