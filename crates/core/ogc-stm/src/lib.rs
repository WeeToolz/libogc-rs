#![no_std]
#![cfg(not(gamecube))]

#[cfg(not(target_arch = "powerpc"))]
compile_error!("This crate targets Wii PowerPC only. Use the powerpc-nintendo-none target.");

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_ipc::{FileDescriptor, IPC_OPEN_NONE, IoctlBuffers, open_file};

#[cfg(any(test, doctest))]
const IPC_HEAP_BYTES: usize = 4096;

#[cfg(any(test, doctest))]
#[repr(align(32))]
struct HostIpcBuffer([u8; IPC_HEAP_BYTES]);

#[cfg(any(test, doctest))]
static mut HOST_IPC_BUFFER: HostIpcBuffer = HostIpcBuffer([0; IPC_HEAP_BYTES]);

#[cfg(any(test, doctest))]
unsafe fn _sys_get_ipc_buffer_lo() -> *mut c_void {
    core::ptr::addr_of_mut!(HOST_IPC_BUFFER.0).cast::<c_void>()
}

#[cfg(any(test, doctest))]
unsafe fn _sys_get_ipc_buffer_hi() -> *mut c_void {
    unsafe {
        core::ptr::addr_of_mut!(HOST_IPC_BUFFER.0)
            .cast::<u8>()
            .add(IPC_HEAP_BYTES)
            .cast::<c_void>()
    }
}

pub use ogc_sys::ErrorCode;

pub type StmResult<T> = ogc_sys::Result<T>;

pub const STM_EVENT_RESET: u32 = 0x0002_0000;
pub const STM_EVENT_POWER: u32 = 0x0000_0800;

pub const STM_EINVAL: i32 = -0x2004;
pub const STM_ENOTINIT: i32 = -0x2100;
pub const STM_ENOHANDLER: i32 = -0x2101;

const IOCTL_STM_EVENTHOOK: u32 = 0x1000;
const IOCTL_STM_HOTRESET: u32 = 0x2001;
const IOCTL_STM_SHUTDOWN: u32 = 0x2003;
const IOCTL_STM_IDLE: u32 = 0x2004;
const IOCTL_STM_RELEASE_EH: u32 = 0x3002;
const IOCTL_STM_LEDMODE: u32 = 0x6002;

pub type StmCallback = Option<unsafe extern "C" fn(event: u32)>;

#[repr(align(32))]
#[derive(Copy, Clone)]
struct StmBuffer([u32; 8]);

impl StmBuffer {
    const fn zeroed() -> Self {
        Self([0; 8])
    }
}

impl Default for StmBuffer {
    fn default() -> Self {
        Self::zeroed()
    }
}

#[derive(Default)]
struct StmState {
    imm_fd: Option<FileDescriptor>,
    eh_fd: Option<FileDescriptor>,
    initialized: bool,
    eh_registered: bool,
    eh_clear: bool,
    event_cb: StmCallback,
    eh_in: StmBuffer,
    eh_out: StmBuffer,
    imm_in: StmBuffer,
    imm_out: StmBuffer,
}

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) -> SpinGuard<'_> {
        while self
            .0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a> {
    lock: &'a SpinLock,
}

impl Drop for SpinGuard<'_> {
    fn drop(&mut self) {
        self.lock.0.store(false, Ordering::Release);
    }
}

static STM_LOCK: SpinLock = SpinLock::new();
static mut STM_STATE: StmState = StmState {
    imm_fd: None,
    eh_fd: None,
    initialized: false,
    eh_registered: false,
    eh_clear: false,
    event_cb: None,
    eh_in: StmBuffer::zeroed(),
    eh_out: StmBuffer::zeroed(),
    imm_in: StmBuffer::zeroed(),
    imm_out: StmBuffer::zeroed(),
};
fn state_ptr() -> *mut StmState {
    core::ptr::addr_of_mut!(STM_STATE)
}

fn fd(fd: &Option<FileDescriptor>) -> Option<FileDescriptor> {
    fd.as_ref().map(|file| FileDescriptor::new(file.raw()))
}

