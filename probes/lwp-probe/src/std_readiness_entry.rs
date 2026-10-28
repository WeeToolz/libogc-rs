#![feature(restricted_std)]
//! One normal-main DOL, with shared suites and separate terminal test modes.
mod futex_probe;
mod std_allocator_probe;
mod std_clock_probe;
mod std_diagnostic_probe;
mod std_error_probe;
mod std_fs_probe;
mod std_identity_probe;
mod std_network_probe;
mod std_panic_probe;
mod std_platform_probe;
mod std_probe_config;
mod std_sync_probe;
mod std_thread_lifecycle;
mod std_thread_probe;
mod std_usb_probe;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
static FOCUS: AtomicU32 = AtomicU32::new(0);
use ogc_system_probe::probe_support::ProbeLogger;
use std::process::ExitCode;
use std::time::Instant;

#[unsafe(no_mangle)]
pub static WII_STD_READINESS_PROBE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

fn suite(logger: &mut ProbeLogger, name: &str, run: impl FnOnce(&mut ProbeLogger)) {
    let focus = FOCUS.load(Ordering::Relaxed);
    let selected = focus == 0
        || matches!(
            name,
            "NORMAL ENTRY, THREADS AND TLS" | "CLOCKS AND DEADLINES"
        )
        || (focus == 1 && name == "TCP, UDP AND DNS")
        || (focus == 2
            && matches!(
                name,
                "SD FILES AND ENVIRONMENT" | "USB FAT FILES AND ENVIRONMENT"
            ))
        || (focus == 3 && name == "USB IOS AND DEFERRED CALLBACKS");
    if !selected {
        logger.note(&format!("UNTESTED this focused run: {name}"));
        return;
    }
    logger.section(name);
    ogc_system_probe::probe_support::send_agent_progress(4);
    let before = logger.check_counts();
    run(logger);
    let after = logger.check_counts();
    let details = format!(
        "{} passed, {} failed",
        after.0 - before.0,
        after.1 - before.1
    );
    logger.note(&format!("{name}: {details}"));
    ogc_system_probe::probe_support::send_agent_progress(5);
}

fn main() -> ExitCode {
    // Capture before IOS pre-main can alter clock or diagnostic state.
    let early = Instant::now();
    let raw_early = ogc_lwp_driver::now_ticks();
    let missing = std_diagnostic_probe::before_console();
    let mut logger = ogc_system_probe::init_named_probe_logger("wii-std-readiness-probe");
    logger.enable_activity_indicator();
    std_identity_probe::run(&mut logger);
    match std_probe_config::value("focus") {
        Ok(value) => {
            let value = value.as_deref().unwrap_or("all");
            let focus = match value {
                "all" => Some(0),
                "network" => Some(1),
                "storage" => Some(2),
                "usb" => Some(3),
                _ => None,
            };
            if let Some(focus) = focus {
                FOCUS.store(focus, Ordering::Relaxed);
                logger.note(&format!(
                    "Run scope: {value}; unselected suites are UNTESTED, never counted as passes."
                ));
            } else {
                logger.record("focus is all, network, storage, or usb", false);
            }
        }
        Err(error) => logger.record_with_detail("focus option is valid", false, error),
    }

    let mode = match std_probe_config::value("mode") {
        Ok(value) => value.unwrap_or_else(|| "normal".into()),
        Err(error) => {
            logger.record_with_detail("loader mode is valid", false, error);
            "invalid".into()
        }
    };
    match mode.as_str() {
        "panic" => std_panic_probe::run(&mut logger),
        "entropy" => {
            logger.section("TERMINAL STD ENTROPY REJECTION");
            logger.note("Expect: Wii random data is unsupported: no verified entropy service");
            logger.note("This mode must stop in the default panic hook; it has no PASS summary.");
            drop(std::collections::HashMap::<u32, u32>::new());
            panic!("ERROR: default HashMap unexpectedly accepted an unverified seed");
        }
        "normal" => {}
        _ => logger.record("mode is normal, panic, or entropy", false),
    }
    suite(&mut logger, "DIAGNOSTICS", |logger| {
        std_diagnostic_probe::run(logger, missing)
    });
    suite(
        &mut logger,
        "ALLOCATOR AND MEMORY OWNERSHIP",
        std_allocator_probe::run,
    );
    suite(
        &mut logger,
        "NORMAL ENTRY, THREADS AND TLS",
        std_thread_lifecycle::run,
    );
    suite(&mut logger, "PRODUCTION FUTEX TRANSPORT", futex_probe::run);
    suite(&mut logger, "STD SYNCHRONIZATION", std_sync_probe::run);
    suite(&mut logger, "CLOCKS AND DEADLINES", |logger| {
        std_clock_probe::run(logger, early, raw_early)
    });
    suite(&mut logger, "TCP, UDP AND DNS", std_network_probe::run);
    match std_probe_config::value("media") {
        Ok(selection) => {
            let selection = selection.as_deref().unwrap_or("both");
            if !matches!(selection, "sd" | "usb" | "both") {
                logger.record_with_detail("media option is valid", false, "use sd, usb, or both");
            } else {
                for (name, medium, selected) in [
                    (
                        "SD FILES AND ENVIRONMENT",
                        std_fs_probe::Medium::Sd,
                        selection != "usb",
                    ),
                    (
                        "USB FAT FILES AND ENVIRONMENT",
                        std_fs_probe::Medium::Usb,
                        selection != "sd",
                    ),
                ] {
                    if selected {
                        suite(&mut logger, name, |logger| {
                            std_fs_probe::run_medium(logger, medium)
                        });
                    } else {
                        logger.note(&format!(
                            "UNTESTED {name}: excluded by explicit media selection"
                        ));
                    }
                }
            }
        }
        Err(error) => logger.record_with_detail("media option is valid", false, error),
    }
    suite(
        &mut logger,
        "USB IOS AND DEFERRED CALLBACKS",
        std_usb_probe::run,
    );
    suite(
        &mut logger,
        "NATIVE PLATFORM LIMITS",
        std_platform_probe::run,
    );
    // Deliberate cleanup faults run last, after the normal baseline checks.
    suite(
        &mut logger,
        "ERROR TRANSLATION AND CONTROLLED CLEANUP",
        std_error_probe::run,
    );
    logger.section("REMAINING READINESS GATES");
    logger.note("Entropy remains unverified. Use mode=entropy for its terminal rejection test.");
    logger.note("Use mode=panic for the default panic-abort diagnostic test.");
    logger.note("Media-removal and power-loss tests require separate controlled runs.");
    // The shared callback checks real main return and initial-thread cleanup
    // before it prints the one final summary and publishes the result.
    std_thread_lifecycle::finish_with_menu_return(logger, &WII_STD_READINESS_PROBE_RESULT, 5)
}
