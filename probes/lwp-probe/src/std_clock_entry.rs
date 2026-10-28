#![feature(restricted_std)]
//! Clock acceptance requires an explicit Wii-local UTC offset from the loader.
mod std_clock_probe;
mod std_probe_config;
use core::sync::atomic::{AtomicI32, Ordering};
use ogc_system_probe::probe_support;
use std::process::ExitCode;
use std::time::Instant;

#[unsafe(no_mangle)]
pub static WII_STD_CLOCK_PROBE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

extern "C" fn after_main(status: i32) -> ! {
    if status != WII_STD_CLOCK_PROBE_RESULT.load(Ordering::Acquire) {
        WII_STD_CLOCK_PROBE_RESULT.store(-2, Ordering::Release);
    }
    loop {
        probe_support::wait_vsync();
    }
}

fn main() -> ExitCode {
    // This read precedes pre_main, which used to rewrite the timebase.
    let early = Instant::now();
    let raw_early = ogc_lwp_driver::now_ticks();
    let mut logger = ogc_system_probe::init_named_probe_logger("wii-std-clock-probe");
    ogc_sys::set_exit_callback(Some(after_main));
    std_clock_probe::run(&mut logger, early, raw_early);
    logger.summary();
    let status = if logger.checks_passed() { 0 } else { 1 };
    WII_STD_CLOCK_PROBE_RESULT.store(status, Ordering::Release);
    ExitCode::from(status as u8)
}
