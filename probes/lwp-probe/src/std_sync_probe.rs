//! Exercise the actual patched std types with production LWP workers.
//! Uses the same synchronization backend as std workers.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;
use ogc_lwp_driver as driver;
use ogc_system_probe::probe_support::ProbeLogger;
use std::sync::{Condvar, Mutex, Once, OnceLock, RwLock};

use crate::futex_probe::spawn;

static PAIR: Mutex<[u32; 2]> = Mutex::new([0; 2]);
static ERRORS: AtomicU32 = AtomicU32::new(0);
static CONDITION: Mutex<bool> = Mutex::new(false);
static NOTIFY: Condvar = Condvar::new();
static RW: RwLock<u32> = RwLock::new(0);
static READ_GATE: AtomicU32 = AtomicU32::new(0);
static READERS: AtomicU32 = AtomicU32::new(0);
static WRITERS: AtomicU32 = AtomicU32::new(0);
static INIT_GATE: AtomicU32 = AtomicU32::new(0);
static ONCE: Once = Once::new();
static ONCE_LOCK: OnceLock<u32> = OnceLock::new();
static INITIALIZATIONS: AtomicU32 = AtomicU32::new(0);
static PUBLICATION: AtomicU32 = AtomicU32::new(0);
static ONCE_COMPLETED: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn mutex_worker(_: *mut c_void) -> *mut c_void {
    for _ in 0..1_000 {
        let Ok(mut value) = PAIR.lock() else {
            return ptr::null_mut();
        };
        if value[0] != value[1] {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
        value[0] += 1;
        driver::yield_thread();
        value[1] += 1;
    }
    0x2000usize as *mut c_void
}

unsafe extern "C" fn condition_worker(_: *mut c_void) -> *mut c_void {
    let Ok(mut condition) = CONDITION.lock() else {
        return ptr::null_mut();
    };
    *condition = true;
    NOTIFY.notify_one();
    0xc0usize as *mut c_void
}

unsafe extern "C" fn reader(_: *mut c_void) -> *mut c_void {
    let Ok(value) = RW.read() else {
        return ptr::null_mut();
    };
    READERS.fetch_add(1, Ordering::Release);
    if !matches!(
        driver::futex_wait_for(&READ_GATE, 0, Some(Duration::from_secs(2))),
        Ok(driver::FutexWait::Woken | driver::FutexWait::ValueChanged)
    ) {
        return ptr::null_mut();
    }
    // The guard must remain live across the wait to exclude the writer.
    if *value == 0 || *value == 0x7257 {
        0x72usize as *mut c_void
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn writer(_: *mut c_void) -> *mut c_void {
    let Ok(mut value) = RW.write() else {
        return ptr::null_mut();
    };
    *value = 0x7257;
    WRITERS.fetch_add(1, Ordering::Release);
    0x57usize as *mut c_void
}

fn initialize_once() -> u32 {
    INITIALIZATIONS.fetch_add(1, Ordering::Relaxed);
    if !matches!(
        driver::futex_wait_for(&INIT_GATE, 0, Some(Duration::from_secs(2))),
        Ok(driver::FutexWait::Woken | driver::FutexWait::ValueChanged)
    ) {
        ERRORS.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    PUBLICATION.store(0x51a7, Ordering::Relaxed);
    0x51a7
}

unsafe extern "C" fn once_worker(argument: *mut c_void) -> *mut c_void {
    let token = argument as usize;
    if token < 3 {
        ONCE.call_once(|| {
            initialize_once();
        });
    } else if *ONCE_LOCK.get_or_init(initialize_once) != 0x51a7 {
        return ptr::null_mut();
    }
    // Relaxed deliberately relies on the Once/OnceLock publication barrier.
    if PUBLICATION.load(Ordering::Relaxed) != 0x51a7 {
        return ptr::null_mut();
    }
    ONCE_COMPLETED.fetch_add(1, Ordering::Release);
    0x51usize as *mut c_void
}

fn mutex_and_condition(logger: &mut ProbeLogger) {
    let mut workers = [None; 2];
    for worker in &mut workers {
        *worker = unsafe { spawn(logger, mutex_worker, 0, 80) };
    }
    for worker in workers.into_iter().flatten() {
        logger.record(
            "std mutex worker joined",
            crate::futex_probe::join(worker) == Ok(0x2000),
        );
    }
    match PAIR.lock() {
        Ok(pair) => logger.record(
            "std mutex preserved both protected counters",
            *pair == [2_000, 2_000] && ERRORS.load(Ordering::Acquire) == 0,
        ),
        Err(_) => logger.record("std mutex remained unpoisoned", false),
    }

    let Ok(condition) = CONDITION.lock() else {
        logger.record("condition mutex acquired", false);
        return;
    };
    let worker = unsafe { spawn(logger, condition_worker, 0, 90) };
    let result = NOTIFY.wait_timeout(condition, Duration::from_secs(2));
    match result {
        Ok((guard, timeout)) => {
            logger.record(
                "std condition releases, notifies, and reacquires",
                *guard && !timeout.timed_out() && CONDITION.try_lock().is_err(),
            );
            drop(guard);
        }
        Err(_) => logger.record("std condition wait remained unpoisoned", false),
    }
    if let Some(worker) = worker {
        logger.record(
            "condition worker joined",
            crate::futex_probe::join(worker) == Ok(0xc0),
        );
    }
    let Ok(guard) = CONDITION.lock() else {
        logger.record("timeout mutex acquired", false);
        return;
    };
    let started = driver::now_ticks();
    match NOTIFY.wait_timeout(guard, Duration::from_millis(25)) {
        Ok((guard, timeout)) => {
            logger.record(
                "std condition timeout reacquires mutex",
                timeout.timed_out()
                    && CONDITION.try_lock().is_err()
                    && driver::now_ticks().wrapping_sub(started) >= 1_518_750,
            );
            drop(guard);
        }
        Err(_) => logger.record("timed wait remained unpoisoned", false),
    }

    // The selected futex backend permits reuse with another mutex. It has no
    // OS handle association that would make this documented optional panic necessary.
    let other = Mutex::new(0u32);
    match other.lock() {
        Ok(guard) => match NOTIFY.wait_timeout(guard, Duration::ZERO) {
            Ok((guard, result)) => {
                logger.record("condition can use a different mutex", result.timed_out());
                drop(guard);
            }
            Err(_) => logger.record("other condition wait remained unpoisoned", false),
        },
        Err(_) => logger.record("other condition mutex remained unpoisoned", false),
    }

    let locks = [const { Mutex::new(0u32) }; 96];
    let mut guards = [const { None }; 96];
    let mut acquired = 0;
    for (lock, output) in locks.iter().zip(&mut guards) {
        match lock.try_lock() {
            Ok(guard) => {
                *output = Some(guard);
                acquired += 1;
            }
            Err(_) => break,
        }
    }
    logger.record(
        "96 std mutexes held without a 64-slot handle pool",
        acquired == 96,
    );
    drop(guards);
}

fn reader_writer(logger: &mut ProbeLogger) {
    let Ok(guard) = RW.read() else {
        logger.record("initial std reader acquired", false);
        return;
    };
    let read_worker = unsafe { spawn(logger, reader, 0, 90) };
    driver::yield_thread();
    logger.record(
        "two std readers coexist and exclude a writer",
        READERS.load(Ordering::Acquire) == 1 && RW.try_write().is_err(),
    );
    let write_worker = unsafe { spawn(logger, writer, 0, 85) };
    driver::yield_thread();
    logger.record("queued writer prevents new readers", RW.try_read().is_err());
    drop(guard);
    driver::yield_thread();
    logger.record(
        "remaining reader still excludes queued writer",
        WRITERS.load(Ordering::Acquire) == 0,
    );
    READ_GATE.store(1, Ordering::Release);
    logger.record(
        "reader gate wakes actual held reader",
        driver::futex_wake(&READ_GATE, true) == 1,
    );
    if let Some(worker) = read_worker {
        logger.record(
            "std reader joined",
            crate::futex_probe::join(worker) == Ok(0x72),
        );
    }
    if let Some(worker) = write_worker {
        logger.record(
            "std writer joined",
            crate::futex_probe::join(worker) == Ok(0x57),
        );
    }
    let Ok(guard) = RW.write() else {
        logger.record("exclusive writer acquired", false);
        return;
    };
    logger.record("writer update is published", *guard == 0x7257);
    let worker = unsafe { spawn(logger, reader, 0, 90) };
    driver::yield_thread();
    logger.record(
        "exclusive std writer blocks reader",
        READERS.load(Ordering::Acquire) == 1,
    );
    drop(guard);
    if let Some(worker) = worker {
        logger.record(
            "reader proceeds after write unlock",
            crate::futex_probe::join(worker) == Ok(0x72) && READERS.load(Ordering::Acquire) == 2,
        );
    }
}

fn once_publication(logger: &mut ProbeLogger) {
    for iteration in 0..2 {
        INIT_GATE.store(0, Ordering::Release);
        PUBLICATION.store(0, Ordering::Relaxed);
        let completed = ONCE_COMPLETED.load(Ordering::Acquire);
        let mut workers = [None; 3];
        for (index, output) in workers.iter_mut().enumerate() {
            *output =
                unsafe { spawn(logger, once_worker, iteration * 3 + index, 90 - index as u8) };
            driver::yield_thread();
        }
        logger.record(
            "once has one initializer and blocked callers",
            INITIALIZATIONS.load(Ordering::Acquire) == (iteration + 1) as u32
                && ONCE_COMPLETED.load(Ordering::Acquire) == completed,
        );
        INIT_GATE.store(1, Ordering::Release);
        logger.record(
            "once initializer gate woke one worker",
            driver::futex_wake(&INIT_GATE, true) == 1,
        );
        for worker in workers.into_iter().flatten() {
            logger.record(
                "once caller observed published value and joined",
                crate::futex_probe::join(worker) == Ok(0x51),
            );
        }
        logger.record(
            "once wakes all callers without repeat initialization",
            ONCE_COMPLETED.load(Ordering::Acquire) == completed + 3
                && INITIALIZATIONS.load(Ordering::Acquire) == (iteration + 1) as u32
                && ERRORS.load(Ordering::Acquire) == 0,
        );
    }
    logger.record("std Once completion remains visible", ONCE.is_completed());
    logger.record(
        "std OnceLock stores the initialized value",
        ONCE_LOCK.get() == Some(&0x51a7),
    );
}

pub(crate) fn run(logger: &mut ProbeLogger) {
    logger.section("ACTUAL WII STD SYNCHRONIZATION TYPES");
    mutex_and_condition(logger);
    reader_writer(logger);
    once_publication(logger);
}
