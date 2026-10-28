//! Actual std clocks, production clock ABI, RTC, and timed wait acceptance.
use core::sync::atomic::{AtomicBool, Ordering};
use ogc_std_runtime as runtime;
use ogc_system::{self as system, ClockError};
use ogc_system_probe::probe_support::ProbeLogger;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn ticks_nanos(ticks: u64) -> u128 {
    // Independent probe calculation; do not call the production converter.
    u128::from(ticks) * 1_000_000_000 / 60_750_000
}

fn arguments(logger: &mut ProbeLogger) -> Option<i32> {
    match crate::std_probe_config::value("utc-offset-seconds") {
        Ok(None) => None,
        Ok(Some(value)) => match value.parse::<i32>() {
            Ok(offset) if (-86_399..=86_399).contains(&offset) => {
                logger.record("UTC offset argument is valid and is not repeated", true);
                Some(offset)
            }
            _ => {
                logger.record("UTC offset argument is valid and is not repeated", false);
                None
            }
        },
        Err(error) => {
            logger.note(error);
            logger.record("UTC offset argument is valid and is not repeated", false);
            None
        }
    }
}

fn arithmetic(logger: &mut ProbeLogger) {
    logger.section("STD CLOCK ARITHMETIC");
    let now = Instant::now();
    let later = now.checked_add(Duration::from_nanos(123));
    logger.record(
        "Instant checked arithmetic keeps nanoseconds",
        later.is_some_and(|later| {
            later.checked_duration_since(now) == Some(Duration::from_nanos(123))
        }),
    );
    logger.record(
        "Instant rejects addition overflow",
        now.checked_add(Duration::MAX).is_none(),
    );
    let earlier = UNIX_EPOCH.checked_sub(Duration::from_nanos(1));
    logger.record(
        "SystemTime supports before-epoch arithmetic",
        earlier.is_some_and(|earlier| {
            earlier
                .duration_since(UNIX_EPOCH)
                .is_err_and(|error| error.duration() == Duration::from_nanos(1))
                && UNIX_EPOCH
                    .duration_since(earlier)
                    .is_ok_and(|value| value == Duration::from_nanos(1))
        }),
    );
    logger.record(
        "SystemTime rejects addition overflow",
        UNIX_EPOCH.checked_add(Duration::MAX).is_none(),
    );
}

fn monotonic(logger: &mut ProbeLogger, early: Instant, raw_early: u64) {
    logger.section("STABLE MONOTONIC CLOCK");
    let snapshot = system::clock_snapshot();
    logger.record(
        "system initialization established a valid origin",
        snapshot.monotonic_initialized && snapshot.monotonic_error.is_none(),
    );
    let after = Instant::now();
    let raw_after = ogc_lwp_driver::now_ticks();
    let span = after.checked_duration_since(early);
    logger.record(
        "pre-main did not reset Instant or the hardware timebase",
        raw_after >= raw_early
            && span.is_some_and(|span| {
                span.as_nanos().abs_diff(ticks_nanos(raw_after - raw_early)) < 10_000_000
            }),
    );
    let mut previous = Instant::now();
    let mut ordered = true;
    for _ in 0..1024 {
        let next = Instant::now();
        ordered &= next >= previous;
        previous = next;
    }
    logger.record("repeated real Instant samples are ordered", ordered);
    logger.record(
        "fallible monotonic facade returns a real sample",
        system::monotonic_time().is_ok(),
    );
    logger.record(
        "fallible system ABI returns a real sample",
        system::sys::sys_monotonic_time().is_ok(),
    );
    let mut seconds = u64::MAX;
    let mut nanos = u32::MAX;
    logger.record(
        "std clock ABI returns normalized monotonic time",
        unsafe { runtime::__wii_std_clock_now(false, &mut seconds, &mut nanos) } == 0
            && nanos < 1_000_000_000,
    );
    let saved_nanos = nanos;
    logger.record(
        "invalid clock output is rejected without changing the other output",
        unsafe { runtime::__wii_std_clock_now(false, core::ptr::null_mut(), &mut nanos) }
            == ogc_sysbase::EINVAL
            && nanos == saved_nanos,
    );
    let mut overlap = 0x1357_2468_9753_8642u64;
    let overlap_pointer = &mut overlap as *mut u64;
    logger.record(
        "overlapping clock outputs are rejected without a write",
        unsafe { runtime::__wii_std_clock_now(false, overlap_pointer, overlap_pointer.cast()) }
            == ogc_sysbase::EINVAL
            && overlap == 0x1357_2468_9753_8642,
    );
    let started = Instant::now();
    let raw_started = ogc_lwp_driver::now_ticks();
    thread::sleep(Duration::from_millis(25));
    let elapsed = started.elapsed();
    let raw_elapsed = ogc_lwp_driver::now_ticks().wrapping_sub(raw_started);
    logger.record(
        "sleep meets its lower bound on the independent raw counter",
        raw_elapsed >= 1_518_750 && raw_elapsed < 121_500_000,
    );
    logger.record(
        "Instant elapsed agrees with raw-counter elapsed",
        elapsed.as_nanos().abs_diff(ticks_nanos(raw_elapsed)) < 1_000_000,
    );
}

