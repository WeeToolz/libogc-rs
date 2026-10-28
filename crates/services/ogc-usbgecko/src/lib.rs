#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::mem::size_of;
use core::slice;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub use ogc_exi as exi;
use ogc_lwp_driver as lwp;
use ogc_lwp_driver::{TICKS_PER_SECOND, now_ticks, yield_thread};

const CHANNELS: usize = 3;

mod command;

static USB_FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);
static EXI_WAIT_QUEUE: [AtomicU32; CHANNELS] = [
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
];

struct GeckoLock(i32);

impl GeckoLock {
    fn acquire(channel: i32) -> Option<Self> {
        let index = channel_index(channel)?;
        let queue = exi_wait_queue(index)?;

        let result = retry_lock_after_wakeup(|| {
            // Keep EXI unlock callbacks from racing between a failed lock and queue sleep.
            let irq = lwp::critical_enter();
            let result = lock_or_sleep(
                queue,
                |callback| unsafe { exi::lock(channel, exi::EXI_DEVICE_0, callback) != 0 },
                lwp::queue_sleep,
            );
            lwp::critical_exit(irq);
            result
        });
        if result.is_ok() {
            Some(Self(channel))
        } else {
            None
        }
    }
}

impl Drop for GeckoLock {
    fn drop(&mut self) {
        let _ = exi::unlock(self.0);
    }
}

fn exi_wait_queue(channel: usize) -> Option<lwp::QueueHandle> {
    if channel >= CHANNELS {
        return None;
    }
    let current = EXI_WAIT_QUEUE[channel].load(Ordering::Acquire);
    if current != lwp::LWP_TQUEUE_NULL {
        return Some(current);
    }

    let mut candidate = lwp::LWP_TQUEUE_NULL;
    if unsafe { lwp::init_queue(&mut candidate) } != lwp::LWP_SUCCESSFUL {
        return None;
    }
    match EXI_WAIT_QUEUE[channel].compare_exchange(
        lwp::LWP_TQUEUE_NULL,
        candidate,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => Some(candidate),
        Err(existing) => {
            lwp::close_queue(candidate);
            Some(existing)
        }
    }
}

/// Return the production wait queue for a valid channel for a DOL probe.
#[doc(hidden)]
pub fn wait_queue_probe(channel: usize) -> Option<lwp::QueueHandle> {
    exi_wait_queue(channel)
}

unsafe extern "C" fn exi_unlock_callback(channel: i32, _device: i32) -> i32 {
    if let Some(index) = channel_index(channel) {
        let queue = EXI_WAIT_QUEUE[index].load(Ordering::Acquire);
        if queue != lwp::LWP_TQUEUE_NULL {
            lwp::queue_broadcast(queue);
        }
    }
    1
}

fn lock_or_sleep(
    queue: lwp::QueueHandle,
    try_lock: impl FnOnce(exi::ExiCallback) -> bool,
    sleep: impl FnOnce(lwp::QueueHandle) -> i32,
) -> Result<bool, i32> {
    if try_lock(Some(exi_unlock_callback)) {
        return Ok(true);
    }
    let result = sleep(queue);
    if result == lwp::LWP_SUCCESSFUL {
        Ok(false)
    } else {
        Err(result)
    }
}

fn retry_lock_after_wakeup(mut attempt: impl FnMut() -> Result<bool, i32>) -> Result<(), i32> {
    loop {
        if attempt()? {
            return Ok(());
        }
    }
}

/// Exercise the production callback, queue-sleep, and retry helpers with controlled results.
#[doc(hidden)]
pub fn lock_retry_probe_with(
    queue: lwp::QueueHandle,
    mut try_lock: impl FnMut(exi::ExiCallback) -> bool,
    mut sleep: impl FnMut(lwp::QueueHandle) -> i32,
) -> Result<(), i32> {
    retry_lock_after_wakeup(|| lock_or_sleep(queue, &mut try_lock, &mut sleep))
}

fn channel_index(chn: i32) -> Option<usize> {
    (chn >= 0 && (chn as usize) < CHANNELS).then_some(chn as usize)
}

fn flash_buffer_is_valid(buffer_is_null: bool, length: usize) -> bool {
    length == 0 || !buffer_is_null
}

fn zero_length_flash_result(length: usize) -> Option<i32> {
    (length == 0).then_some(1)
}