fn path(bytes: &'static [u8]) -> &'static core::ffi::CStr {
    match core::ffi::CStr::from_bytes_with_nul(bytes) {
        Ok(path) => path,
        Err(_) => unreachable!(),
    }
}

fn imm_fd() -> core::result::Result<FileDescriptor, i32> {
    open_file(path(b"/dev/stm/immediate\0"), IPC_OPEN_NONE)
}

fn eh_fd() -> core::result::Result<FileDescriptor, i32> {
    open_file(path(b"/dev/stm/eventhook\0"), IPC_OPEN_NONE)
}

fn set_callback(cb: StmCallback) -> StmCallback {
    let state = unsafe { &mut *state_ptr() };
    let old = state.event_cb;
    state.event_cb = cb;
    old
}

unsafe extern "C" fn event_hook_handler(result: i32, _usrdata: *mut c_void) -> i32 {
    unsafe {
        let (event, cb) = {
            let _guard = STM_LOCK.lock();
            let state = { &mut *state_ptr() };
            state.eh_registered = false;

            if result < 0 {
                return result;
            }

            if state.eh_clear {
                return 0;
            }

            (state.eh_out.0[0], state.event_cb)
        };

        if let Some(cb) = cb {
            cb(event);
        }
        let _ = _stm_set_event_hook();
        0
    }
}

fn ioctl_buffers<'a>(in_buf: &'a mut StmBuffer, out_buf: &'a mut StmBuffer) -> IoctlBuffers<'a> {
    let in_bytes = unsafe {
        core::slice::from_raw_parts_mut(
            in_buf.0.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&in_buf.0),
        )
    };
    let out_bytes = unsafe {
        core::slice::from_raw_parts_mut(
            out_buf.0.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&out_buf.0),
        )
    };
    IoctlBuffers::from_slices(Some(in_bytes), Some(out_bytes))
}

fn _stm_set_event_hook_locked(state: &mut StmState) -> i32 {
    if !state.initialized {
        return STM_ENOTINIT;
    }

    state.eh_clear = false;
    let Some(fd) = fd(&state.eh_fd) else {
        return STM_ENOTINIT;
    };

    let buffers = ioctl_buffers(&mut state.eh_in, &mut state.eh_out);
    let result = unsafe {
        fd.ioctl_async(
            IOCTL_STM_EVENTHOOK as i32,
            buffers,
            Some(event_hook_handler),
            core::ptr::null_mut(),
        )
    };
    state.eh_registered = result >= 0;
    result
}

fn _stm_set_event_hook() -> i32 {
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    _stm_set_event_hook_locked(state)
}

fn _stm_release_event_hook_locked(state: &mut StmState) -> i32 {
    if !state.initialized {
        return STM_ENOTINIT;
    }
    if !state.eh_registered {
        return STM_ENOHANDLER;
    }
    let Some(fd) = fd(&state.imm_fd) else {
        return STM_ENOTINIT;
    };

    state.eh_clear = true;
    let buffers = ioctl_buffers(&mut state.imm_in, &mut state.imm_out);
    let result = fd.ioctl(IOCTL_STM_RELEASE_EH as i32, buffers);
    if result >= 0 {
        state.eh_registered = false;
    }
    result
}

fn _stm_release_event_hook() -> i32 {
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    _stm_release_event_hook_locked(state)
}

fn set_led_mode_input(mode: u32) {
    let state = unsafe { &mut *state_ptr() };
    state.imm_in.0[0] = mode;
}

pub fn init() -> i32 {
    init_with_openers(
        imm_fd,
        eh_fd,
        true,
        _stm_set_event_hook_locked,
        FileDescriptor::close,
    )
}

pub fn init_without_event_hook() -> i32 {
    init_with_openers(
        imm_fd,
        eh_fd,
        false,
        _stm_set_event_hook_locked,
        FileDescriptor::close,
    )
}

pub fn try_init() -> StmResult<()> {
    let result = init();
    if result > 0 {
        Ok(())
    } else {
        Err(ErrorCode::new(if result < 0 {
            result
        } else {
            STM_ENOTINIT
        }))
    }
}

