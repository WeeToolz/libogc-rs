//! Dolphin-only test of a caller-owned request when IOS is replaced.
use crate::probe_support::ProbeLogger;
use core::ffi::c_void;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use ogc_ipc as ipc;

const EVENT_HOOK: i32 = 0x1000;
const RELEASE_HOOK: i32 = 0x3002;
const IPC_EEXIST: i32 = -2;
const UNSET: i32 = i32::MIN;
const TIMER_HZ: u64 = 60_750_000;
#[repr(C, align(32))]
struct Buffer([u8; 32]);
// Keep storage valid for the entire probe, including an abandoned request.
static mut OLD_IN: Buffer = Buffer([0; 32]);
static mut OLD_OUT: Buffer = Buffer([0xa5; 32]);
static mut NEW_IN: Buffer = Buffer([0; 32]);
static mut NEW_OUT: Buffer = Buffer([0xa5; 32]);
static OLD_COUNT: AtomicU32 = AtomicU32::new(0);
static NEW_COUNT: AtomicU32 = AtomicU32::new(0);
static NEW_RESULT: AtomicI32 = AtomicI32::new(UNSET);
unsafe extern "C" fn old_callback(result: i32, _: *mut c_void) -> i32 {
    OLD_COUNT.fetch_add(1, Ordering::Release);
    result
}
unsafe extern "C" fn new_callback(result: i32, _: *mut c_void) -> i32 {
    NEW_RESULT.store(result, Ordering::Relaxed);
    NEW_COUNT.fetch_add(1, Ordering::Release);
    result
}
fn open_hook() -> i32 {
    unsafe { ipc::IOS_Open(c"/dev/stm/eventhook".as_ptr(), ipc::IPC_OPEN_NONE) }
}
fn control(fd: i32, command: i32) -> i32 {
    let mut input = Buffer([0; 32]);
    let mut output = Buffer([0; 32]);
    unsafe {
        ipc::IOS_Ioctl(
            fd,
            command,
            input.0.as_mut_ptr().cast(),
            32,
            output.0.as_mut_ptr().cast(),
            32,
        )
    }
}
fn release_hook() -> bool {
    let fd = unsafe { ipc::IOS_Open(c"/dev/stm/immediate".as_ptr(), ipc::IPC_OPEN_NONE) };
    if fd < 0 {
        return false;
    }
    let result = control(fd, RELEASE_HOOK);
    let close = ipc::IOS_Close(fd);
    result == 0 && close == 0
}
pub(super) fn run(logger: &mut ProbeLogger, version: i32) {
    logger.section("Outstanding application IPC across IOS replacement");
    // Remove the library hook so reload shutdown cannot release our hook.
    let closed = ogc_stm::close();
    logger.record("pending reload closes library STM hook", closed == 0);
    if closed != 0 {
        return;
    }
    let old_fd = open_hook();
    logger.record("pending reload opens independent STM hook", old_fd >= 0);
    if old_fd < 0 {
        let _ = ogc_stm::init();
        return;
    }
    // SAFETY: Each request has separate, aligned, static buffers.
    let submitted = unsafe {
        ipc::IOS_IoctlAsync(
            old_fd,
            EVENT_HOOK,
            core::ptr::addr_of_mut!(OLD_IN).cast(),
            32,
            core::ptr::addr_of_mut!(OLD_OUT).cast(),
            32,
            Some(old_callback),
            core::ptr::null_mut(),
        )
    };
    // Dolphin returns EEXIST if the async hook reached IOS and is pending.
    // The runner has a timeout if this invariant fails.
    let duplicate = if submitted == 0 {
        control(old_fd, EVENT_HOOK)
    } else {
        UNSET
    };
    let pending =
        submitted == 0 && duplicate == IPC_EEXIST && OLD_COUNT.load(Ordering::Acquire) == 0;
    logger.note_i32("pending reload old submit", submitted);
    logger.note_i32("pending reload duplicate IOS reply", duplicate);
    logger.record(
        "pending reload old IOS holds real application request",
        pending,
    );
    if !pending {
        let _ = release_hook();
        let _ = ipc::IOS_Close(old_fd);
        let _ = ogc_stm::init();
        return;
    }
    // Leave the request pending. Same-version reload constructs a new IOS in Dolphin.
    let result = ogc_ios::reload_ios(version);
    logger.note_i32("pending reload result", result);
    logger.record(
        "pending reload replaces IOS successfully",
        result == 0 && ogc_ios::get_version() == version,
    );
    if result != 0 {
        return;
    }
    // The old numeric descriptor may refer to a different device now.
    let closed = ogc_stm::close();
    logger.record("pending reload new library STM hook closes", closed == 0);
    if closed != 0 {
        return;
    }
    let new_fd = open_hook();
    logger.record("pending reload opens new IOS hook", new_fd >= 0);
    if new_fd < 0 {
        let _ = ogc_stm::init();
        return;
    }
    let submitted = unsafe {
        ipc::IOS_IoctlAsync(
            new_fd,
            EVENT_HOOK,
            core::ptr::addr_of_mut!(NEW_IN).cast(),
            32,
            core::ptr::addr_of_mut!(NEW_OUT).cast(),
            32,
            Some(new_callback),
            core::ptr::null_mut(),
        )
    };
    let duplicate = if submitted == 0 {
        control(new_fd, EVENT_HOOK)
    } else {
        UNSET
    };
    logger.record(
        "pending reload new IOS holds new request",
        submitted == 0 && duplicate == IPC_EEXIST,
    );
    let released = release_hook();
    let start = ogc_system::time();
    while NEW_COUNT.load(Ordering::Acquire) == 0
        && ogc_system::time().wrapping_sub(start) < TIMER_HZ * 2
    {
        ipc::poll();
        ogc_lwp::sys::yield_thread();
    }
    let closed = ipc::IOS_Close(new_fd);
    let initialized = ogc_stm::init();
    logger.note_u32(
        "pending reload old callback count",
        OLD_COUNT.load(Ordering::Acquire),
    );
    logger.note_u32(
        "pending reload new callback count",
        NEW_COUNT.load(Ordering::Acquire),
    );
    logger.record(
        "pending reload new hook releases exactly once",
        released
            && NEW_COUNT.load(Ordering::Acquire) == 1
            && NEW_RESULT.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "pending reload old callback not delivered by new IOS",
        OLD_COUNT.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "pending reload restores STM and accepts fresh IPC",
        closed == 0
            && initialized >= 0
            && ipc::open_file(c"/dev/fs", ipc::IPC_OPEN_NONE).is_ok_and(|fd| fd.close().is_ok()),
    );
}