fn deadlines(logger: &mut ProbeLogger) {
    logger.section("STD ABSOLUTE DEADLINES AND SPURIOUS NOTIFICATIONS");
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let ready = Arc::new(AtomicBool::new(false));
    let child_pair = pair.clone();
    let child_ready = ready.clone();
    let worker = match thread::Builder::new().spawn(move || {
        while !child_ready.load(Ordering::Acquire) {
            thread::yield_now();
        }
        for _ in 0..12 {
            thread::sleep(Duration::from_millis(5));
            child_pair.1.notify_one();
        }
        12
    }) {
        Ok(worker) => worker,
        Err(_) => {
            logger.record("notification worker created", false);
            return;
        }
    };
    let guard = pair.0.lock().unwrap();
    let start = Instant::now();
    let raw_start = ogc_lwp_driver::now_ticks();
    ready.store(true, Ordering::Release);
    let waited = pair
        .1
        .wait_timeout_while(guard, Duration::from_millis(20), |value| !*value);
    let elapsed = start.elapsed();
    let raw_elapsed = ogc_lwp_driver::now_ticks().wrapping_sub(raw_start);
    logger.record(
        "notifications do not restart the original condition deadline",
        waited.is_ok_and(|(guard, result)| !*guard && result.timed_out())
            && elapsed >= Duration::from_millis(20)
            && elapsed < Duration::from_millis(40)
            && raw_elapsed >= 1_215_000
            && raw_elapsed < 2_430_000,
    );
    logger.record(
        "notification worker completed and joined",
        worker.join().is_ok_and(|value| value == 12),
    );
    let raw_start = ogc_lwp_driver::now_ticks();
    thread::park_timeout(Duration::ZERO);
    logger.record(
        "zero park timeout does not become an infinite wait",
        ogc_lwp_driver::now_ticks().wrapping_sub(raw_start) < 60_750_000,
    );
}