fn delay_microseconds(microseconds: u64) {
    if microseconds == 0 {
        return;
    }
    let ticks = (TICKS_PER_SECOND.saturating_mul(microseconds) / 1_000_000).max(1);
    let start = now_ticks();
    while now_ticks().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

fn with_gecko_lock<R>(channel: i32, f: impl FnOnce() -> R) -> Option<R> {
    let _guard = GeckoLock::acquire(channel)?;
    Some(f())
}

fn send_command(channel: i32, cmd: &mut u16) -> bool {
    struct ExiCommand(i32);
    impl command::Transport for ExiCommand {
        fn select(&mut self) -> bool {
            exi::select(self.0, exi::EXI_DEVICE_0, exi::EXI_SPEED32MHZ) != 0
        }
        fn start(&mut self, cmd: &mut u16) -> bool {
            unsafe {
                exi::imm(
                    self.0,
                    (cmd as *mut u16).cast(),
                    size_of::<u16>() as u32,
                    exi::EXI_READWRITE as i32,
                    None,
                ) != 0
            }
        }
        fn sync(&mut self) -> bool {
            exi::sync(self.0) != 0
        }
        fn deselect(&mut self) -> bool {
            exi::deselect(self.0) != 0
        }
    }
    command::send(&mut ExiCommand(channel), cmd)
}

fn flash_write_command(channel: i32, flashaddress: u32, flashdata: u8) -> bool {
    if flashaddress > 0x7FFFF {
        return false;
    }
    let mut ret = true;
    let mut val = 0xE000_0000 | (flashaddress << 9) | ((flashdata as u32) << 1);

    if exi::select(channel, exi::EXI_DEVICE_0, exi::EXI_SPEED16MHZ) == 0 {
        ret = false;
    }
    if unsafe {
        exi::imm_ex(
            channel,
            &mut val as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
            exi::EXI_WRITE as i32,
        )
    } == 0
    {
        ret = false;
    }
    if unsafe {
        exi::imm_ex(
            channel,
            &mut val as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
            exi::EXI_WRITE as i32,
        )
    } == 0
    {
        ret = false;
    }
    if exi::deselect(channel) == 0 {
        ret = false;
    }

    ret
}

fn flash_program_byte_with(
    flashaddress: u32,
    flashdata: u8,
    mut write_command: impl FnMut(u32, u8) -> bool,
) -> bool {
    for (address, data) in [
        (0x5555, 0xAA),
        (0x2AAA, 0x55),
        (0x5555, 0xA0),
        (flashaddress, flashdata),
    ] {
        if !write_command(address, data) {
            return false;
        }
    }
    true
}

fn flash_program_byte(channel: i32, flashaddress: u32, flashdata: u8) -> bool {
    flash_program_byte_with(flashaddress, flashdata, |address, data| {
        flash_write_command(channel, address, data)
    })
}

fn flash_read_command(channel: i32, flashaddress: u32, flashdata: &mut u8) -> bool {
    if flashaddress > 0x7FFFF {
        return false;
    }
    let mut ret = true;
    let mut val = 0xF000_0000 | (flashaddress << 9);

    if exi::select(channel, exi::EXI_DEVICE_0, exi::EXI_SPEED16MHZ) == 0 {
        ret = false;
    }
    if unsafe {
        exi::imm_ex(
            channel,
            &mut val as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
            exi::EXI_WRITE as i32,
        )
    } == 0
    {
        ret = false;
    }
    if unsafe {
        exi::imm_ex(
            channel,
            &mut val as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
            exi::EXI_READ as i32,
        )
    } == 0
    {
        ret = false;
    }
    if exi::deselect(channel) == 0 {
        ret = false;
    }

    if ret {
        *flashdata = (val >> 23) as u8;
    }
    ret
}

fn flash_software_id_entry(channel: i32) -> bool {
    flash_write_command(channel, 0x5555, 0xAA)
        && flash_write_command(channel, 0x2AAA, 0x55)
        && flash_write_command(channel, 0x5555, 0x90)
}

fn flash_software_id_exit(channel: i32) -> bool {
    flash_write_command(channel, 0x5555, 0xAA)
        && flash_write_command(channel, 0x2AAA, 0x55)
        && flash_write_command(channel, 0x5555, 0xF0)
}

fn recv_byte_with(byte: &mut u8, mut send: impl FnMut(&mut u16) -> bool) -> bool {
    *byte = 0;
    let mut val = 0xA000u16;
    if !send(&mut val) {
        return false;
    }
    if val & 0x0800 == 0 {
        return false;
    }
    *byte = (val & 0x00FF) as u8;
    true
}

fn recv_byte(channel: i32, byte: &mut u8) -> bool {
    recv_byte_with(byte, |val| send_command(channel, val))
}

fn send_byte(channel: i32, byte: u8) -> bool {
    let mut val = 0xB000u16 | ((byte as u16) << 4);
    if !send_command(channel, &mut val) {
        return false;
    }
    (val & 0x0400) != 0
}

fn check_recv(channel: i32) -> bool {
    let mut val = 0xD000u16;
    if !send_command(channel, &mut val) {
        return false;
    }
    (val & 0x0400) != 0
}

fn check_send(channel: i32) -> bool {
    let mut val = 0xC000u16;
    if !send_command(channel, &mut val) {
        return false;
    }
    (val & 0x0400) != 0
}

/// Drain pending input from the channel.
pub fn flush(chn: i32) {
    if channel_index(chn).is_none() {
        return;
    }

    USB_FLUSH_COUNT.fetch_add(1, Ordering::AcqRel);
    let _ = with_gecko_lock(chn, || {
        loop {
            let mut tmp = 0u8;
            if !recv_byte(chn, &mut tmp) {
                break;
            }
        }
    });
}

fn is_gecko_alive_for_id(id: Option<u32>, mut send: impl FnMut(&mut u16) -> bool) -> i32 {
    if id != Some(0) {
        return 0;
    }

    let mut val = 0x9000u16;
    if !send(&mut val) {
        return 0;
    }
    (val & 0x0470 != 0) as i32
}

pub fn is_gecko_alive(chn: i32) -> i32 {
    if channel_index(chn).is_none() {
        return 0;
    }

    // BUG FIX: C requires EXI_GetID success with device ID zero before probing.
    let id = exi::read_id(chn, exi::EXI_DEVICE_0).ok();
    if id != Some(0) {
        return 0;
    }

    with_gecko_lock(chn, || {
        is_gecko_alive_for_id(id, |val| send_command(chn, val))
    })
    .unwrap_or(0)
}

fn transfer_bytes(chn: i32, buffer: *mut c_void, size: i32, retries: i32, safe: bool) -> i32 {
    if buffer.is_null() || size < 0 {
        return 0;
    }
    if size == 0 || channel_index(chn).is_none() {
        return 0;
    }

    let mut retries_left = retries;
    let mut copied = 0usize;
    let total = size as usize;
    let bytes = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), total) };

    let Some(result) = with_gecko_lock(chn, || {
        while copied < total {
            if safe && !check_recv(chn) {
                if retries_left >= 0 {
                    retries_left -= 1;
                    if retries_left == 0 {
                        break;
                    }
                }
                yield_thread();
                continue;
            }

            if !recv_byte(chn, &mut bytes[copied]) {
                break;
            }
            copied += 1;

            if retries_left >= 0 {
                retries_left -= 1;
                if retries_left == 0 {
                    break;
                }
            }
        }
        copied as i32
    }) else {
        return 0;
    };

    result
}

