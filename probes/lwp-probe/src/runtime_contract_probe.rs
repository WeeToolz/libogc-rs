use ogc_lwp_driver::{LwpClock, LwpDeferredExecutor, LwpInterruptWake, LwpTimerService};
use ogc_runtime::{
    Deadline, DeferredExecutor, InterruptWake, MonotonicClock, ThreadId, TimerService, WakeToken,
    WiiError,
};
use ogc_system_probe::probe_support::ProbeLogger;

fn deferred_noop(_argument: usize) {}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("SHARED RUNTIME SCHEDULER CONTRACTS");

    let thread = ThreadId::new(core::num::NonZeroU32::new(7).unwrap());
    logger.record(
        "runtime thread ID returns its scheduler handle",
        thread.is_some_and(|thread| thread.get() == 7),
    );

    let clock = LwpClock;
    let now = clock.now();
    logger.record("runtime monotonic clock returns a tick value", now > 0);
    logger.record(
        "runtime clock reports the LWP timebase rate",
        clock.ticks_per_second() == ogc_lwp_driver::TICKS_PER_SECOND,
    );

    let mut timer = LwpTimerService;
    logger.record(
        "runtime timer rejects the null wake token",
        timer.register(Deadline(now), WakeToken(0)) == Err(WiiError::InvalidArgument),
    );
    logger.record(
        "runtime timer cancellation accepts an absent token",
        timer.cancel(WakeToken(0)).is_ok(),
    );

    let executor = LwpDeferredExecutor;
    logger.record(
        "runtime deferred executor accepts a callback",
        executor.enqueue(deferred_noop, 0).is_ok(),
    );
    logger.record(
        "runtime deferred executor rejects a null wake token",
        executor.wake(WakeToken(0)) == Err(WiiError::InvalidArgument),
    );

    let interrupt_wake = LwpInterruptWake;
    logger.record(
        "runtime interrupt wake rejects a null token",
        interrupt_wake.wake_from_interrupt(WakeToken(0)) == Err(WiiError::InvalidArgument),
    );
}