fn wall(logger: &mut ProbeLogger, offset: Option<i32>) {
    logger.section("REAL RTC AND EXPLICIT UTC CALENDAR");
    let before = system::clock_snapshot();
    logger.record(
        "invalid UTC offset leaves the clock state unchanged",
        system::set_clock_utc_offset(86_400) == Err(ClockError::InvalidUtcOffset)
            && system::clock_snapshot() == before,
    );
    let mut seconds = 0x1357_2468;
    let mut nanos = 0x9753;
    let unconfigured = unsafe { runtime::__wii_std_clock_now(true, &mut seconds, &mut nanos) };
    logger.record(
        "unconfigured UTC clock returns an error and preserves outputs",
        unconfigured != 0 && seconds == 0x1357_2468 && nanos == 0x9753,
    );
    let Some(offset) = offset else {
        logger.note("NOT RUN: UTC wall acceptance needs --utc-offset-seconds=<seconds east of UTC> for this Wii");
        logger.record("real UTC offset supplied for wall-clock acceptance", false);
        return;
    };
    logger.note_i32("configured seconds east of UTC", offset);
    let configured = system::set_clock_utc_offset(offset);
    logger.record("explicit UTC offset accepted", configured.is_ok());
    if configured.is_err() {
        return;
    }
    let refreshed = system::synchronize_clock();
    logger.record("actual RTC and console bias refreshed", refreshed.is_ok());
    if let Err(error) = refreshed {
        logger.note("Wall clock unavailable; inspect the retained system ClockError");
        logger.record(
            "fallible wall read preserves refresh failure",
            system::wall_time() == Err(error),
        );
        return;
    }
    let snapshot = system::clock_snapshot();
    logger.record(
        "refreshed wall clock has a valid UTC state",
        snapshot.wall_error.is_none(),
    );
    if snapshot.wall_error.is_some() {
        return;
    }
    let actual = SystemTime::now().duration_since(UNIX_EPOCH);
    let reference = snapshot
        .rtc_seconds
        .zip(snapshot.counter_bias)
        .map(|(rtc, bias)| 946_684_800i64 + i64::from(rtc.wrapping_add(bias)) - i64::from(offset));
    logger.record(
        "actual SystemTime uses the RTC epoch, bias, and supplied UTC offset",
        actual.is_ok_and(|actual| {
            reference.is_some_and(|reference| {
                actual.as_secs() as i64 >= reference && actual.as_secs() as i64 <= reference + 1
            })
        }),
    );
    logger.record(
        "fallible wall facade returns real UTC time",
        system::wall_time().is_ok(),
    );
    logger.record(
        "fallible wall ABI returns real UTC time",
        system::sys::sys_wall_time().is_ok(),
    );
    logger.record(
        "direct clock snapshot agrees with the facade",
        system::sys::sys_clock_snapshot() == system::clock_snapshot(),
    );
    logger.record(
        "direct offset ABI accepts the same explicit setting",
        system::sys::sys_set_clock_utc_offset(offset).is_ok(),
    );
    let rtc_before = snapshot.rtc_seconds;
    let wall_before = SystemTime::now();
    let instant = Instant::now();
    thread::sleep(Duration::from_millis(1100));
    let elapsed = instant.elapsed();
    let refresh = system::sys::sys_synchronize_clock();
    let rtc_after = system::clock_snapshot().rtc_seconds;
    logger.record(
        "RTC seconds and monotonic elapsed agree across a real wait",
        refresh.is_ok()
            && elapsed >= Duration::from_millis(1100)
            && elapsed < Duration::from_secs(3)
            && rtc_before
                .zip(rtc_after)
                .is_some_and(|(before, after)| (1..=3).contains(&after.wrapping_sub(before))),
    );
    if refresh.is_ok() && system::clock_snapshot().wall_error.is_none() {
        logger.record(
            "UTC wall time advances after the real RTC refresh",
            SystemTime::now()
                .duration_since(wall_before)
                .is_ok_and(|value| value > Duration::ZERO && value < Duration::from_secs(4)),
        );
    } else {
        logger.record("UTC wall time refresh remains valid", false);
    }
    let irq = ogc_lwp_driver::critical_enter();
    let masked = system::synchronize_clock();
    ogc_lwp_driver::critical_exit(irq);
    logger.record(
        "RTC refresh rejects masked context before EXI access",
        masked == Err(ClockError::InterruptsDisabled),
    );
}

pub(crate) fn run(logger: &mut ProbeLogger, early: Instant, raw_early: u64) {
    let offset = arguments(logger);
    arithmetic(logger);
    monotonic(logger, early, raw_early);
    deadlines(logger);
    wall(logger, offset);
}
