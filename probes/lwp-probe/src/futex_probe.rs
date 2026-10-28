//! Real LWP/provider tests. Build evidence does not mean hardware acceptance.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use ogc_lwp_driver as driver;
use ogc_std_runtime::{__wii_std_futex_wait, __wii_std_futex_wake};
use ogc_system_probe::probe_support::ProbeLogger;

static WORDS: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
static COMPLETED: AtomicU32 = AtomicU32::new(0);
static IRQ_RESULT: AtomicI32 = AtomicI32::new(i32::MAX);
static IRQ_WOKEN: AtomicI32 = AtomicI32::new(-1);

/// # Safety
/// entry must accept the integer token as an opaque pointer and keep every
/// accessed object live through join. The worker gets a real dynamic stack.
pub(crate) unsafe fn spawn(
    logger: &mut ProbeLogger,
    entry: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    token: usize,
    priority: u8,
) -> Option<driver::LwpThread> {
    let mut thread = driver::LWP_THREAD_NULL;
    // SAFETY: The caller supplies the callback contract. LWP owns the stack.
    let result = unsafe {
        driver::create_thread(
            &mut thread,
            Some(entry),
            token as *mut c_void,
            ptr::null_mut(),
            40 * 1024,
            priority,
        )
    };
    logger.record("synchronization worker created", result == 0);
    if result == 0 { Some(thread) } else { None }
}

unsafe extern "C" fn worker(argument: *mut c_void) -> *mut c_void {
    let token = argument as usize;
    let word = if token == 3 {
        &WORDS[1]
    } else if token == 4 {
        &WORDS[2]
    } else {
        &WORDS[0]
    };
    // SAFETY: Every word is static. The finite timeout permits probe cleanup
    // even when a wake fails. Completion is checked from the actual wait.
    if unsafe { __wii_std_futex_wait(word, 0, 2, 0, true) } == 0 {
        COMPLETED.fetch_or(1 << token, Ordering::AcqRel);
        (0x10 + token) as *mut c_void
    } else {
        ptr::null_mut()
    }
}

unsafe fn interrupt_wake(_: *mut c_void) {
    // SAFETY: This callback accesses only static atomic words. A wait in IRQ
    // context must fail; waking from this real decrementer callback is valid.
    IRQ_RESULT.store(
        unsafe { __wii_std_futex_wait(&WORDS[2], 0, 0, 1, true) },
        Ordering::Release,
    );
    IRQ_WOKEN.store(
        unsafe { __wii_std_futex_wake(&WORDS[2], true) },
        Ordering::Release,
    );
}

