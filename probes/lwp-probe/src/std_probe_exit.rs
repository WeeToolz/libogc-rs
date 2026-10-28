//! Shared normal-main completion and visible result hold.
use core::sync::atomic::{AtomicI32, Ordering};
use ogc_system_probe::probe_support::{self, ProbeLogger};
use std::process::ExitCode;

#[unsafe(no_mangle)]
pub static WII_STD_SERVICE_PROBE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

extern "C" fn after_main(status: i32) -> ! {
    if status != WII_STD_SERVICE_PROBE_RESULT.load(Ordering::Acquire)
        || ogc_std_runtime::runtime_snapshot().cleanup_result != Some(0)
        || ogc_sys::std_return_result() != Some(0)
    {
        WII_STD_SERVICE_PROBE_RESULT.store(-2, Ordering::Release);
        if let Err(error) = ogc_video::console_write(b"FAIL runtime return cleanup\n") {
            WII_STD_SERVICE_PROBE_RESULT.store(error, Ordering::Release);
        }
    }
    loop {
        probe_support::wait_vsync();
    }
}

pub fn start(name: &'static str) -> ProbeLogger {
    let logger = ogc_system_probe::init_named_probe_logger(name);
    ogc_sys::set_exit_callback(Some(after_main));
    logger
}

pub fn finish(mut logger: ProbeLogger) -> ExitCode {
    logger.summary();
    let status = if logger.checks_passed() { 0 } else { 1 };
    WII_STD_SERVICE_PROBE_RESULT.store(status, Ordering::Release);
    ExitCode::from(status as u8)
}
