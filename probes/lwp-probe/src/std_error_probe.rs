//! Controlled devoptab fault endpoint. It does not represent hardware acceptance.
use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use ogc_sysbase::{self as sysbase, DeviceFile, Reent};
use ogc_system_probe::probe_support::ProbeLogger;
use std::io::ErrorKind;

static OPEN_ERROR: AtomicI32 = AtomicI32::new(0);
static OPEN_FLAGS: AtomicI32 = AtomicI32::new(0);
static CLOSE_CALLS: AtomicU32 = AtomicU32::new(0);
static WRITE_PREFIX: AtomicU32 = AtomicU32::new(0);
static WRITE_MODE: AtomicU32 = AtomicU32::new(0);

#[repr(C)]
struct State {
    written: usize,
    bytes: [u8; 6],
}
unsafe extern "C" fn open(
    r: *mut Reent,
    state: *mut c_void,
    _path: *const c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    let error = OPEN_ERROR.load(Ordering::Acquire);
    if error != 0 {
        unsafe { sysbase::set_reent_errno(r, error) };
        return -1;
    }
    OPEN_FLAGS.store(flags, Ordering::Release);
    unsafe {
        state.cast::<State>().write(State {
            written: 0,
            bytes: [0; 6],
        })
    };
    0
}
unsafe extern "C" fn close(r: *mut Reent, _state: *mut c_void) -> i32 {
    CLOSE_CALLS.fetch_add(1, Ordering::AcqRel);
    unsafe { sysbase::set_reent_errno(r, 19) };
    -1
}
unsafe extern "C" fn read(
    r: *mut Reent,
    _state: *mut c_void,
    _output: *mut c_char,
    _length: usize,
) -> isize {
    unsafe { sysbase::set_reent_errno(r, 19) };
    -1
}
unsafe extern "C" fn write(
    r: *mut Reent,
    state: *mut c_void,
    input: *const c_char,
    length: usize,
) -> isize {
    match WRITE_MODE.load(Ordering::Acquire) {
        1 => return (length + 1) as isize, // Deliberately malformed transport result.
        2 => return 0,                     // Deliberate failure to make progress.
        _ => {}
    }
    let state = unsafe { &mut *state.cast::<State>() };
    if state.written != 0 {
        unsafe { sysbase::set_reent_errno(r, 28) };
        return -1;
    }
    let count = length.min(2);
    let bytes = unsafe { core::slice::from_raw_parts(input.cast::<u8>(), count) };
    state.bytes[..count].copy_from_slice(bytes);
    state.written = count;
    WRITE_PREFIX.store(
        u32::from(u16::from_be_bytes([state.bytes[0], state.bytes[1]])),
        Ordering::Release,
    );
    count as isize
}
unsafe extern "C" fn seek(
    r: *mut Reent,
    _state: *mut c_void,
    _position: i64,
    _direction: i32,
) -> i64 {
    unsafe { sysbase::set_reent_errno(r, 29) };
    -1
}
unsafe extern "C" fn sync(r: *mut Reent, _state: *mut c_void) -> i32 {
    unsafe { sysbase::set_reent_errno(r, 30) };
    -1
}
pub fn run(logger: &mut ProbeLogger) {
    logger.section("STD ERRORS AND CONTROLLED CLEANUP FAILURES");
    logger.note("Controlled fault endpoint: checks production error handling, not media.");
    for (code, kind) in [
        (2, ErrorKind::NotFound),
        (13, ErrorKind::PermissionDenied),
        (11, ErrorKind::WouldBlock),
        (12, ErrorKind::OutOfMemory),
        (28, ErrorKind::StorageFull),
        (30, ErrorKind::ReadOnlyFilesystem),
        (77, ErrorKind::InvalidData),
        (111, ErrorKind::ConnectionRefused),
        (116, ErrorKind::TimedOut),
    ] {
        let error = std::io::Error::from_raw_os_error(code);
        logger.record(
            "std classifies errno and retains raw code",
            error.kind() == kind && error.raw_os_error() == Some(code),
        );
    }
    // Devoptab has only integer, raw pointer, and nullable callback fields.
    let device = Box::new(sysbase::Devoptab {
        name: c"std-errors".as_ptr(),
        struct_size: core::mem::size_of::<State>(),
        open_r: Some(open),
        close_r: Some(close),
        read_r: Some(read),
        write_r: Some(write),
        seek_r: Some(seek),
        fsync_r: Some(sync),
        ..unsafe { core::mem::zeroed() }
    });
    let registered = unsafe { sysbase::add_device(&*device) };
    logger.record("controlled fault endpoint registered", registered >= 0);
    if registered < 0 {
        return;
    }
    OPEN_ERROR.store(13, Ordering::Release);
    let baseline = ogc_std_runtime::heap_used_bytes();
    let failed = DeviceFile::open_checked(c"std-errors:/test", sysbase::O_RDONLY);
    let released = ogc_std_runtime::heap_used_bytes() == baseline;
    logger.record(
        "open returns actual errno and releases failed state",
        failed.is_err_and(|e| e == -13) && released,
    );
    OPEN_ERROR.store(0, Ordering::Release);
    match DeviceFile::open_checked(c"std-errors:/test", sysbase::O_WRONLY | sysbase::O_APPEND) {
        Ok(file) => {
            logger.record(
                "open passes exact append flags",
                OPEN_FLAGS.load(Ordering::Acquire) == sysbase::O_WRONLY | sysbase::O_APPEND,
            );
            logger.record(
                "partial write returns final storage error",
                file.write_all(b"abcdef") == Err(-28)
                    && WRITE_PREFIX.load(Ordering::Acquire) == 0x6162,
            );
            logger.record(
                "read preserves device errno",
                file.read(&mut [0; 4]) == Err(-19),
            );
            logger.record(
                "seek preserves not-seekable errno",
                file.seek_off_t(0, sysbase::SEEK_SET) == Err(-29),
            );
            logger.record("sync preserves read-only failure", file.sync() == Err(-30));
            WRITE_MODE.store(1, Ordering::Release);
            logger.record(
                "oversized transport count rejected",
                file.write_all(b"x") == Err(-5),
            );
            WRITE_MODE.store(2, Ordering::Release);
            logger.record(
                "zero-progress write rejected",
                file.write_all(b"x") == Err(-5),
            );
            let before = sysbase::device_close_failures();
            let calls = CLOSE_CALLS.load(Ordering::Acquire);
            logger.record("explicit close returns failure", file.close() == Err(-19));
            logger.record(
                "failed explicit close recorded once without retry",
                sysbase::device_close_failures() == before + 1
                    && sysbase::last_device_close_errno() == 19
                    && CLOSE_CALLS.load(Ordering::Acquire) == calls + 1,
            );
        }
        Err(error) => {
            logger.note_u32("open error", (-error) as u32);
            logger.record("controlled file opens", false);
        }
    }
    match DeviceFile::open_checked(c"std-errors:/test", sysbase::O_RDONLY) {
        Ok(file) => {
            let before = sysbase::device_close_failures();
            let calls = CLOSE_CALLS.load(Ordering::Acquire);
            drop(file);
            logger.record(
                "Drop close failure stays visible",
                sysbase::device_close_failures() == before + 1
                    && sysbase::last_device_close_errno() == 19
                    && CLOSE_CALLS.load(Ordering::Acquire) == calls + 1,
            );
        }
        Err(error) => {
            logger.note_u32("open error", (-error) as u32);
            logger.record("Drop test file opens", false);
        }
    }
    let removed = sysbase::remove_device(c"std-errors");
    logger.record("controlled endpoint removed", removed == 0);
    if removed != 0 {
        // Keep the registered pointer valid after the failed removal.
        core::mem::forget(device);
        logger.note("Endpoint state retained after removal failure");
    }
}