fn send_bytes(chn: i32, buffer: *const c_void, size: i32, retries: i32, safe: bool) -> i32 {
    if buffer.is_null() || size < 0 {
        return 0;
    }
    if size == 0 || channel_index(chn).is_none() {
        return 0;
    }

    let mut retries_left = retries;
    let mut sent = 0usize;
    let total = size as usize;
    let bytes = unsafe { slice::from_raw_parts(buffer.cast::<u8>(), total) };

    let Some(result) = with_gecko_lock(chn, || {
        while sent < total {
            if safe && !check_send(chn) {
                if retries_left >= 0 {
                    retries_left -= 1;
                    if retries_left == 0 {
                        break;
                    }
                }
                yield_thread();
                continue;
            }

            if !send_byte(chn, bytes[sent]) {
                break;
            }
            sent += 1;

            if retries_left >= 0 {
                retries_left -= 1;
                if retries_left == 0 {
                    break;
                }
            }
        }
        sent as i32
    }) else {
        return 0;
    };

    result
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn receive_buffer(chn: i32, buffer: *mut c_void, size: i32) -> i32 {
    transfer_bytes(chn, buffer, size, -1, false)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn send_buffer(chn: i32, buffer: *const c_void, size: i32) -> i32 {
    send_bytes(chn, buffer, size, -1, false)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn recvbuffer_safe(chn: i32, buffer: *mut c_void, size: i32) -> i32 {
    transfer_bytes(chn, buffer, size, -1, true)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn sendbuffer_safe(chn: i32, buffer: *const c_void, size: i32) -> i32 {
    send_bytes(chn, buffer, size, -1, true)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn recvbuffer_ex(chn: i32, buffer: *mut c_void, size: i32, retries: i32) -> i32 {
    transfer_bytes(chn, buffer, size, retries, false)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn sendbuffer_ex(chn: i32, buffer: *const c_void, size: i32, retries: i32) -> i32 {
    send_bytes(chn, buffer, size, retries, false)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn recvbuffer_safe_ex(chn: i32, buffer: *mut c_void, size: i32, retries: i32) -> i32 {
    transfer_bytes(chn, buffer, size, retries, true)
}

/// # Safety
///
/// `buffer` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn sendbuffer_safe_ex(chn: i32, buffer: *const c_void, size: i32, retries: i32) -> i32 {
    send_bytes(chn, buffer, size, retries, true)
}

/// # Safety
///
/// `buffer` must be null only when `length` is zero. Otherwise, it must be properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn flashread(chn: i32, offset: u32, buffer: *mut c_void, length: usize) -> i32 {
    if !flash_buffer_is_valid(buffer.is_null(), length) || channel_index(chn).is_none() {
        return 0;
    }

    let Some(result) = with_gecko_lock(chn, || {
        if let Some(result) = zero_length_flash_result(length) {
            return result;
        }
        let out = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), length) };
        let mut ok = true;
        let mut index = 0usize;

        while index < length {
            let mut byte = 0u8;
            if !flash_read_command(chn, offset.wrapping_add(index as u32), &mut byte) {
                ok = false;
                break;
            }
            out[index] = byte;
            index += 1;
        }

        if ok { 1 } else { 0 }
    }) else {
        return 0;
    };

    result
}

/// # Safety
///
/// `buffer` must be null only when `length` is zero. Otherwise, it must be properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn flashwrite(chn: i32, offset: u32, buffer: *const c_void, length: usize) -> i32 {
    if !flash_buffer_is_valid(buffer.is_null(), length) || channel_index(chn).is_none() {
        return 0;
    }

    let Some(result) = with_gecko_lock(chn, || {
        if let Some(result) = zero_length_flash_result(length) {
            return result;
        }
        let input = unsafe { slice::from_raw_parts(buffer.cast::<u8>(), length) };
        let mut index = 0usize;
        while index < length {
            if !flash_program_byte(chn, offset.wrapping_add(index as u32), input[index]) {
                return 0;
            }
            delay_microseconds(20);

            let mut verify = 0u8;
            if !flash_read_command(chn, offset.wrapping_add(index as u32), &mut verify)
                || verify != input[index]
            {
                return 0;
            }
            index += 1;
        }

        1
    }) else {
        return 0;
    };

    result
}

pub fn flashverify(chn: i32) -> i32 {
    if channel_index(chn).is_none() {
        return 0;
    }

    with_gecko_lock(chn, || {
        if !flash_software_id_entry(chn) {
            return 0;
        }

        let mut id = [0u8; 2];
        let ok = flash_read_command(chn, 0, &mut id[0])
            && flash_read_command(chn, 1, &mut id[1])
            && id == [0xBF, 0xD7]
            && flash_software_id_exit(chn);
        ok as i32
    })
    .unwrap_or(0)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    #[test]
    fn exi_lock_contention_sleeps_on_callback_queue() {
        let lock_attempted = core::cell::Cell::new(false);
        let callback_registered = core::cell::Cell::new(false);
        let queue_slept = core::cell::Cell::new(false);
        let result = super::lock_or_sleep(
            12,
            |callback| {
                lock_attempted.set(true);
                callback_registered.set(callback.is_some());
                false
            },
            |queue| {
                assert_eq!(queue, 12);
                queue_slept.set(true);
                super::lwp::LWP_SUCCESSFUL
            },
        );
        assert_eq!(result, Ok(false));
        assert!(lock_attempted.get());
        assert!(callback_registered.get());
        assert!(queue_slept.get());
    }

    #[test]
    fn exi_lock_retries_after_callback_wakeup() {
        let tries = core::cell::Cell::new(0);
        let wakeups = core::cell::Cell::new(0);
        let result = super::retry_lock_after_wakeup(|| {
            tries.set(tries.get() + 1);
            if tries.get() == 1 {
                wakeups.set(wakeups.get() + 1);
                Ok(false)
            } else {
                Ok(true)
            }
        });

        assert_eq!(result, Ok(()));
        assert_eq!(tries.get(), 2);
        assert_eq!(wakeups.get(), 1);
    }

    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    fn test_guard() -> TestGuard<'static> {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        TestGuard { lock: &TEST_LOCK }
    }

    struct TestGuard<'a> {
        lock: &'a AtomicBool,
    }

    impl Drop for TestGuard<'_> {
        fn drop(&mut self) {
            self.lock.store(false, Ordering::Release);
        }
    }

    #[test]
    fn helpers_exist() {
        let _guard = test_guard();
        let _ = flush as fn(i32);
        let _ = is_gecko_alive as fn(i32) -> i32;
        let _ = receive_buffer as fn(i32, *mut c_void, i32) -> i32;
        let _ = send_buffer as fn(i32, *const c_void, i32) -> i32;
        let _ = recvbuffer_safe as fn(i32, *mut c_void, i32) -> i32;
        let _ = sendbuffer_safe as fn(i32, *const c_void, i32) -> i32;
        let _ = recvbuffer_ex as fn(i32, *mut c_void, i32, i32) -> i32;
        let _ = sendbuffer_ex as fn(i32, *const c_void, i32, i32) -> i32;
        let _ = recvbuffer_safe_ex as fn(i32, *mut c_void, i32, i32) -> i32;
        let _ = sendbuffer_safe_ex as fn(i32, *const c_void, i32, i32) -> i32;
        let _ = flashread as fn(i32, u32, *mut c_void, usize) -> i32;
        let _ = flashwrite as fn(i32, u32, *const c_void, usize) -> i32;
        let _ = flashverify as fn(i32) -> i32;
    }

    #[test]
    fn flush_matches_void_c_signature_and_ignores_invalid_channel() {
        let _guard = test_guard();
        let _: fn(i32) = flush;
        flush(-1);
        flush(CHANNELS as i32);
    }

    #[test]
    fn invalid_channels_and_null_buffers_are_rejected() {
        let _guard = test_guard();
        assert_eq!(is_gecko_alive(-1), 0);
        assert_eq!(is_gecko_alive(3), 0);

        assert_eq!(flashread(-1, 0, core::ptr::null_mut(), 1), 0);
        assert_eq!(flashwrite(-1, 0, core::ptr::null(), 1), 0);
        assert_eq!(flashverify(-1), 0);
        assert_eq!(receive_buffer(-1, core::ptr::null_mut(), 1), 0);
        assert_eq!(send_buffer(-1, core::ptr::null(), 1), 0);
    }

    #[test]
    fn zero_length_flash_accepts_null_without_accessing_the_buffer() {
        let _guard = test_guard();
        assert!(flash_buffer_is_valid(true, 0));
        assert!(!flash_buffer_is_valid(true, 1));
        assert!(flash_buffer_is_valid(false, 0));
        assert_eq!(zero_length_flash_result(0), Some(1));
        assert_eq!(zero_length_flash_result(1), None);
    }

    #[test]
    fn flash_program_byte_emits_unlock_setup_then_target_command() {
        let _guard = test_guard();
        let mut commands = [(0u32, 0u8); 4];
        let mut count = 0;
        assert!(flash_program_byte_with(0x3456, 0xA7, |address, data| {
            commands[count] = (address, data);
            count += 1;
            true
        }));
        assert_eq!(count, 4);
        assert_eq!(
            commands,
            [
                (0x5555, 0xAA),
                (0x2AAA, 0x55),
                (0x5555, 0xA0),
                (0x3456, 0xA7),
            ]
        );
    }

    #[test]
    fn alive_probe_requires_successful_zero_exi_id_before_sending_command() {
        let _guard = test_guard();
        let mut sends = 0;
        for id in [None, Some(1)] {
            assert_eq!(
                is_gecko_alive_for_id(id, |_| {
                    sends += 1;
                    true
                }),
                0
            );
        }
        assert_eq!(sends, 0);

        assert_eq!(
            is_gecko_alive_for_id(Some(0), |command| {
                sends += 1;
                assert_eq!(*command, 0x9000);
                *command = 0x0470;
                true
            }),
            1
        );
        assert_eq!(sends, 1);
    }

    #[test]
    fn receive_byte_zeroes_output_on_command_failure_or_not_ready() {
        let _guard = test_guard();

        let mut byte = 0xA5;
        assert!(!recv_byte_with(&mut byte, |_| false));
        assert_eq!(byte, 0);

        byte = 0xA5;
        assert!(!recv_byte_with(&mut byte, |command| {
            assert_eq!(*command, 0xA000);
            *command = 0x0034;
            true
        }));
        assert_eq!(byte, 0);

        byte = 0xA5;
        assert!(recv_byte_with(&mut byte, |command| {
            *command = 0x0800 | 0x0034;
            true
        }));
        assert_eq!(byte, 0x34);
    }
}