pub(crate) fn run(logger: &mut ProbeLogger) {
    logger.section("WII STD FUTEX PROVIDER");
    logger.record(
        "null futex wait rejected",
        unsafe { __wii_std_futex_wait(ptr::null(), 0, 0, 0, false) } == -ogc_sysbase::EINVAL,
    );
    logger.record(
        "null futex wake rejected",
        unsafe { __wii_std_futex_wake(ptr::null(), false) } == -ogc_sysbase::EINVAL,
    );
    let misaligned = (ptr::addr_of!(WORDS[0]) as *const u8)
        .wrapping_add(1)
        .cast();
    logger.record(
        "misaligned word rejected",
        unsafe { __wii_std_futex_wait(misaligned, 0, 0, 0, false) } == -ogc_sysbase::EINVAL,
    );
    logger.record(
        "invalid nanoseconds rejected",
        unsafe { __wii_std_futex_wait(&WORDS[0], 0, 0, 1_000_000_000, true) }
            == -ogc_sysbase::EINVAL,
    );
    let irq = driver::critical_enter();
    let masked = unsafe { __wii_std_futex_wait(&WORDS[0], 0, 0, 0, false) };
    driver::critical_exit(irq);
    logger.record(
        "wait with interrupts disabled rejected",
        masked == -ogc_sysbase::EBUSY,
    );

    WORDS[0].store(7, Ordering::Release);
    logger.record(
        "LWP mismatch returns without a wake",
        driver::futex_wait(&WORDS[0], 0, None) == Ok(driver::FutexWait::ValueChanged),
    );
    let early_wake = unsafe { __wii_std_futex_wake(&WORDS[0], true) };
    let before = driver::debug_counters().blocking_waits;
    let mismatch = unsafe { __wii_std_futex_wait(&WORDS[0], 0, 0, 0, false) };
    let after = driver::debug_counters().blocking_waits;
    // Measure before logging. Each logger record waits for a real VI retrace,
    // which can increase the scheduler's blocking-wait counter.
    logger.record(
        "wake before registration reports no waiter",
        early_wake == 0,
    );
    logger.record("changed value prevents lost-wake sleep", mismatch == 0);
    logger.record("mismatch did not enter scheduler wait", after == before);
    WORDS[0].store(0, Ordering::Release);
    logger.record(
        "LWP duration timeout is explicit",
        driver::futex_wait_for(&WORDS[0], 0, Some(core::time::Duration::ZERO))
            == Ok(driver::FutexWait::TimedOut),
    );
    logger.record(
        "LWP wake without a waiter returns zero",
        driver::futex_wake(&WORDS[0], true) == 0,
    );
    logger.record(
        "zero timeout reports timeout",
        unsafe { __wii_std_futex_wait(&WORDS[0], 0, 0, 0, true) } == 1,
    );
    let start = driver::now_ticks();
    let result = unsafe { __wii_std_futex_wait(&WORDS[0], 0, 0, 25_000_000, true) };
    let elapsed = driver::now_ticks().wrapping_sub(start);
    logger.record(
        "finite wait reports real timeout",
        result == 1 && elapsed >= 1_518_750,
    );
    logger.record(
        "timed-out waiter was removed",
        unsafe { __wii_std_futex_wake(&WORDS[0], true) } == 0,
    );
    logger.record(
        "invalid tick span rejected",
        driver::futex_wait(&WORDS[0], 0, Some(u64::MAX)) == Err(driver::FutexError::InvalidTimeout),
    );

    COMPLETED.store(0, Ordering::Release);
    let mut workers = [None; 3];
    for (slot, (token, priority)) in [(1, 80), (2, 90), (3, 85)].into_iter().enumerate() {
        workers[slot] = unsafe { spawn(logger, worker, token, priority) };
        driver::yield_thread();
    }
    logger.record(
        "workers blocked before wake",
        COMPLETED.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "wake one returned the actual count",
        unsafe { __wii_std_futex_wake(&WORDS[0], false) } == 1,
    );
    driver::yield_thread();
    logger.record(
        "highest priority waiter woke alone",
        COMPLETED.load(Ordering::Acquire) == 1 << 2,
    );
    logger.record(
        "broadcast found the remaining address waiter",
        unsafe { __wii_std_futex_wake(&WORDS[0], true) } == 1,
    );
    logger.record(
        "other address stayed asleep",
        COMPLETED.load(Ordering::Acquire) & (1 << 3) == 0,
    );
    logger.record(
        "other address wakes separately",
        unsafe { __wii_std_futex_wake(&WORDS[1], true) } == 1,
    );
    for (index, worker) in workers.into_iter().enumerate() {
        if let Some(worker) = worker {
            logger.record(
                "woken worker joined and released stack",
                join(worker) == Ok(0x11 + index),
            );
        }
    }
    logger.record(
        "no stale waiter after joins",
        unsafe { __wii_std_futex_wake(&WORDS[0], true) } == 0,
    );

    if let Some(worker) = unsafe { spawn(logger, worker, 4, 90) } {
        driver::yield_thread();
        let key = 0x5354_4446;
        let started = unsafe {
            driver::timer_start(
                key,
                Some(interrupt_wake),
                ptr::null_mut(),
                driver::TICKS_PER_SECOND / 40,
                0,
            )
        };
        logger.record("real IRQ wake timer started", started == 0);
        logger.record("IRQ-woken worker joined", join(worker) == Ok(0x14));
        driver::timer_cancel(key);
        logger.record(
            "wait in IRQ context rejected",
            IRQ_RESULT.load(Ordering::Acquire) == -ogc_sysbase::EACCES,
        );
        logger.record(
            "IRQ wake returned the real waiter count",
            IRQ_WOKEN.load(Ordering::Acquire) == 1,
        );
    }
}

/// Join a production worker and return its final value.
pub(crate) fn join(thread: driver::LwpThread) -> Result<usize, i32> {
    let mut value = ptr::null_mut();
    // SAFETY: The output remains valid until the synchronous join completes.
    let result = unsafe { driver::join_thread(thread, &mut value) };
    if result == 0 {
        Ok(value as usize)
    } else {
        Err(result)
    }
}
