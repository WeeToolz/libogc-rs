//! Real std thread/park operations and production ABI failure boundaries.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use core::time::Duration;
use ogc_lwp_driver as driver;
use ogc_std_runtime as runtime;
use ogc_system_probe::probe_support::ProbeLogger;
use std::thread;

static JOIN_DROPS: AtomicU32 = AtomicU32::new(0);
static JOIN_DTOR_ERRORS: AtomicU32 = AtomicU32::new(0);
static CAPTURE_DROPS: AtomicU32 = AtomicU32::new(0);
static FAILED_CLOSURE_RUNS: AtomicU32 = AtomicU32::new(0);
static DETACHED_DROPS: AtomicU32 = AtomicU32::new(0);
static PARK_READY: AtomicU32 = AtomicU32::new(0);
static PARK_GATE: AtomicU32 = AtomicU32::new(0);
static PARK_DATA: AtomicU32 = AtomicU32::new(0);
static RAW_KEY: AtomicU32 = AtomicU32::new(0);
static RAW_DTORS: AtomicU32 = AtomicU32::new(0);
static RAW_CLEANUP: AtomicBool = AtomicBool::new(false);
static RAW_CLEANUP_CALLS: AtomicU32 = AtomicU32::new(0);
static RAW_ERRORS: AtomicU32 = AtomicU32::new(0);
static RAW_MARKER: u32 = 0x1357_2468;
static FAILURE_KEY: AtomicU32 = AtomicU32::new(0);
static FAILURE_DTORS: AtomicU32 = AtomicU32::new(0);
static EXPECTED_TLS_FAILURES: AtomicU32 = AtomicU32::new(0);

pub(crate) fn expected_tls_failures() -> u32 {
    EXPECTED_TLS_FAILURES.load(Ordering::Acquire)
}

struct JoinTls;
std::thread_local! { static JOIN_TLS: JoinTls = const { JoinTls }; }

impl Drop for JoinTls {
    fn drop(&mut self) {
        if JOIN_TLS.try_with(|_| ()).is_ok() || thread::current().name() != Some("wii-std-worker") {
            JOIN_DTOR_ERRORS.fetch_add(1, Ordering::AcqRel);
        }
        // Result publication occurs before this delay. Join must still wait
        // for the actual TLS destructor and LWP completion publication.
        thread::sleep(Duration::from_millis(25));
        JOIN_DROPS.fetch_add(1, Ordering::AcqRel);
    }
}