fn init_with_open_results(
    imm: core::result::Result<FileDescriptor, i32>,
    eh: core::result::Result<FileDescriptor, i32>,
    install_event_hook: bool,
    set_event_hook: fn(&mut StmState) -> i32,
    close_fd: fn(FileDescriptor) -> core::result::Result<(), i32>,
) -> i32 {
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    if state.initialized {
        drop(_guard);
        if let Ok(fd) = imm {
            let _ = close_fd(fd);
        }
        if let Ok(fd) = eh {
            let _ = close_fd(fd);
        }
        return 1;
    }

    state.eh_clear = false;
    state.eh_registered = false;

    let imm = match imm {
        Ok(fd) => fd,
        Err(open_error) => {
            drop(_guard);
            if let Ok(fd) = eh {
                let raw = fd.raw();
                if let Err(close_error) = close_fd(FileDescriptor::new(raw)) {
                    let _guard = STM_LOCK.lock();
                    let state = unsafe { &mut *state_ptr() };
                    state.eh_fd = Some(FileDescriptor::new(raw));
                    return close_error;
                }
            }
            return open_error;
        }
    };
    let eh = match eh {
        Ok(fd) => fd,
        Err(open_error) => {
            drop(_guard);
            let raw = imm.raw();
            if let Err(close_error) = close_fd(FileDescriptor::new(raw)) {
                let _guard = STM_LOCK.lock();
                let state = unsafe { &mut *state_ptr() };
                state.imm_fd = Some(FileDescriptor::new(raw));
                return close_error;
            }
            return open_error;
        }
    };

    state.imm_fd = Some(imm);
    state.eh_fd = Some(eh);
    state.initialized = true;

    let hook_result = if install_event_hook {
        set_event_hook(state)
    } else {
        0
    };
    drop(_guard);
    if hook_result < 0 {
        let cleanup = close_with_operations(_stm_release_event_hook_locked, close_fd);
        return if cleanup < 0 { cleanup } else { hook_result };
    }
    1
}

fn init_with_openers(
    open_immediate: impl FnOnce() -> core::result::Result<FileDescriptor, i32>,
    open_eventhook: impl FnOnce() -> core::result::Result<FileDescriptor, i32>,
    install_event_hook: bool,
    set_event_hook: fn(&mut StmState) -> i32,
    close_fd: fn(FileDescriptor) -> core::result::Result<(), i32>,
) -> i32 {
    let has_partial_open = {
        let _guard = STM_LOCK.lock();
        let state = unsafe { &mut *state_ptr() };
        if state.initialized {
            return 1;
        }
        state.imm_fd.is_some() || state.eh_fd.is_some()
    };
    if has_partial_open {
        let cleanup = close_with_operations(_stm_release_event_hook_locked, close_fd);
        if cleanup < 0 {
            return cleanup;
        }
    }

    let imm = match open_immediate() {
        Ok(fd) => fd,
        Err(error) => return error,
    };
    let eh = match open_eventhook() {
        Ok(fd) => fd,
        Err(error) => {
            if let Err(close_error) = close_fd(FileDescriptor::new(imm.raw())) {
                let _guard = STM_LOCK.lock();
                let state = unsafe { &mut *state_ptr() };
                state.imm_fd = Some(imm);
                return close_error;
            }
            return error;
        }
    };
    init_with_open_results(
        Ok(imm),
        Ok(eh),
        install_event_hook,
        set_event_hook,
        close_fd,
    )
}

pub fn close() -> i32 {
    close_with_operations(_stm_release_event_hook_locked, FileDescriptor::close)
}

fn close_with_operations(
    release_event_hook: fn(&mut StmState) -> i32,
    close_fd: fn(FileDescriptor) -> core::result::Result<(), i32>,
) -> i32 {
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };

    let mut ret = 0;
    if state.eh_registered {
        // C ignores the event-hook release result and reports IOS_Close errors only.
        let _ = release_event_hook(state);
    }
    if let Some(raw) = state.imm_fd.as_ref().map(FileDescriptor::raw) {
        if let Err(err) = close_fd(FileDescriptor::new(raw)) {
            // BUG FIX: C lets each later IOS_Close error replace the earlier result.
            ret = err;
        } else {
            state.imm_fd = None;
        }
    }
    if let Some(raw) = state.eh_fd.as_ref().map(FileDescriptor::raw) {
        if let Err(err) = close_fd(FileDescriptor::new(raw)) {
            ret = err;
        } else {
            state.eh_fd = None;
        }
    }
    state.initialized = false;
    state.eh_registered = false;
    state.eh_clear = false;
    ret
}

pub fn try_close() -> StmResult<()> {
    ogc_sys::unit_from_status(close())
}

pub fn register_event_handler(newhandler: StmCallback) -> StmCallback {
    set_callback(newhandler)
}

