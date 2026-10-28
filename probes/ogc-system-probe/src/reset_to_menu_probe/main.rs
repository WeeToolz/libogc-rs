#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};
use ogc_system as system;
use ogc_system_probe::probe_support::{self, ProbeDisplay, ProbeLogger, ProbeLoggerOptions};

/// Debuggers can inspect the last completed application stage without a network.
/// 1: entry, 2: system initialization, 3: pre-main, 4: report returned,
/// 5: reset requested, 6: reset unexpectedly returned, MAX: panic.
#[unsafe(no_mangle)]
pub static RESET_PROBE_STAGE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub static RESET_PROBE_RESET_TRACE: AtomicU32 = AtomicU32::new(0);
static RESET_TRACE_RECORDS: AtomicU32 = AtomicU32::new(0);

unsafe fn reset_progress(stage: u32, priority: u32, result: i32) {
    RESET_PROBE_RESET_TRACE.store(stage, Ordering::Release);
    if RESET_TRACE_RECORDS.fetch_add(1, Ordering::Relaxed) < 64 {
        system::report(format_args!(
            "RESET_PROBE: reset stage={stage} priority={priority} result={result}\n"
        ));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    RESET_PROBE_STAGE.store(1, Ordering::Release);
    system::init();
    // SAFETY: the runtime supplies valid argc/argv. The helper copies them.
    unsafe { probe_support::set_agent_arguments(argc, argv) };
    RESET_PROBE_STAGE.store(2, Ordering::Release);
    system::report(format_args!("RESET_PROBE: entered diagnostic DOL\n"));
    let display = ProbeDisplay::init();
    display.println("RESET PROBE: DOL entry reached");
    system::pre_main();
    RESET_PROBE_STAGE.store(3, Ordering::Release);
    // No SD, USB, or NAND log is opened. Only the existing report transport is used.
    let mut logger = ProbeLogger::with_options(display, ProbeLoggerOptions::disabled());
    logger.section("RESET TO SYSTEM MENU");
    let startup = system::snapshot_startup();
    logger.record("system initialization completed", startup.init_completed);
    logger.record(
        "pre-main initialization completed",
        system::snapshot_pre_main().completed,
    );
    logger.record(
        "Wiiload report identity available",
        probe_support::report_identity_available(),
    );
    logger.note("DOL entry reached; System Menu reset will be requested after this report");
    logger.note("This report does not confirm reset completion");
    logger.summary_before_explicit_exit();
    RESET_PROBE_STAGE.store(4, Ordering::Release);
    system::report(format_args!("RESET_PROBE: requesting System Menu reset\n"));
    system::sys::set_reset_debug_hook(Some(reset_progress));
    RESET_PROBE_STAGE.store(5, Ordering::Release);
    // SAFETY: this DOL's explicit purpose is to return to the System Menu.
    // This is the production reset path, with no firmware installation or writes.
    unsafe { system::reset_system(system::SYS_RETURNTOMENU as i32, 0, 0) };
    RESET_PROBE_STAGE.store(6, Ordering::Release);
    system::report(format_args!("RESET_PROBE: ERROR reset call returned\n"));
    // Do not return through an HBC stub: that would hide a failed menu reset.
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    RESET_PROBE_STAGE.store(u32::MAX, Ordering::Release);
    system::report(format_args!("RESET_PROBE: PANIC {info}\n"));
    loop {
        core::hint::spin_loop();
    }
}