struct DropNotice(&'static AtomicU32);
impl Drop for DropNotice {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

fn wait_count(counter: &AtomicU32, wanted: u32) -> bool {
    let started = driver::now_ticks();
    while counter.load(Ordering::Acquire) < wanted {
        if driver::now_ticks().wrapping_sub(started) >= 2 * driver::TICKS_PER_SECOND {
            return false;
        }
        thread::sleep(Duration::from_millis(1));
    }
    true
}

fn spawn_join_and_scopes(logger: &mut ProbeLogger) {
    logger.section("ACTUAL STD THREAD SPAWN AND JOIN");
    let main_id = thread::current().id();
    let started = driver::now_ticks();
    let child = thread::Builder::new()
        .name("wii-std-worker".into())
        .stack_size(98_305)
        .spawn(|| {
            JOIN_TLS.with(|_| ());
            let current = thread::current();
            let native = driver::current_thread_checked();
            let bounds = native.and_then(|thread| driver::thread_stack_bounds(thread));
            let sp: usize;
            unsafe {
                core::arch::asm!("mr {stack}, 1", stack = out(reg) sp, options(nostack, nomem));
            }
            let valid_stack = bounds.is_ok_and(|bounds| {
                bounds.high - bounds.low >= 98_305 && sp >= bounds.low && sp < bounds.high
            });
            let mut name = [0; driver::THREAD_NAME_CAPACITY];
            let valid_name = native.and_then(|id| driver::thread_name(id, &mut name)) == Ok(14)
                && &name[..14] == b"wii-std-worker";
            thread::yield_now();
            (
                current.id(),
                current.name() == Some("wii-std-worker"),
                valid_name,
                valid_stack,
                [7u32, 11, 13].into_iter().sum::<u32>(),
            )
        });
    match child {
        Ok(child) => {
            let child_id = child.thread().id();
            logger.record(
                "spawned Rust identity differs from main",
                child_id != main_id,
            );
            logger.record(
                "join publishes real value, identity, name, and checked stack",
                child
                    .join()
                    .is_ok_and(|value| value == (child_id, true, true, true, 31)),
            );
            logger.record(
                "join includes completed TLS destruction",
                JOIN_DROPS.load(Ordering::Acquire) == 1
                    && JOIN_DTOR_ERRORS.load(Ordering::Acquire) == 0
                    && driver::now_ticks().wrapping_sub(started) >= 1_518_750,
            );
        }
        Err(error) => {
            match error.raw_os_error() {
                Some(code) => logger.note_i32("std spawn error", code),
                None => logger.note("std spawn error has no OS code"),
            }
            logger.record("named std worker created", false);
        }
    }
    let mut values = [0u32; 4];
    thread::scope(|scope| {
        let (left, right) = values.split_at_mut(2);
        let left_worker = scope.spawn(move || {
            thread::sleep(Duration::from_millis(1));
            left.copy_from_slice(&[11, 22]);
        });
        let right_worker = scope.spawn(move || {
            thread::yield_now();
            right.copy_from_slice(&[33, 44]);
        });
        drop((left_worker, right_worker));
    });
    logger.record(
        "scoped workers complete borrowed writes before scope returns",
        values == [11, 22, 33, 44],
    );
    logger.record(
        "hardware parallelism is one on supported Wii",
        thread::available_parallelism().is_ok_and(|count| count.get() == 1),
    );
}

fn expect_creation_failure(logger: &mut ProbeLogger, stack: usize, errno: i32) {
    let before = CAPTURE_DROPS.load(Ordering::Acquire);
    let capture = DropNotice(&CAPTURE_DROPS);
    let result = thread::Builder::new().stack_size(stack).spawn(move || {
        FAILED_CLOSURE_RUNS.fetch_add(1, Ordering::AcqRel);
        drop(capture);
        0x51u32
    });
    match result {
        Err(error) => logger.record(
            "spawn returned the real stack/allocator failure",
            error.raw_os_error() == Some(errno),
        ),
        Ok(worker) => {
            logger.record("failing stack request was rejected", false);
            logger.record(
                "unexpected successful worker was cleaned up",
                worker.join().is_ok_and(|value| value == 0x51),
            );
        }
    }
    logger.record(
        "failed spawn drops its captured value exactly once",
        CAPTURE_DROPS.load(Ordering::Acquire) == before + 1
            && FAILED_CLOSURE_RUNS.load(Ordering::Acquire) == 0,
    );
}

fn creation_failure_and_detach(logger: &mut ProbeLogger) {
    logger.section("STD THREAD FAILURE AND DETACH CLEANUP");
    expect_creation_failure(logger, usize::MAX, ogc_sysbase::EINVAL);
    // A 128 MiB stack exceeds total Wii RAM. This valid arithmetic request
    // reaches the real workspace allocator and must fail without publication.
    expect_creation_failure(logger, 128 * 1024 * 1024, ogc_sysbase::ENOMEM);
    match thread::Builder::new().stack_size(1).spawn(|| 0x7257u32) {
        Ok(worker) => logger.record(
            "minimum stack adjustment works after allocation failure",
            worker.join().is_ok_and(|value| value == 0x7257),
        ),
        Err(_) => logger.record("minimum stack worker created", false),
    }

    let before = driver::thread_cleanup_counters();
    let drops = DETACHED_DROPS.load(Ordering::Acquire);
    let mut completed = 0;
    for index in 0..64u32 {
        let result = thread::Builder::new().stack_size(40 * 1024).spawn(|| {
            thread::sleep(Duration::from_millis(1));
            DropNotice(&DETACHED_DROPS)
        });
        let worker = match result {
            Ok(worker) => worker,
            Err(_) => {
                logger.record("detached std worker created", false);
                break;
            }
        };
        let retained_handle = worker.thread().clone();
        drop(worker);
        let started = driver::now_ticks();
        while driver::thread_cleanup_counters()
            .detached_reaped
            .wrapping_sub(before.detached_reaped)
            <= index
        {
            if driver::now_ticks().wrapping_sub(started) >= 2 * driver::TICKS_PER_SECOND {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        if driver::thread_cleanup_counters()
            .detached_reaped
            .wrapping_sub(before.detached_reaped)
            <= index
        {
            logger.record("detached worker stack release completed", false);
            break;
        }
        // The public Thread handle owns its parker independently of the native
        // join handle and remains safe after the worker and stack are gone.
        retained_handle.unpark();
        drop(retained_handle);
        completed += 1;
    }
    logger.record(
        "64 detached std workers released their dynamic stacks",
        completed == 64,
    );
    logger.record(
        "detached results were dropped after actual worker completion",
        DETACHED_DROPS.load(Ordering::Acquire) == drops + 64,
    );
    logger.record(
        "detached cleanup did not hide a TLS failure",
        driver::thread_cleanup_counters().tls_failures == before.tls_failures,
    );
}

fn parking(logger: &mut ProbeLogger) {
    logger.section("ACTUAL STD PARK AND UNPARK");
    let child = thread::Builder::new().spawn(|| {
        let current = thread::current();
        current.unpark();
        current.unpark();
        let before = driver::debug_counters().blocking_waits;
        thread::park();
        let consumed_one_token = driver::debug_counters().blocking_waits == before;
        let started = driver::now_ticks();
        thread::park_timeout(Duration::from_millis(25));
        // No thread can unpark this worker here. The second unpark must have
        // coalesced with the first, rather than creating a second token.
        (
            consumed_one_token,
            driver::now_ticks().wrapping_sub(started) >= 1_518_750,
        )
    });
    match child {
        Ok(worker) => logger.record(
            "unpark-before-park retains exactly one coalesced token",
            worker.join().is_ok_and(|value| value == (true, true)),
        ),
        Err(_) => logger.record("token probe worker created", false),
    }

    let child = thread::Builder::new().spawn(|| {
        PARK_READY.store(1, Ordering::Release);
        let started = driver::now_ticks();
        loop {
            thread::park_timeout(Duration::from_millis(25));
            if PARK_GATE.load(Ordering::Relaxed) != 0 {
                return PARK_DATA.load(Ordering::Relaxed);
            }
            if driver::now_ticks().wrapping_sub(started) >= 2 * driver::TICKS_PER_SECOND {
                return 0;
            }
        }
    });
    match child {
        Ok(worker) => {
            logger.record(
                "park worker reached wait protocol",
                wait_count(&PARK_READY, 1),
            );
            PARK_DATA.store(0x1357_2468, Ordering::Relaxed);
            PARK_GATE.store(1, Ordering::Relaxed);
            worker.thread().unpark();
            logger.record(
                "cross-thread unpark publishes data to the waking worker",
                worker.join().is_ok_and(|value| value == 0x1357_2468),
            );
        }
        Err(_) => logger.record("park wake worker created", false),
    }

    // Race a timeout and unpark, then demand progress for every round. The
    // token can be consumed by the timed park or retained for the next park.
    PARK_READY.store(0, Ordering::Release);
    PARK_GATE.store(0, Ordering::Release);
    let child = thread::Builder::new().spawn(|| {
        for round in 1..=32 {
            PARK_READY.store(round, Ordering::Release);
            thread::park_timeout(Duration::from_nanos(1));
            let started = driver::now_ticks();
            while PARK_GATE.load(Ordering::Acquire) < round {
                thread::park_timeout(Duration::from_millis(1));
                if driver::now_ticks().wrapping_sub(started) >= 2 * driver::TICKS_PER_SECOND {
                    return 0;
                }
            }
        }
        32u32
    });
    match child {
        Ok(worker) => {
            let mut rounds = 0;
            for round in 1..=32 {
                if !wait_count(&PARK_READY, round) {
                    break;
                }
                PARK_GATE.store(round, Ordering::Release);
                worker.thread().unpark();
                rounds += 1;
            }
            let joined = worker.join();
            logger.record(
                "timeout/unpark races retain progress in all rounds",
                rounds == 32 && joined.is_ok_and(|value| value == 32),
            );
        }
        Err(_) => logger.record("park timeout race worker created", false),
    }
}

unsafe extern "C" fn raw_destructor(value: *mut c_void) {
    let pass = RAW_DTORS.fetch_add(1, Ordering::AcqRel);
    if value != ptr::addr_of!(RAW_MARKER).cast_mut().cast() {
        RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
    }
    if pass == 0 && unsafe { driver::tls_set(RAW_KEY.load(Ordering::Acquire), value) }.is_err() {
        RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
    }
}

unsafe extern "C" fn raw_cleanup() {
    RAW_CLEANUP_CALLS.fetch_add(1, Ordering::AcqRel);
    if RAW_DTORS.load(Ordering::Acquire) != 2
        || driver::tls_get(RAW_KEY.load(Ordering::Acquire)) != Ok(ptr::null_mut())
    {
        RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
    }
    RAW_CLEANUP.store(true, Ordering::Release);
}

unsafe extern "C" fn conflicting_cleanup() {
    RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn repeat_destructor(value: *mut c_void) {
    FAILURE_DTORS.fetch_add(1, Ordering::AcqRel);
    // This deliberate static-value reinstatement exercises the real LWP limit.
    // There is no heap allocation to leak when the limit is reported.
    if unsafe { driver::tls_set(FAILURE_KEY.load(Ordering::Acquire), value) }.is_err() {
        RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
    }
}

unsafe extern "C" fn cleanup_failure_worker(value: *mut c_void) -> *mut c_void {
    if unsafe { driver::tls_set(FAILURE_KEY.load(Ordering::Acquire), value) }.is_ok() {
        ptr::null_mut()
    } else {
        1usize as *mut c_void
    }
}

fn cleanup_failure(logger: &mut ProbeLogger) {
    let before = driver::thread_cleanup_counters().tls_failures;
    let key = match unsafe { driver::tls_create(Some(repeat_destructor)) } {
        Ok(key) => key,
        Err(_) => {
            logger.record("cleanup failure key created", false);
            return;
        }
    };
    FAILURE_KEY.store(key, Ordering::Release);
    let mut worker = driver::LWP_THREAD_NULL;
    let created = unsafe {
        runtime::__wii_std_thread_create(
            Some(cleanup_failure_worker),
            ptr::addr_of!(RAW_MARKER).cast_mut().cast(),
            65_536,
            &mut worker,
        )
    };
    logger.record("cleanup failure worker created", created == 0);
    if created == 0 {
        let joined = unsafe { runtime::__wii_std_thread_join(worker) };
        let actual = driver::thread_cleanup_counters()
            .tls_failures
            .wrapping_sub(before);
        logger.record(
            "destructor limit returns a real join error",
            joined == ogc_sysbase::EIO && actual == 1 && FAILURE_DTORS.load(Ordering::Acquire) == 4,
        );
        if actual == 1 {
            EXPECTED_TLS_FAILURES.fetch_add(1, Ordering::AcqRel);
        }
        let mut name = [0; driver::THREAD_NAME_CAPACITY];
        logger.record(
            "failed cleanup still releases the joined native slot",
            driver::thread_name(worker, &mut name) == Err(driver::ThreadError::InvalidThread),
        );
    }
    logger.record(
        "cleanup failure key released",
        driver::tls_destroy(key).is_ok(),
    );
}

unsafe extern "C" fn raw_worker(data: *mut c_void) -> *mut c_void {
    let mut current = 0;
    let name = b"raw-std-worker";
    let mut native_name = [0; driver::THREAD_NAME_CAPACITY];
    let valid = unsafe { data.cast::<u32>().read() } == 0x1357_2468
        && unsafe { runtime::__wii_std_thread_current(&mut current) } == 0
        && driver::current_thread_checked() == Ok(current)
        && driver::set_current_thread_name("before-std-name").is_ok()
        && unsafe { runtime::__wii_std_thread_set_name(name.as_ptr(), name.len()) } == 0
        && driver::thread_name(current, &mut native_name) == Ok(14)
        && &native_name[..14] == name
        && unsafe { driver::register_thread_cleanup(raw_cleanup) }.is_ok()
        && unsafe { runtime::__wii_std_thread_cleanup_register(Some(raw_cleanup)) } == 0
        && unsafe { runtime::__wii_std_thread_cleanup_register(Some(conflicting_cleanup)) }
            == ogc_sysbase::EBUSY
        && unsafe { driver::tls_set(RAW_KEY.load(Ordering::Acquire), data) }.is_ok()
        && runtime::__wii_std_thread_sleep(0, 1_000_000) == 0
        && driver::sleep_for(Duration::from_nanos(1)).is_ok()
        && runtime::__wii_std_thread_yield() == 0;
    // All user TLS access has ended. The automatic exit path must preserve
    // this completed result and must not run destructors or callbacks twice.
    let cleaned = unsafe { driver::cleanup_current_thread() }.is_ok();
    if valid && cleaned {
        ptr::null_mut()
    } else {
        RAW_ERRORS.fetch_add(1, Ordering::AcqRel);
        1usize as *mut c_void
    }
}

fn raw_provider(logger: &mut ProbeLogger) {
    logger.section("STD THREAD ABI VALIDATION AND CLEANUP ORDER");
    let mut output = 0x2468_1357;
    let marker = ptr::addr_of!(RAW_MARKER).cast_mut().cast();
    logger.record(
        "invalid callback rejects without publishing a handle",
        unsafe { runtime::__wii_std_thread_create(None, marker, 65_536, &mut output) }
            == ogc_sysbase::EINVAL
            && output == 0x2468_1357,
    );
    logger.record(
        "null handle output rejects before creation",
        unsafe {
            runtime::__wii_std_thread_create(Some(raw_worker), marker, 65_536, ptr::null_mut())
        } == ogc_sysbase::EINVAL,
    );
    logger.record(
        "driver validates a missing entry",
        unsafe { driver::spawn_thread(None, marker, 65_536) }
            == Err(driver::ThreadError::MissingEntry),
    );
    logger.record(
        "invalid native join is an error",
        unsafe { driver::join_thread_checked(driver::LWP_THREAD_NULL) }
            == Err(driver::ThreadError::InvalidThread)
            && unsafe { runtime::__wii_std_thread_join(driver::LWP_THREAD_NULL) }
                == ogc_sysbase::EINVAL,
    );
    logger.record(
        "invalid detach is an error",
        unsafe { runtime::__wii_std_thread_detach(driver::LWP_THREAD_NULL) } == ogc_sysbase::EINVAL
            && unsafe { driver::detach_thread_checked(driver::LWP_THREAD_NULL) }
                == Err(driver::ThreadError::InvalidThread),
    );
    logger.record(
        "null cleanup callback is rejected",
        unsafe { runtime::__wii_std_thread_cleanup_register(None) } == ogc_sysbase::EINVAL,
    );
    logger.record(
        "invalid duration is rejected",
        runtime::__wii_std_thread_sleep(0, 1_000_000_000) == ogc_sysbase::EINVAL,
    );
    let irq = driver::critical_enter();
    let masked = runtime::__wii_std_thread_yield();
    driver::critical_exit(irq);
    logger.record(
        "masked thread operation fails before scheduling",
        masked == ogc_sysbase::EBUSY,
    );
    let mut count = 0;
    logger.record(
        "parallelism reads a supported initialized Wii",
        unsafe { runtime::__wii_std_thread_parallelism(&mut count) } == 0 && count == 1,
    );
    let key = match unsafe { driver::tls_create(Some(raw_destructor)) } {
        Ok(key) => key,
        Err(_) => {
            logger.record("cleanup-order TLS key created", false);
            return;
        }
    };
    RAW_KEY.store(key, Ordering::Release);
    let created =
        unsafe { runtime::__wii_std_thread_create(Some(raw_worker), marker, 65_537, &mut output) };
    logger.record(
        "raw std worker published only after real creation",
        created == 0 && output != 0x2468_1357,
    );
    if created == 0 {
        logger.record(
            "compatibility handles cannot consume std ownership",
            unsafe { driver::join_thread(output, ptr::null_mut()) } == driver::LWP_CLOSED
                && driver::detach_thread(output) == driver::LWP_CLOSED,
        );
        logger.record(
            "raw std join includes real callback completion",
            unsafe { runtime::__wii_std_thread_join(output) } == 0
                && RAW_CLEANUP.load(Ordering::Acquire)
                && RAW_CLEANUP_CALLS.load(Ordering::Acquire) == 1
                && RAW_DTORS.load(Ordering::Acquire) == 2
                && RAW_ERRORS.load(Ordering::Acquire) == 0,
        );
    }
    logger.record(
        "cleanup-order TLS key released",
        driver::tls_destroy(key).is_ok(),
    );
}

pub(crate) fn run(logger: &mut ProbeLogger) {
    raw_provider(logger);
    cleanup_failure(logger);
    spawn_join_and_scopes(logger);
    creation_failure_and_detach(logger);
    parking(logger);
    logger.record(
        "all raw TLS and callback operations returned their real result",
        RAW_ERRORS.load(Ordering::Acquire) == 0,
    );
}