pub fn shutdown_to_standby() -> i32 {
    unsafe {
        clear_video();
    }
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    if !state.initialized {
        return STM_ENOTINIT;
    }
    let Some(fd) = fd(&state.imm_fd) else {
        return STM_ENOTINIT;
    };
    state.imm_in.0[0] = 0;
    fd.ioctl(
        IOCTL_STM_SHUTDOWN as i32,
        ioctl_buffers(&mut state.imm_in, &mut state.imm_out),
    )
}

pub fn try_shutdown_to_standby() -> StmResult<()> {
    ogc_sys::unit_from_status(shutdown_to_standby())
}

pub fn shutdown_to_idle() -> i32 {
    unsafe {
        clear_video();
    }
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    if !state.initialized {
        return STM_ENOTINIT;
    }
    let Some(fd) = fd(&state.imm_fd) else {
        return STM_ENOTINIT;
    };
    state.imm_in.0[0] = match unsafe { ogc_sys::sys_get_hollywood_revision() } {
        0..=2 => 0xFCA0_8280,
        _ => 0xFCE0_82C0,
    };
    fd.ioctl(
        IOCTL_STM_IDLE as i32,
        ioctl_buffers(&mut state.imm_in, &mut state.imm_out),
    )
}

pub fn try_shutdown_to_idle() -> StmResult<()> {
    ogc_sys::unit_from_status(shutdown_to_idle())
}

pub fn set_led_mode(mode: u32) -> i32 {
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    if !state.initialized {
        return STM_ENOTINIT;
    }
    let Some(fd) = fd(&state.imm_fd) else {
        return STM_ENOTINIT;
    };
    set_led_mode_input(mode);
    fd.ioctl(
        IOCTL_STM_LEDMODE as i32,
        ioctl_buffers(&mut state.imm_in, &mut state.imm_out),
    )
}

pub fn try_set_led_mode(mode: u32) -> StmResult<()> {
    ogc_sys::unit_from_status(set_led_mode(mode))
}

pub fn reboot_system() -> i32 {
    unsafe {
        clear_video();
    }
    let _guard = STM_LOCK.lock();
    let state = unsafe { &mut *state_ptr() };
    if !state.initialized {
        return STM_ENOTINIT;
    }
    let Some(fd) = fd(&state.imm_fd) else {
        return STM_ENOTINIT;
    };
    state.imm_in.0[0] = 0;
    fd.ioctl(
        IOCTL_STM_HOTRESET as i32,
        ioctl_buffers(&mut state.imm_in, &mut state.imm_out),
    )
}

pub fn try_reboot_system() -> StmResult<()> {
    ogc_sys::unit_from_status(reboot_system())
}

