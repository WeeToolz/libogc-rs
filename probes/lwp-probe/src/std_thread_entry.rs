#![feature(restricted_std)]
//! Standalone normal-main and post-return checks use the shared lifecycle suite.
mod std_thread_lifecycle;
mod std_thread_probe;
use core::sync::atomic::AtomicI32;
use std::process::ExitCode;
#[unsafe(no_mangle)]
pub static WII_STD_THREAD_PROBE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
fn main() -> ExitCode {
    let mut logger = ogc_system_probe::init_named_probe_logger("wii-std-thread-probe");
    std_thread_lifecycle::run(&mut logger);
    std_thread_lifecycle::finish(logger, &WII_STD_THREAD_PROBE_RESULT)
}
