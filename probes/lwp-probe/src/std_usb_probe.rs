//! Real IOS USB opens and closes through the production deferred IPC worker.
use core::ffi::{CStr, c_void};
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use ogc_system_probe::probe_support::ProbeLogger;
use std::time::{Duration, Instant};

static COMPLETED: AtomicU32 = AtomicU32::new(0);
static RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
static CALLBACK_THREAD: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn completed(result: i32, _: *mut c_void) -> i32 {
    CALLBACK_THREAD.store(ogc_lwp_driver::current_thread(), Ordering::Relaxed);
    RESULT.store(result, Ordering::Relaxed);
    COMPLETED.fetch_add(1, Ordering::Release);
    0
}

fn wait(logger: &mut ProbeLogger, previous: u32) -> Option<i32> {
    let start = Instant::now();
    while COMPLETED.load(Ordering::Acquire) == previous && start.elapsed() < Duration::from_secs(3)
    {
        ogc_ipc::poll();
        std::thread::yield_now();
    }
    let done = COMPLETED.load(Ordering::Acquire) == previous + 1;
    logger.record("one final IOS callback completed", done);
    logger.record(
        "callback ran on the deferred worker",
        done && CALLBACK_THREAD.load(Ordering::Acquire) != ogc_lwp_driver::current_thread(),
    );
    done.then(|| RESULT.load(Ordering::Acquire))
}

fn open_and_close(logger: &mut ProbeLogger, path: &CStr, exists: bool) -> bool {
    let previous = COMPLETED.load(Ordering::Acquire);
    let submitted = ogc_ipc::open_file_async(
        path,
        ogc_ipc::IPC_OPEN_NONE,
        Some(completed),
        core::ptr::null_mut(),
    );
    logger.note_i32("open submission", submitted);
    logger.record("USB open request accepted", submitted == 0);
    if submitted != 0 {
        return false;
    }
    let Some(result) = wait(logger, previous) else {
        return false;
    };
    logger.note_i32("final IOS open result", result);
    logger.record(
        "final open result matches real device path",
        if exists {
            result >= 0
        } else {
            result == ogc_ipc::IPC_ENOENT
        },
    );
    if result < 0 {
        return !exists && result == ogc_ipc::IPC_ENOENT;
    }
    let previous = COMPLETED.load(Ordering::Acquire);
    // SAFETY: The callback state is static. This request has no borrowed buffer.
    let submitted = unsafe {
        ogc_ipc::FileDescriptor::new(result).close_async(Some(completed), core::ptr::null_mut())
    };
    logger.record("USB close request accepted", submitted == 0);
    if submitted != 0 {
        logger.record(
            "failed close submission cleaned up descriptor",
            ogc_ipc::FileDescriptor::new(result).close().is_ok(),
        );
        return false;
    }
    let Some(closed) = wait(logger, previous) else {
        return false;
    };
    logger.record("final USB close succeeded", closed == 0);
    closed == 0 && exists
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB IOS AND DEFERRED WORKER ACCEPTANCE");
    logger.note("No USB storage writes. Controller reports need a separate device run.");
    // USB host managers retain exclusive IOS descriptors. Release those from
    // earlier suites before the raw asynchronous open/close acceptance run.
    let cleanup = ogc_usb::try_deinitialize();
    logger.record_with_detail(
        "USB host released before raw IOS opens",
        cleanup.is_ok(),
        &format!("{cleanup:?}"),
    );
    if cleanup.is_err() {
        return;
    }
    let completed_before = COMPLETED.load(Ordering::Acquire);
    let before = ogc_lwp_driver::debug_counters();
    for _ in 0..8 {
        if !open_and_close(logger, c"/dev/usb/hid", true) {
            break;
        }
    }
    let missing = open_and_close(logger, c"/dev/usb/std-probe-missing", false);
    logger.record("missing USB path reports final IOS error", missing);
    let after = ogc_lwp_driver::debug_counters();
    logger.record(
        "all 17 requests completed on production worker",
        COMPLETED
            .load(Ordering::Acquire)
            .wrapping_sub(completed_before)
            == 17
            && after.deferred_callbacks >= before.deferred_callbacks + 17,
    );
    logger.record(
        "no deferred completion was dropped",
        after.dropped_deferred_callbacks == before.dropped_deferred_callbacks,
    );
    logger.record(
        "production USB host initialization succeeds",
        ogc_usb::try_initialize().is_ok(),
    );
    let mut entries = [ogc_usb::UsbDeviceEntry::default(); 32];
    match ogc_usb::device_list(&mut entries, 3) {
        Ok(count) => {
            logger.note_u32("actual HID device count", count as u32);
            logger.record("real HID list is bounded", count <= entries.len());
        }
        Err(error) => {
            logger.note_i32("HID list error", error.raw());
            logger.record("real HID list is available", false);
        }
    }
    logger.record(
        "production USB cleanup succeeds",
        ogc_usb::try_deinitialize().is_ok(),
    );
}