/// # Safety
///
/// Directly touches the VI control register used by STM shutdown paths.
/// BUG FIX: C clears VI before it checks whether STM has initialized.
unsafe fn clear_video() {
    unsafe {
        core::ptr::write_volatile(0xCC00_2002 as *mut u16, 0);
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::cell::Cell;
    use core::sync::atomic::{AtomicU32, Ordering};

    const TEST_CLOSE_ERROR: i32 = -0x2200;
    static CLOSED_FDS: AtomicU32 = AtomicU32::new(0);

    fn reset_test_state() {
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE = StmState::default();
        }
        CLOSED_FDS.store(0, Ordering::Release);
    }

    fn event_hook_succeeds(state: &mut StmState) -> i32 {
        state.eh_registered = true;
        0
    }

    fn event_hook_fails(state: &mut StmState) -> i32 {
        state.eh_registered = false;
        STM_EINVAL
    }

    fn event_hook_must_not_run(_state: &mut StmState) -> i32 {
        panic!("event hook was installed for no-hook initialization")
    }

    fn record_close(fd: FileDescriptor) -> core::result::Result<(), i32> {
        CLOSED_FDS.fetch_or(1 << fd.raw(), Ordering::AcqRel);
        Ok(())
    }

    fn record_close_with_imm_failure(fd: FileDescriptor) -> core::result::Result<(), i32> {
        CLOSED_FDS.fetch_or(1 << fd.raw(), Ordering::AcqRel);
        if fd.raw() == 1 {
            Err(TEST_CLOSE_ERROR)
        } else {
            Ok(())
        }
    }

    #[test]
    fn constants_match_header() {
        assert_eq!(STM_EVENT_RESET, 0x0002_0000);
        assert_eq!(STM_EVENT_POWER, 0x0000_0800);
    }

    #[test]
    fn set_led_mode_preserves_full_word() {
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE = StmState::default();
            set_led_mode_input(0x1234_5678);
        }

        unsafe {
            let _guard = STM_LOCK.lock();
            assert_eq!(STM_STATE.imm_in.0[0], 0x1234_5678);
            STM_STATE = StmState::default();
        }
    }

    #[test]
    fn init_failure_preserves_open_error() {
        reset_test_state();

        assert_eq!(
            init_with_open_results(
                Err(STM_EINVAL),
                Err(STM_EINVAL),
                true,
                event_hook_succeeds,
                record_close,
            ),
            STM_EINVAL
        );
    }

    #[test]
    fn event_hook_failure_closes_descriptors_and_returns_error() {
        reset_test_state();

        assert_eq!(
            init_with_open_results(
                Ok(FileDescriptor::new(1)),
                Ok(FileDescriptor::new(2)),
                true,
                event_hook_fails,
                record_close,
            ),
            STM_EINVAL
        );

        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(!STM_STATE.initialized);
            assert!(!STM_STATE.eh_registered);
            assert!(STM_STATE.imm_fd.is_none());
            assert!(STM_STATE.eh_fd.is_none());
        }
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), 6);
    }

    #[test]
    fn successful_event_hook_completes_initialization() {
        reset_test_state();

        assert_eq!(
            init_with_open_results(
                Ok(FileDescriptor::new(1)),
                Ok(FileDescriptor::new(2)),
                true,
                event_hook_succeeds,
                record_close,
            ),
            1
        );

        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(STM_STATE.initialized);
            assert!(STM_STATE.eh_registered);
            STM_STATE = StmState::default();
        }
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), 0);
    }

    #[test]
    fn no_hook_initialization_skips_event_registration() {
        reset_test_state();

        assert_eq!(
            init_with_open_results(
                Ok(FileDescriptor::new(1)),
                Ok(FileDescriptor::new(2)),
                false,
                event_hook_must_not_run,
                record_close,
            ),
            1
        );

        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(STM_STATE.initialized);
            assert!(!STM_STATE.eh_registered);
            STM_STATE = StmState::default();
        }
    }

    #[test]
    fn repeated_initialization_does_not_retry_event_registration() {
        reset_test_state();
        assert_eq!(
            init_with_open_results(
                Ok(FileDescriptor::new(1)),
                Ok(FileDescriptor::new(2)),
                false,
                event_hook_must_not_run,
                record_close,
            ),
            1
        );

        assert_eq!(
            init_with_open_results(
                Err(STM_EINVAL),
                Err(STM_EINVAL),
                true,
                event_hook_must_not_run,
                record_close,
            ),
            1
        );
        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(STM_STATE.initialized);
            assert!(!STM_STATE.eh_registered);
            assert_eq!(STM_STATE.imm_fd.as_ref().map(FileDescriptor::raw), Some(1));
            assert_eq!(STM_STATE.eh_fd.as_ref().map(FileDescriptor::raw), Some(2));
        }
    }

    #[test]
    fn repeated_initialization_returns_before_opening_devices() {
        reset_test_state();
        assert_eq!(
            init_with_open_results(
                Ok(FileDescriptor::new(1)),
                Ok(FileDescriptor::new(2)),
                false,
                event_hook_must_not_run,
                record_close,
            ),
            1
        );

        let open_calls = Cell::new(0);
        assert_eq!(
            init_with_openers(
                || {
                    open_calls.set(open_calls.get() + 1);
                    Err(STM_EINVAL)
                },
                || {
                    open_calls.set(open_calls.get() + 1);
                    Err(STM_EINVAL)
                },
                true,
                event_hook_must_not_run,
                record_close,
            ),
            1
        );
        assert_eq!(open_calls.get(), 0);
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), 0);

        let (initialized, imm_fd, eh_fd) = unsafe {
            let _guard = STM_LOCK.lock();
            let state = &*state_ptr();
            (
                state.initialized,
                state.imm_fd.as_ref().map(FileDescriptor::raw),
                state.eh_fd.as_ref().map(FileDescriptor::raw),
            )
        };
        assert!(initialized);
        assert_eq!(imm_fd, Some(1));
        assert_eq!(eh_fd, Some(2));
        reset_test_state();
    }

    #[test]
    fn immediate_open_failure_does_not_attempt_eventhook_open() {
        reset_test_state();
        let eventhook_opened = Cell::new(false);
        assert_eq!(
            init_with_openers(
                || Err(STM_EINVAL),
                || {
                    eventhook_opened.set(true);
                    Ok(FileDescriptor::new(2))
                },
                true,
                event_hook_must_not_run,
                record_close,
            ),
            STM_EINVAL
        );
        assert!(!eventhook_opened.get());
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), 0);
    }

    #[test]
    fn failed_cleanup_keeps_descriptor_for_close_retry() {
        reset_test_state();
        assert_eq!(
            init_with_openers(
                || Ok(FileDescriptor::new(1)),
                || Err(STM_EINVAL),
                true,
                event_hook_must_not_run,
                record_close_with_imm_failure,
            ),
            TEST_CLOSE_ERROR
        );
        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(!STM_STATE.initialized);
            assert_eq!(STM_STATE.imm_fd.as_ref().map(FileDescriptor::raw), Some(1));
        }
        assert_eq!(
            close_with_operations(event_hook_must_not_run, record_close),
            0
        );
        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(STM_STATE.imm_fd.is_none());
        }
    }

    #[test]
    fn close_ignores_event_hook_release_failure_and_closes_descriptors() {
        reset_test_state();
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE.initialized = true;
            STM_STATE.eh_registered = true;
            STM_STATE.imm_fd = Some(FileDescriptor::new(1));
            STM_STATE.eh_fd = Some(FileDescriptor::new(2));
        }

        assert_eq!(
            close_with_operations(event_hook_fails, record_close_with_imm_failure),
            TEST_CLOSE_ERROR
        );
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), (1 << 1) | (1 << 2));
        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(!STM_STATE.initialized);
            assert!(!STM_STATE.eh_registered);
            assert_eq!(STM_STATE.imm_fd.as_ref().map(FileDescriptor::raw), Some(1));
            assert!(STM_STATE.eh_fd.is_none());
        }
    }

    #[test]
    fn close_ignores_event_hook_release_failure_when_closes_succeed() {
        reset_test_state();
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE.initialized = true;
            STM_STATE.eh_registered = true;
            STM_STATE.imm_fd = Some(FileDescriptor::new(1));
            STM_STATE.eh_fd = Some(FileDescriptor::new(2));
        }

        assert_eq!(close_with_operations(event_hook_fails, record_close), 0);
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), (1 << 1) | (1 << 2));
    }

    #[test]
    fn close_returns_descriptor_failure_and_continues_cleanup() {
        reset_test_state();
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE.initialized = true;
            STM_STATE.imm_fd = Some(FileDescriptor::new(1));
            STM_STATE.eh_fd = Some(FileDescriptor::new(2));
        }

        assert_eq!(
            close_with_operations(event_hook_must_not_run, record_close_with_imm_failure),
            TEST_CLOSE_ERROR
        );
        assert_eq!(CLOSED_FDS.load(Ordering::Acquire), (1 << 1) | (1 << 2));
        unsafe {
            let _guard = STM_LOCK.lock();
            assert!(!STM_STATE.initialized);
            assert!(!STM_STATE.eh_registered);
            assert_eq!(STM_STATE.imm_fd.as_ref().map(FileDescriptor::raw), Some(1));
            assert!(STM_STATE.eh_fd.is_none());
        }
    }

    #[test]
    fn result_helpers_preserve_stm_init_semantics() {
        unsafe {
            let _guard = STM_LOCK.lock();
            STM_STATE = StmState::default();
        }

        assert_eq!(try_set_led_mode(0), Err(ErrorCode::new(STM_ENOTINIT)));
        assert_eq!(try_shutdown_to_standby(), Err(ErrorCode::new(STM_ENOTINIT)));
        assert_eq!(try_shutdown_to_idle(), Err(ErrorCode::new(STM_ENOTINIT)));
        assert_eq!(try_reboot_system(), Err(ErrorCode::new(STM_ENOTINIT)));
    }

    #[test]
    fn event_callback_type_is_public_api() {
        unsafe extern "C" fn cb(_event: u32) {}

        let callback: StmCallback = Some(cb);
        let previous = register_event_handler(callback);
        assert!(previous.is_none());
        assert!(register_event_handler(None).is_some());
    }
}
