#![cfg_attr(target_arch = "powerpc", no_std)]
//! Hardware probe binaries for validating libogc-rs on Wii/GameCube targets.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
extern crate alloc;

#[cfg(target_arch = "powerpc")]
pub mod aesnd_native_probe;
#[cfg(target_arch = "powerpc")]
mod aesnd_output_evidence;
#[cfg(target_arch = "powerpc")]
mod aesnd_stream_probe;
#[cfg(target_arch = "powerpc")]
pub mod aram_probe;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub mod arena_probe;
#[cfg(target_arch = "powerpc")]
pub mod bluetooth_probe;
#[cfg(target_arch = "powerpc")]
pub mod bt_sync_menu;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub mod bus_probe;
#[cfg(target_arch = "powerpc")]
pub mod callback_probe;
#[cfg(all(target_arch = "powerpc", feature = "agent-report"))]
mod capture_append;
pub mod controller;
#[cfg(target_arch = "powerpc")]
pub mod debugger_probe;
#[cfg(target_arch = "powerpc")]
pub mod event_line;
#[cfg(target_arch = "powerpc")]
mod exfat_sd_probe;
#[cfg(target_arch = "powerpc")]
mod fat_sd_probe;
#[cfg(target_arch = "powerpc")]
pub mod font_probe;
#[cfg(target_arch = "powerpc")]
pub mod general_probe;
#[cfg(target_arch = "powerpc")]
mod hid_descriptor_probe;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub mod hid_probe;
#[cfg(target_arch = "powerpc")]
pub mod input_probe;
#[cfg(target_arch = "powerpc")]
pub mod ipc_async_probe;
#[cfg(target_arch = "powerpc")]
pub mod keyboard_probe;
#[cfg(target_arch = "powerpc")]
mod modplay_probe;
#[cfg(target_arch = "powerpc")]
pub mod network_probe;
#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub mod pad_diagnostics_probe;
#[cfg(target_arch = "powerpc")]
pub mod probe_support;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub mod runtime_probe;
#[cfg(target_arch = "powerpc")]
pub mod sound_probe;
#[cfg(target_arch = "powerpc")]
pub mod sram_probe;
#[cfg(target_arch = "powerpc")]
pub mod startup_probe;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
mod stm_api_probe;
#[cfg(target_arch = "powerpc")]
pub mod storage_probe;
#[cfg(all(target_arch = "powerpc", feature = "ui-input-probe"))]
mod ui_input_probe;
#[cfg(all(target_arch = "powerpc", feature = "ui-input-probe"))]
mod ui_input_probe_core;
#[cfg(target_arch = "powerpc")]
mod usb_volumes_probe;
#[cfg(target_arch = "powerpc")]
mod usbstorage_capacity_probe;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
mod usbstorage_transport_probe;
#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub mod video_probe;
#[cfg(target_arch = "powerpc")]
mod wd_probe;
#[cfg(target_arch = "powerpc")]
pub mod wii_remote_stability_probe;
#[cfg(target_arch = "powerpc")]
mod wiisd_probe;
#[cfg(target_arch = "powerpc")]
pub mod wpad_probe;

#[cfg(target_arch = "powerpc")]
pub mod alloc_support {
    use core::alloc::{GlobalAlloc, Layout};
    use core::ptr::null_mut;
    use core::sync::atomic::{AtomicU8, Ordering};
    use ogc_alloc::ArenaAllocator;

    const HEAP_BYTES: usize = 2 * 1024 * 1024;

    #[repr(align(32))]
    struct Heap([u8; HEAP_BYTES]);

    /// Fixed-region allocator used by probe and example binaries on bare hardware.
    pub struct BumpAllocator;

    static mut HEAP: Heap = Heap([0; HEAP_BYTES]);
    static ARENA: ArenaAllocator = ArenaAllocator::new();
    static ARENA_STATE: AtomicU8 = AtomicU8::new(0);

    fn ensure_initialized() -> bool {
        loop {
            match ARENA_STATE.load(Ordering::Acquire) {
                2 => return true,
                3 => return false,
                1 => core::hint::spin_loop(),
                _ => {
                    if ARENA_STATE
                        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                        .is_err()
                    {
                        continue;
                    }
                    let start = unsafe { core::ptr::addr_of_mut!(HEAP.0).cast::<u8>() };
                    let result = unsafe { ARENA.initialize_region(start, HEAP_BYTES) };
                    ARENA_STATE.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release);
                    return result.is_ok();
                }
            }
        }
    }

    unsafe impl GlobalAlloc for BumpAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if !ensure_initialized() {
                return null_mut();
            }
            ARENA.allocate(layout.size(), layout.align())
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            if ARENA_STATE.load(Ordering::Acquire) == 2 {
                unsafe { ARENA.deallocate(pointer, layout.size(), layout.align()) };
            }
        }
    }

    #[cfg_attr(not(feature = "std-system-allocator"), global_allocator)]
    pub static ALLOCATOR: BumpAllocator = BumpAllocator;

    pub fn used_bytes() -> usize {
        if ARENA_STATE.load(Ordering::Acquire) == 2 {
            ARENA.used_bytes()
        } else {
            0
        }
    }
}

#[cfg(target_arch = "powerpc")]
/// Initializes the standard console + persistent probe logger for a focused
/// hardware probe. Logs are appended to ISFS and, when enabled, the mounted
/// device selected by the existing probe logger policy.
pub fn init_named_probe_logger(app_dir: &'static str) -> probe_support::ProbeLogger {
    configure_probe_app_dir(app_dir);
    init_probe_logger()
}

#[cfg(target_arch = "powerpc")]
fn init_probe_logger() -> probe_support::ProbeLogger {
    init_probe_logger_with_page_reports_and_sync(false, log_options(), true)
}

#[cfg(target_arch = "powerpc")]
fn init_probe_logger_with_page_reports(
    page_reports: bool,
    options: probe_support::ProbeLoggerOptions,
) -> probe_support::ProbeLogger {
    init_probe_logger_with_page_reports_and_sync(page_reports, options, true)
}

#[cfg(target_arch = "powerpc")]
fn init_probe_logger_without_display_sync() -> probe_support::ProbeLogger {
    init_probe_logger_with_page_reports_and_sync(false, log_options(), false)
}

#[cfg(target_arch = "powerpc")]
fn init_probe_logger_with_page_reports_and_sync(
    page_reports: bool,
    options: probe_support::ProbeLoggerOptions,
    wait_for_display_sync: bool,
) -> probe_support::ProbeLogger {
    use ogc_system::{self as system};

    static IPC_PROGRESS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
    #[cfg(feature = "dolphin-automation")]
    static PRE_MAIN_SEQUENCE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

    unsafe fn ipc_progress(stage: u32, status: u32, _b: u32, _c: u32) {
        use core::sync::atomic::Ordering;

        fn write_registers(status: u32, arm_message: u32) {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut line = *b"IPC: ctrl=0x00000000 arm=0x00000000\n";
            for index in 0..8 {
                let shift = (7 - index) * 4;
                line[12 + index] = HEX[((status >> shift) & 0xf) as usize];
                line[27 + index] = HEX[((arm_message >> shift) & 0xf) as usize];
            }
            let _ = probe_support::screen_write(&line);
            ogc_video::flush();
        }

        let (bit, message): (u32, &[u8]) = match stage {
            0 => (1 << 0, b"IPC: sync state ready\n"),
            1 => (1 << 1, b"IPC: request prepared\n"),
            5 => (1 << 2, b"IPC: request sent\n"),
            20 if status & 0x22 == 0x22 => (1 << 3, b"IPC: IOS acknowledged request\n"),
            20 if status & 0x14 == 0x14 => (1 << 4, b"IPC: IOS reply ready\n"),
            20 => {
                if IPC_PROGRESS.fetch_or(1 << 5, Ordering::AcqRel) & (1 << 5) == 0 {
                    let _ = probe_support::screen_write(b"IPC: polling, no completion bits\n");
                    write_registers(status, _b);
                }
                return;
            }
            _ => return,
        };
        if IPC_PROGRESS.fetch_or(bit, Ordering::AcqRel) & bit == 0 {
            if stage == 1 || stage == 5 {
                let mut line = *b"IPC: command=0x00000000 fd=0x00000000\n";
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for index in 0..8 {
                    let shift = (7 - index) * 4;
                    line[15 + index] = HEX[((status >> shift) & 0xf) as usize];
                    line[29 + index] = HEX[((_b >> shift) & 0xf) as usize];
                }
                if probe_support::screen_write(&line).is_err() {
                    probe_support::OGC_PROBE_BOOT_STAGE.store(u32::MAX, Ordering::Release);
                }
            }
            let _ = probe_support::screen_write(message);
            ogc_video::flush();
        }
    }

    unsafe fn pre_main_progress(stage: u32) {
        #[cfg(feature = "dolphin-automation")]
        use core::sync::atomic::Ordering;

        #[cfg(feature = "dolphin-automation")]
        if PRE_MAIN_SEQUENCE
            .compare_exchange(stage, stage + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            PRE_MAIN_SEQUENCE.store(u32::MAX, Ordering::Release);
        }
        IPC_PROGRESS.store(0, core::sync::atomic::Ordering::Release);
        let message: &[u8] = match stage {
            0 => b"PRE 0/7: closing inherited IOS descriptors\n",
            1 => b"PRE 1/7: IOS startup\n",
            2 => b"PRE 2/7: IOS subsystems\n",
            3 => b"PRE 3/7: STM handler\n",
            4 => b"PRE 4/7: STM devices\n",
            5 => b"PRE 5/7: configuration\n",
            6 => b"PRE 6/7: Wii launch state\n",
            7 => b"PRE 7/7: complete\n",
            _ => b"PRE unknown\n",
        };
        let _ = probe_support::screen_write(message);
        ogc_video::flush();
    }

    system::init();
    probe_support::OGC_PROBE_BOOT_STAGE.store(1, core::sync::atomic::Ordering::Release);
    system::check_argv();
    probe_support::capture_system_agent_arguments();

    // Bring up VI before IOS pre-main work. A stalled synchronous IPC request
    // now leaves a visible boot-stage marker instead of an undifferentiated
    // black screen.
    let display = probe_support::ProbeDisplay::init();
    display.println("BOOT 1/3: system and video initialized");
    display.print_agent_argument_status();
    let startup = system::snapshot_startup();
    if !startup.init_completed {
        display.println("FAIL SYS_Init incomplete");
    } else if startup.init_failed_stage != 0 {
        display.println("WARN SYS_Init completed with a failed nonfatal step");
    } else {
        display.println("PASS SYS_Init completed");
    }
    display.print_startup_status(&startup);
    display.println("BOOT watchdog arm starting");
    if probe_support::arm_agent_recovery_watchdog() {
        display.println("INFO recovery reset watchdog armed");
    } else {
        display.println("INFO recovery reset watchdog not armed");
    }
    display.println("BOOT watchdog arm complete");
    display.println("BOOT Wiiload report argument check");
    #[cfg(feature = "agent-report")]
    display.println(if probe_support::report_identity_available() {
        "PASS Wiiload report arguments parsed"
    } else {
        "FAIL Wiiload report arguments missing after argv parse"
    });
    #[cfg(not(feature = "agent-report"))]
    display.println("INFO Wiiload report check disabled for this probe build");
    display.println("BOOT before first vsync");
    if wait_for_display_sync {
        display.wait_vsync();
    }
    display.println("BOOT 2/3: entering IOS pre-main");
    if wait_for_display_sync {
        display.wait_vsync();
    }
    probe_support::OGC_PROBE_BOOT_STAGE.store(2, core::sync::atomic::Ordering::Release);
    ogc_ipc::set_debug_hook(Some(ipc_progress));
    system::sys::set_pre_main_progress_callback(Some(pre_main_progress));
    system::pre_main();
    system::sys::set_pre_main_progress_callback(None);
    ogc_ipc::set_debug_hook(None);
    probe_support::OGC_PROBE_BOOT_STAGE.store(3, core::sync::atomic::Ordering::Release);
    let sd_run_log = options.file_policy == probe_support::LogFilePolicy::SdRunLog;
    if !sd_run_log {
        probe_support::send_agent_progress(3);
    }
    if page_reports && !sd_run_log {
        display.enable_page_reports();
    }
    display.println("BOOT 3/3: IOS pre-main completed");
    if page_reports && !sd_run_log {
        display.capture_page();
    }
    if wait_for_display_sync {
        display.wait_vsync();
    }

    system::stdio_report(true);
    let mount_status = maybe_mount_external_logs();

    let mut logger = probe_support::ProbeLogger::with_options(display, options);
    #[cfg(feature = "dolphin-automation")]
    {
        let pre_main = system::sys::snapshot_pre_main();
        logger.record(
            "pre-main progress callback reports stages in order",
            PRE_MAIN_SEQUENCE.load(core::sync::atomic::Ordering::Acquire) == 8,
        );
        logger.record(
            "system pre-main completes without a failed stage",
            pre_main.called
                && pre_main.completed
                && pre_main.failed_stage == 0
                && pre_main.result == 0,
        );
    }
    let log_ready = if logger.logging_requested() {
        logger.start_logging()
    } else {
        false
    };
    logger.record("display initialized", logger.display().console_ready());
    logger.note("boot stage: system/video initialized");
    logger.note("boot stage: IOS pre-main completed");
    if logger.logging_requested() {
        logger.record("probe log opened before scenarios", log_ready);
        logger.note(logger.log_backend_name());
    } else {
        logger.note("log backend disabled");
    }
    if sd_run_log {
        if page_reports {
            logger.enable_page_reports();
        }
        probe_support::send_agent_progress(3);
    }
    #[cfg(feature = "mounted-log")]
    if let Some(argv0) = probe_support::argv0_bytes() {
        logger.note(&alloc::format!(
            "argv0={}",
            core::str::from_utf8(argv0).unwrap_or("<non-utf8>")
        ));
    } else {
        logger.note("argv0 unavailable");
        if let Some(argv) = probe_support::startup_argv_summary() {
            logger.note(&alloc::format!(
                "argv raw magic=0x{:08x} len={} argc={} cmd=0x{:08x} argv=0x{:08x} end=0x{:08x}",
                argv.argv_magic as u32,
                argv.length,
                argv.argc,
                argv.command_line as u32,
                argv.argv as u32,
                argv.end_argv as u32
            ));
        }
    }
    let power = probe_support::install_power_shutdown();
    logger.record("power button callback installed", true);
    logger.note("power-button STM actions deferred until shutdown");
    #[cfg(feature = "mounted-log")]
    logger.note(&alloc::format!(
        "fat mount sd={} usb={}",
        mount_status.sd,
        mount_status.usb
    ));
    #[cfg(not(feature = "mounted-log"))]
    {
        logger.note("fat mount sd");
        logger.note(mount_status.sd);
        logger.note("fat mount usb");
        logger.note(mount_status.usb);
    }
    logger.note_u32(
        "previous stm handler chained",
        power.chained_previous_stm_handler as u32,
    );
    if power.had_previous_power_callback {
        logger.note("previous power callback replaced");
    } else {
        logger.record("previous power callback absent", true);
    }
    logger
}

#[cfg(target_arch = "powerpc")]
struct MountStatus {
    sd: &'static str,
    usb: &'static str,
}

#[cfg(all(target_arch = "powerpc", feature = "mounted-log"))]
fn maybe_mount_external_logs() -> MountStatus {
    use ogc_fat as fat;

    fn status(result: Result<(), fat::FatError>) -> &'static str {
        match result {
            Ok(()) => "ok",
            Err(fat::FatError::AlreadyMounted) => "already-mounted",
            Err(fat::FatError::AlreadyExists) => "already-exists",
            Err(fat::FatError::Busy) => "busy",
            Err(fat::FatError::DeviceUnavailable) => "device-unavailable",
            Err(fat::FatError::MediaRemoved) => "media-removed",
            Err(fat::FatError::InvalidBootSector) => "invalid-boot-sector",
            Err(fat::FatError::UnsupportedVariant) => "unsupported-variant",
            Err(fat::FatError::PathError) => "path-error",
            Err(fat::FatError::Io) => "io",
            Err(fat::FatError::NotFound) => "not-found",
            Err(fat::FatError::NoSpace) => "no-space",
            Err(fat::FatError::ReadOnly) => "read-only",
            Err(fat::FatError::IsDirectory) => "is-directory",
            Err(fat::FatError::NotDirectory) => "not-directory",
            Err(fat::FatError::DirectoryNotEmpty) => "directory-not-empty",
            Err(fat::FatError::NotSupported) => "not-supported",
            Err(fat::FatError::PermissionDenied) => "permission-denied",
            Err(fat::FatError::CrossDevice) => "cross-device",
            Err(fat::FatError::BadDescriptor) => "bad-descriptor",
            Err(fat::FatError::FileTooLarge) => "file-too-large",
        }
    }

    let argv0 = probe_support::argv0_bytes();
    let from_sd = argv0.is_some_and(|path| path.starts_with(b"sd:/"));
    let from_usb = argv0.is_some_and(|path| path.starts_with(b"usb:/"));

    MountStatus {
        sd: if from_sd {
            status(fat::mount_sd())
        } else {
            "not-selected"
        },
        usb: if from_usb {
            status(fat::mount_usb())
        } else {
            "not-selected"
        },
    }
}

#[cfg(all(target_arch = "powerpc", not(feature = "mounted-log")))]
fn maybe_mount_external_logs() -> MountStatus {
    MountStatus {
        sd: "feature-disabled",
        usb: "feature-disabled",
    }
}

#[cfg(all(
    target_arch = "powerpc",
    feature = "mounted-log",
    not(feature = "dolphin-automation")
))]
const fn log_options() -> probe_support::ProbeLoggerOptions {
    probe_support::ProbeLoggerOptions::mounted_only()
}

#[cfg(all(
    target_arch = "powerpc",
    any(not(feature = "mounted-log"), feature = "dolphin-automation")
))]
const fn log_options() -> probe_support::ProbeLoggerOptions {
    probe_support::ProbeLoggerOptions::disabled()
}

#[cfg(target_arch = "powerpc")]
fn configure_probe_app_dir(app_dir: &'static str) {
    probe_support::set_probe_app_dir(app_dir);
}

#[cfg(all(target_arch = "powerpc", not(any(feature = "agent-report", ogc_dev))))]
fn idle_forever() -> ! {
    loop {
        probe_support::wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
fn finish_probe() -> i32 {
    #[cfg(all(not(feature = "agent-report"), not(ogc_dev)))]
    idle_forever();
    #[cfg(any(feature = "agent-report", ogc_dev))]
    {
        0
    }
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs the full hardware probe suite, then enters the live input monitor in standalone mode.
pub fn run_system_probe() -> i32 {
    configure_probe_app_dir("ogc-system-probe");
    let options = if cfg!(all(
        any(feature = "agent-report", ogc_dev),
        not(feature = "dolphin-automation")
    )) {
        probe_support::ProbeLoggerOptions {
            file_policy: probe_support::LogFilePolicy::SdRunLog,
        }
    } else {
        log_options()
    };
    let mut logger = init_probe_logger_with_page_reports(true, options);
    if logger.uses_mounted_log()
        && let Some((run_id, build_id)) = probe_support::input_run_identity()
    {
        logger.note(&alloc::format!("SD run log: sd:/ogc-probe-{run_id}.log"));
        logger.note(&alloc::format!("DOL SHA-256: {build_id}"));
    }

    logger.enable_activity_indicator();

    logger.report_step("General");
    general_probe::run(&mut logger);
    logger.report_step("Startup");
    startup_probe::run(&mut logger);
    logger.report_step("SRAM");
    sram_probe::run(&mut logger);
    logger.report_step("Arena");
    arena_probe::run(&mut logger);
    logger.report_step("Callbacks");
    callback_probe::run(&mut logger);
    logger.report_step("Runtime");
    runtime_probe::run(&mut logger);
    logger.report_step("Font");
    font_probe::run(&mut logger);
    logger.report_step("Bus");
    bus_probe::run(&mut logger);
    logger.report_step("Video");
    video_probe::run(&mut logger);
    logger.report_step("Sound");
    sound_probe::run(&mut logger);
    logger.report_step("Network");
    network_probe::run(&mut logger);
    logger.report_step("Storage");
    storage_probe::run(&mut logger, false);
    logger.report_step("Keyboard");
    keyboard_probe::run(&mut logger);

    logger.summary();
    #[cfg(any(feature = "agent-report", ogc_dev))]
    {
        finish_probe()
    }
    #[cfg(all(not(feature = "agent-report"), not(ogc_dev)))]
    input_probe::monitor(&mut logger);
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
fn run_pad_probe(
    app_dir: &'static str,
    probe: impl FnOnce(&mut probe_support::ProbeLogger),
) -> i32 {
    configure_probe_app_dir(app_dir);
    let mut logger = init_probe_logger();
    logger.note("PAD probe persistent-log schema version 1");
    probe(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_state_probe_bin() -> i32 {
    run_pad_probe("pad-state-probe", pad_diagnostics_probe::run_state)
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_command_probe_bin(spec: u32) -> i32 {
    let app_dir = match spec {
        0 => "pad-command-spec0",
        1 => "pad-command-spec1",
        2 => "pad-command-spec2",
        4 => "pad-command-spec4",
        _ => "pad-command-spec5",
    };
    run_pad_probe(app_dir, |logger| {
        pad_diagnostics_probe::run_command(logger, spec)
    })
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_wired_probe_bin() -> i32 {
    run_pad_probe("pad-wired-probe", pad_diagnostics_probe::run_wired)
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_wavebird_probe_bin() -> i32 {
    run_pad_probe("pad-wavebird-probe", pad_diagnostics_probe::run_wavebird)
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_stress_probe_bin() -> i32 {
    run_pad_probe("pad-stress-probe", pad_diagnostics_probe::run_stress)
}

#[cfg(all(target_arch = "powerpc", feature = "pad-diagnostics"))]
pub fn run_pad_reset_probe_bin() -> i32 {
    run_pad_probe("pad-reset-probe", pad_diagnostics_probe::run_reset)
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs only bus-related probes.
pub fn run_bus_probe_bin() -> i32 {
    configure_probe_app_dir("bus-probe");
    let mut logger = init_probe_logger();
    bus_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
pub fn run_arena_probe_bin() -> i32 {
    configure_probe_app_dir("arena-probe");
    let mut logger = init_probe_logger();
    arena_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
pub fn run_callback_probe_bin() -> i32 {
    configure_probe_app_dir("callback-probe");
    let mut logger = init_probe_logger();
    callback_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only general startup probes.
pub fn run_general_probe_bin() -> i32 {
    configure_probe_app_dir("general-probe");
    let mut logger = init_probe_logger();
    general_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs only video probes.
pub fn run_video_probe_bin() -> i32 {
    configure_probe_app_dir("video-probe");
    let mut logger = init_probe_logger();
    video_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs only focused GX public API probes.
pub fn run_gx_api_probe_bin() -> i32 {
    configure_probe_app_dir("gx-api-probe");
    let mut logger = init_probe_logger();
    video_probe::run_gx_api_probe(&mut logger);
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs focused STM interface and LED API probes.
pub fn run_stm_api_probe_bin() -> i32 {
    configure_probe_app_dir("stm-api-probe");
    let mut logger = init_probe_logger();
    #[cfg(feature = "stm-api-probe")]
    stm_api_probe::run_close(&mut logger);
    #[cfg(not(feature = "stm-api-probe"))]
    stm_api_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Sends a small report and framebuffer to the host to check the return path.
pub fn run_report_smoke_probe_bin() -> i32 {
    configure_probe_app_dir("report-smoke-probe");
    let mut logger = init_probe_logger();
    logger.record(
        "Wiiload report identity available",
        probe_support::report_identity_available(),
    );
    logger.record("report smoke probe started", true);
    logger.note("report transport smoke test; no SD card is required");
    if probe_support::failure_injection_requested() {
        logger.record("requested probe failure injection", false);
    }
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs only runtime/IOS/crypto probes.
pub fn run_runtime_probe_bin() -> i32 {
    configure_probe_app_dir("runtime-probe");
    let mut logger = init_probe_logger();
    runtime_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only font probes.
pub fn run_font_probe_bin() -> i32 {
    configure_probe_app_dir("font-probe");
    let mut logger = init_probe_logger();
    font_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
pub fn run_sram_probe_bin() -> i32 {
    configure_probe_app_dir("sram-probe");
    let mut logger = init_probe_logger();
    sram_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
pub fn run_startup_probe_bin() -> i32 {
    configure_probe_app_dir("startup-probe");
    let mut logger = init_probe_logger();
    startup_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks real AESND DSP output, process metrics and reset/reinit.
pub fn run_aesnd_native_probe_bin() -> i32 {
    configure_probe_app_dir("aesnd-native-probe");
    let mut logger = init_probe_logger_with_page_reports(true, log_options());
    logger.enable_activity_indicator();
    logger.report_step("AESND probe startup complete");
    aesnd_native_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only sound probes.
pub fn run_sound_probe_bin() -> i32 {
    configure_probe_app_dir("sound-probe");
    let mut logger = init_probe_logger();
    sound_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks the low-level AI register and DSP DMA audio APIs.
pub fn run_audio_sys_probe_bin() -> i32 {
    configure_probe_app_dir("audio-sys-probe");
    let mut logger = init_probe_logger();
    logger.section("OGC AUDIO SYS PROBE");
    sound_probe::probe_audio_sys(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks raw and owned ASND voice APIs.
pub fn run_asnd_sys_probe_bin() -> i32 {
    configure_probe_app_dir("asnd-sys-probe");
    let mut logger = init_probe_logger();
    sound_probe::probe_asnd_sys(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks that ModPlay mixes in its LWP worker and stops that worker cleanly.
pub fn run_modplay_probe_bin() -> i32 {
    configure_probe_app_dir("modplay-probe");
    let mut logger = init_probe_logger();
    modplay_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks MP3 buffer and reader playback, callbacks, pause and cleanup.
pub fn run_mp3player_probe_bin(run: fn(&mut probe_support::ProbeLogger)) -> i32 {
    configure_probe_app_dir("mp3player-probe");
    let mut logger = init_probe_logger();
    run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks ARAM DMA, allocation, ARQ scheduling, and the ARQM stack on Dolphin.
pub fn run_aram_probe_bin() -> i32 {
    configure_probe_app_dir("aram-probe");
    let mut logger = init_probe_logger();
    aram_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks USB storage capacity response decoding through the production helper.
pub fn run_usbstorage_capacity_probe_bin() -> i32 {
    configure_probe_app_dir("usbstorage-capacity-probe");
    let mut logger = init_probe_logger();
    usbstorage_capacity_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks Wii SD startup, read, empty write, shutdown, and restart.
pub fn run_wiisd_probe_bin() -> i32 {
    configure_probe_app_dir("wiisd-probe");
    let mut logger = init_probe_logger();
    wiisd_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks the read-only exFAT API through the Wii SD driver.
pub fn run_exfat_sd_probe_bin() -> i32 {
    configure_probe_app_dir("exfat-sd-probe");
    let mut logger = init_probe_logger();
    exfat_sd_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks FAT C and Rust APIs on the Dolphin virtual SD card.
pub fn run_fat_sd_probe_bin() -> i32 {
    configure_probe_app_dir("fat-sd-probe");
    let mut logger = init_probe_logger();
    fat_sd_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks two USB media through the real FAT and USB APIs without file writes.
pub fn run_usb_volumes_probe_bin() -> i32 {
    configure_probe_app_dir("usb-volumes-probe");
    let mut logger = init_probe_logger();
    usb_volumes_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Checks physical USB recovery and repeated reads without raw sector writes.
pub fn run_usbstorage_transport_probe_bin() -> i32 {
    configure_probe_app_dir("usbstorage-transport-probe");
    let mut logger = init_probe_logger();
    usbstorage_transport_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks HID descriptor conversion and v5 preflight with fixed operation results.
pub fn run_hid_descriptor_probe_bin() -> i32 {
    configure_probe_app_dir("hid-descriptor-probe");
    let mut logger = init_probe_logger();
    hid_descriptor_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only network probes.
pub fn run_network_probe_bin() -> i32 {
    configure_probe_app_dir("network-probe");
    let mut logger = init_probe_logger();
    network_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only wireless driver API checks.
pub fn run_wd_probe_bin() -> i32 {
    configure_probe_app_dir("wd-probe");
    let mut logger = init_probe_logger();
    wd_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks TinySMB argument guards without opening an SMB session.
pub fn run_tinysmb_probe_bin() -> i32 {
    configure_probe_app_dir("tinysmb-probe");
    let mut logger = init_probe_logger();
    network_probe::probe_smb_invalid_arguments(&mut logger);
    #[cfg(feature = "dolphin-automation")]
    network_probe::probe_smb_fixture(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only debugger/USB Gecko probes.
pub fn run_debugger_probe_bin() -> i32 {
    configure_probe_app_dir("debugger-probe");
    let mut logger = init_probe_logger_without_display_sync();
    debugger_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only the pure Wii network diagnostic mapping checks.
pub fn run_libdb_diagnostics_probe_bin() -> i32 {
    configure_probe_app_dir("libdb-diagnostics-probe");
    let mut logger = init_probe_logger();
    debugger_probe::probe_wii_network_diagnostics(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only storage probes.
pub fn run_storage_probe_bin(destructive_card_format: bool) -> i32 {
    configure_probe_app_dir("storage-probe");
    let mut logger = init_probe_logger();
    storage_probe::run(&mut logger, destructive_card_format);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", feature = "gamecube-card-probe"))]
pub fn run_gamecube_card_probe_bin(destructive_card_format: bool, hotplug: bool) -> ! {
    let startup = ogc_system::init_checked();
    let display = probe_support::ProbeDisplay::init();
    let mut logger = probe_support::ProbeLogger::with_options(
        display,
        probe_support::ProbeLoggerOptions::disabled(),
    );
    logger.record("GameCube system initialization", startup.is_ok());
    if hotplug {
        storage_probe::run_card_hotplug_probe(&mut logger);
    } else {
        storage_probe::run_card_probe(&mut logger, destructive_card_format);
    }
    logger.summary();
    loop {
        probe_support::wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
/// Runs DVD and `/dev/di` checks without USB, SD, or memory-card requirements.
pub fn run_dvd_probe_bin() -> i32 {
    configure_probe_app_dir("dvd-probe");
    let mut logger = init_probe_logger();
    let no_disc = storage_probe::probe_dvd(&mut logger);
    let shutdown = ogc_dvd::shutdown();
    logger.note_i32("dvd shutdown", shutdown);
    logger.record("dvd shutdown", shutdown == 0);
    if no_disc {
        storage_probe::probe_di_no_disc(&mut logger);
    } else {
        storage_probe::probe_di_media(&mut logger);
    }
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs only keyboard probes.
pub fn run_keyboard_probe_bin() -> i32 {
    configure_probe_app_dir("keyboard-probe");
    let mut logger = init_probe_logger();
    keyboard_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks WPAD initialization, no-controller behavior, and shutdown.
pub fn run_wpad_probe_bin() -> i32 {
    configure_probe_app_dir("wpad-probe");
    let mut logger = init_probe_logger();
    wpad_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Checks that a Wii Remote stays connected through the shared input path.
pub fn run_wii_remote_stability_probe_bin() -> i32 {
    configure_probe_app_dir("wii-remote-stability-probe");
    let mut logger = init_probe_logger();
    wii_remote_stability_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs the live controller input monitor.
pub fn run_input_probe_bin() -> i32 {
    configure_probe_app_dir("input-probe");
    let mut logger = init_probe_logger();
    input_probe::monitor(&mut logger);
}

#[cfg(all(target_arch = "powerpc", feature = "ui-input-probe"))]
/// Runs the live controller-to-widget UI monitor.
pub fn run_ui_input_probe_bin() -> i32 {
    configure_probe_app_dir("ui-input-probe");
    let mut logger = init_probe_logger();
    ui_input_probe::monitor(&mut logger)
}

#[cfg(all(target_arch = "powerpc", not(feature = "gamecube-card-probe")))]
/// Runs the live HID probe.
pub fn run_hid_probe_bin() -> i32 {
    configure_probe_app_dir("hid-probe");
    let mut logger = init_probe_logger();
    hid_probe::monitor(&mut logger);
}

#[cfg(target_arch = "powerpc")]
/// Runs the IPC async completion probe.
pub fn run_ipc_async_probe_bin() -> i32 {
    configure_probe_app_dir("ipc-async-probe");
    let mut logger = init_probe_logger();
    ipc_async_probe::monitor(&mut logger);
}

#[cfg(target_arch = "powerpc")]
/// Starts a system menu handoff for an external Dolphin observer.
pub fn run_ipc_relaunch_probe_bin() -> i32 {
    use ogc_es_sys as es;
    use ogc_ipc as ipc;
    #[repr(align(32))]
    struct Aligned<T>(T);

    let title_id = option_env!("LIBOGC_DOLPHIN_RELAUNCH_TITLE")
        .map_or(Ok(0x0000_0001_0000_0002), |id| u64::from_str_radix(id, 16))
        .expect("invalid Dolphin relaunch title ID");
    configure_probe_app_dir("ipc-relaunch-probe");
    let mut logger = init_probe_logger();
    if cfg!(any(
        feature = "ipc-relaunch-wiilaunch",
        feature = "ipc-relaunch-wiilaunch-legacy",
        feature = "ipc-relaunch-wiilaunch-args",
        feature = "ipc-relaunch-wiilaunch-args-legacy"
    )) {
        let legacy = cfg!(any(
            feature = "ipc-relaunch-wiilaunch-legacy",
            feature = "ipc-relaunch-wiilaunch-args-legacy"
        ));
        let with_args = cfg!(any(
            feature = "ipc-relaunch-wiilaunch-args",
            feature = "ipc-relaunch-wiilaunch-args-legacy"
        ));
        let api = if with_args && legacy {
            "launch_title_with_args"
        } else if with_args {
            "try_launch_title_with_args"
        } else if legacy {
            "launch_title"
        } else {
            "try_launch_title"
        };
        let mode = "foreground";
        let initialized = ogc_wiilaunch::initialize();
        logger.note_i32("Wii launch initialize", initialized);
        if initialized < 0 {
            ogc_system::report(format_args!("IPC_RELAUNCH returned={initialized}\n"));
            logger.record("Wii launch initializes before title handoff", false);
            logger.summary();
            return finish_probe();
        }
        logger.record("Wii launch initializes before title handoff", true);
        let rejected_error = match ogc_wiilaunch::try_launch_title(0) {
            Err(ogc_wiilaunch::LaunchError::Es(error)) if error < 0 => error,
            _ => 0,
        };
        let rejected = if legacy {
            rejected_error < 0 && ogc_wiilaunch::launch_title(0) == rejected_error
        } else {
            rejected_error < 0
        };
        logger.record("Wii launch rejects unknown title before handoff", rejected);
        if !rejected {
            logger.summary();
            return finish_probe();
        }
        logger.note_i32("Wii launch unknown title IOS error", rejected_error);
        ogc_system::report(format_args!(
            "IPC_RELAUNCH source api={api} mode={mode} title={title_id:016x}\n"
        ));
        let result = if with_args && legacy {
            ogc_wiilaunch::launch_title_with_args(title_id, 0x4a2, &[c"parity-probe", c"value=42"])
        } else if with_args {
            ogc_wiilaunch::try_launch_title_with_args(
                title_id,
                0x4a2,
                &[c"parity-probe", c"value=42"],
            )
            .map_or_else(|error| error.raw_code(), |_| 0)
        } else if legacy {
            ogc_wiilaunch::launch_title(title_id)
        } else {
            ogc_wiilaunch::try_launch_title(title_id).map_or_else(|error| error.raw_code(), |_| 0)
        };
        ogc_system::report(format_args!("IPC_RELAUNCH returned={result}\n"));
        logger.note_i32("Wii launch title request returned", result);
        logger.record("Wii launch title request accepted", result == 0);
        logger.summary();
        return finish_probe();
    }
    let initialized = es::init();
    let mut view = Aligned(es::Tikview::default());
    let view_result = if initialized >= 0 {
        unsafe { es::get_ticket_views(title_id, &mut view.0, 1) }
    } else {
        initialized
    };
    let valid = view_result == 0 && view.0.titleid == title_id;
    logger.record("IPC relaunch destination ticket view", valid);
    if !valid {
        logger.summary();
        return finish_probe();
    }
    let fd = match ipc::open_file(c"/dev/es", ipc::IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(error) => {
            logger.note_i32("IPC relaunch open error", error);
            logger.record("IPC relaunch opens ES", false);
            logger.summary();
            return finish_probe();
        }
    };
    #[cfg(not(feature = "ipc-relaunch-rust"))]
    let mut title = Aligned(title_id);
    #[cfg(not(feature = "ipc-relaunch-rust"))]
    let mut vectors = Aligned([
        ipc::Ioctlv {
            data: (&mut title.0 as *mut u64).cast(),
            len: 8,
        },
        ipc::Ioctlv {
            data: (&mut view.0 as *mut es::Tikview).cast(),
            len: core::mem::size_of::<es::Tikview>() as u32,
        },
    ]);
    #[cfg(feature = "ipc-relaunch-background")]
    logger.note("IPC relaunch mode=background");
    #[cfg(not(feature = "ipc-relaunch-background"))]
    logger.note("IPC relaunch mode=foreground");
    let api = if cfg!(feature = "ipc-relaunch-rust") {
        "rust"
    } else {
        "c"
    };
    let mode = if cfg!(feature = "ipc-relaunch-background") {
        "background"
    } else {
        "foreground"
    };
    ogc_system::report(format_args!(
        "IPC_RELAUNCH source api={api} mode={mode} title={title_id:016x}\n"
    ));
    logger.note("IPC relaunch handoff begins");
    #[cfg(feature = "ipc-relaunch-rust")]
    let result = {
        let title = Aligned(title_id.to_be_bytes());
        // The ticket view is initialized and remains live through submission.
        let ticket = unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(&view.0).cast::<u8>(),
                core::mem::size_of::<es::Tikview>(),
            )
        };
        let entries = [ipc::IoctlvEntry::In(&title.0), ipc::IoctlvEntry::In(ticket)];
        if cfg!(feature = "ipc-relaunch-background") {
            unsafe { fd.ioctlv_reboot_background(8, 2, entries) }
        } else {
            fd.ioctlv_reboot(8, 2, entries)
        }
    };
    #[cfg(all(
        not(feature = "ipc-relaunch-rust"),
        not(feature = "ipc-relaunch-background")
    ))]
    let result = unsafe { ipc::IOS_IoctlvReboot(fd.raw(), 8, 2, 0, vectors.0.as_mut_ptr()) };
    #[cfg(all(
        not(feature = "ipc-relaunch-rust"),
        feature = "ipc-relaunch-background"
    ))]
    let result =
        unsafe { ipc::IOS_IoctlvRebootBackground(fd.raw(), 8, 2, 0, vectors.0.as_mut_ptr()) };
    ogc_system::report(format_args!("IPC_RELAUNCH returned={result}\n"));
    logger.note_i32("IPC relaunch returned", result);
    logger.record("IPC relaunch request accepted", result == 0);
    if result == 0 {
        // IOS may still consume the vectors after acknowledging the request.
        // Keep this frame alive until the launched title replaces the probe.
        loop {
            core::hint::spin_loop();
        }
    }
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", feature = "ios-reload-probe"))]
static IOS_RELOAD_RESET_MSR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(u32::MAX);

#[cfg(all(target_arch = "powerpc", feature = "ios-reload-probe"))]
unsafe fn ios_reload_ipc_debug_hook(stage: u32, _: u32, _: u32, _: u32) {
    if stage == 70 {
        let msr: u32;
        // SAFETY: Reading MSR does not change processor state.
        unsafe { core::arch::asm!("mfmsr {value}", value = out(reg) msr, options(nomem, nostack)) };
        IOS_RELOAD_RESET_MSR.store(msr, core::sync::atomic::Ordering::Release);
    }
}

#[cfg(all(target_arch = "powerpc", feature = "ios-reload-probe"))]
fn es_inventory_invalid_results() -> [i32; 16] {
    let null = core::ptr::null_mut();
    // SAFETY: All pointers are null and must be rejected before dereference.
    unsafe {
        [
            ogc_es_sys::get_num_owned_titles(null),
            ogc_es_sys::get_owned_titles(core::ptr::null_mut(), 0),
            ogc_es_sys::get_num_titles(null),
            ogc_es_sys::get_titles(core::ptr::null_mut(), 0),
            ogc_es_sys::get_num_stored_tmd_contents(core::ptr::null(), 0, null),
            ogc_es_sys::get_stored_tmd_contents(core::ptr::null(), 0, null, 0),
            ogc_es_sys::get_stored_tmd_size(0, null),
            ogc_es_sys::get_stored_tmd(0, core::ptr::null_mut(), 0),
            ogc_es_sys::get_title_contents_count(0, null),
            ogc_es_sys::get_title_contents(0, null, 0),
            ogc_es_sys::get_tmd_view_size(0, null),
            ogc_es_sys::get_tmd_view(0, core::ptr::null_mut(), 0),
            ogc_es_sys::get_consumption_count(0, null),
            ogc_es_sys::get_consumption(0, core::ptr::null_mut(), 0),
            ogc_es_sys::get_num_shared_contents(null),
            ogc_es_sys::get_shared_contents(core::ptr::null_mut(), 0),
        ]
    }
}

#[cfg(all(target_arch = "powerpc", feature = "ios-reload-probe"))]
fn es_operations_invalid_results() -> [i32; 24] {
    use ogc_es_sys as es;
    let p = core::ptr::null();
    let m = core::ptr::null_mut();
    // SAFETY: These inputs must be rejected before access or IOS submission.
    unsafe {
        [
            es::get_data_dir(0, m),
            es::di_get_tmd_view_size(p, 0, m.cast()),
            es::di_get_tmd_view(p, 0, m.cast(), 0),
            es::get_ticket_view(p, m.cast()),
            es::identify(p, 0, p, 0, p, 0, m.cast()),
            es::verify_with_ticket_view(p, 0, p, 0, p.cast(), m.cast()),
            es::add_ticket(p, 0, p, 0, p, 0),
            es::delete_ticket(p.cast()),
            es::export_title_init(0, m.cast(), 0),
            es::export_content_data(-1, m, 0),
            es::reimport_title_init(p, 0),
            es::add_title_start(p, 0, p, 0, p, 0),
            es::add_content_data(-1, p.cast(), 0),
            es::read_content(-1, m, 0),
            es::seek_content(-1, 0, 0),
            es::close_content(-1),
            es::encrypt(0, m.cast(), p.cast(), 0, m),
            es::decrypt(0, m.cast(), p.cast(), 0, m),
            es::sign(p.cast(), 0, m, m.cast()),
            es::verify_sign(p.cast(), 0, p.cast(), p, 0),
            es::get_device_cert(m),
            es::get_device_id(m.cast()),
            es::get_boot2_version(m.cast()),
            es::import_boot(es::ImportBootParams {
                tik: p,
                tik_size: 0,
                tik_certs: p,
                tik_certs_size: 0,
                tmd: p,
                tmd_size: 0,
                tmd_certs: p,
                tmd_certs_size: 0,
                content: p.cast(),
                content_size: 0,
            }),
        ]
    }
}

#[cfg(all(
    target_arch = "powerpc",
    feature = "ios-reload-probe",
    feature = "dolphin-automation"
))]
mod ios_reload_pending;

#[cfg(all(target_arch = "powerpc", feature = "ios-reload-probe"))]
/// Reloads the current IOS and checks IPC after it returns.
pub fn run_ios_reload_probe_bin() -> i32 {
    configure_probe_app_dir("ios-reload-probe");
    let mut logger = init_probe_logger();
    let version = ogc_ios::get_version();
    logger.note_i32("IOS version before reload", version);
    if !(3..=255).contains(&version) {
        logger.record("current IOS version is valid", false);
        logger.summary();
        return finish_probe();
    }
    logger.record("current IOS version is valid", true);
    let shutdown = ogc_ios::shutdown_subsystems();
    logger.note_i32("IOS subsystem shutdown result", shutdown);
    logger.record("IOS subsystem shutdown succeeds", shutdown >= 0);
    if shutdown < 0 {
        logger.summary();
        return finish_probe();
    }
    let initialized = ogc_ios::initialize_subsystems();
    logger.note_i32("IOS subsystem initialization result", initialized);
    logger.record("IOS subsystem initialization succeeds", initialized >= 0);
    if initialized < 0 {
        logger.summary();
        return finish_probe();
    }
    let startup_ios = ogc_ios::load_startup_ios();
    logger.note_i32("IOS startup loader result", startup_ios);
    logger.record(
        "disabled C startup loader returns success without launching IOS",
        startup_ios == 0,
    );
    let invalid_launch = ogc_ios::launch_new_ios(2);
    logger.note_i32("invalid IOS launch result", invalid_launch);
    logger.record(
        "IOS launch rejects version below supported range",
        invalid_launch == ogc_ios::IOS_EBADVERSION,
    );
    let closed = ogc_es_sys::close();
    logger.record("ES closes before argument checks", closed == 0);
    if closed != 0 {
        logger.summary();
        return finish_probe();
    }
    // SAFETY: These invalid inputs are checked before any dereference.
    let closed_results = unsafe {
        [
            ogc_es_sys::get_num_ticket_views(0, core::ptr::null_mut()),
            ogc_es_sys::get_ticket_views(0, core::ptr::null_mut(), 0),
            ogc_es_sys::launch_title(0, core::ptr::null()),
            ogc_es_sys::launch_title_background(0, core::ptr::null()),
        ]
    };
    logger.record(
        "closed ES reports not initialized before invalid arguments",
        closed_results
            .iter()
            .all(|&rc| rc == ogc_es_sys::ES_ENOTINIT),
    );
    logger.record(
        "closed ES inventory calls report not initialized first",
        es_inventory_invalid_results()
            .iter()
            .all(|&rc| rc == ogc_es_sys::ES_ENOTINIT),
    );
    logger.record(
        "closed ES operation calls report not initialized first",
        es_operations_invalid_results()
            .iter()
            .all(|&rc| rc == ogc_es_sys::ES_ENOTINIT),
    );
    let initialized = ogc_es_sys::init();
    logger.record("ES reopens after argument checks", initialized == 0);
    if initialized != 0 {
        logger.summary();
        return finish_probe();
    }
    // SAFETY: These invalid inputs are checked before any dereference.
    let open_results = unsafe {
        [
            ogc_es_sys::get_num_ticket_views(0, core::ptr::null_mut()),
            ogc_es_sys::get_ticket_views(0, core::ptr::null_mut(), 0),
            ogc_es_sys::launch_title(0, core::ptr::null()),
            ogc_es_sys::launch_title_background(0, core::ptr::null()),
        ]
    };
    logger.record(
        "open ES rejects invalid arguments",
        open_results.iter().all(|&rc| rc == ogc_es_sys::ES_EINVAL),
    );
    logger.record(
        "open ES inventory calls reject invalid arguments",
        es_inventory_invalid_results()
            .iter()
            .all(|&rc| rc == ogc_es_sys::ES_EINVAL),
    );
    logger.record(
        "open ES operation calls reject invalid arguments",
        es_operations_invalid_results()
            .iter()
            .all(|&rc| rc == ogc_es_sys::ES_EINVAL),
    );
    let mut limits_count = 0xa5a5_a5a5;
    // SAFETY: The output points to a writable u32.
    let consumption_rc = unsafe { ogc_es_sys::get_consumption_count(u64::MAX, &mut limits_count) };
    logger.note_i32("ES consumption result", consumption_rc);
    logger.note_u32("ES consumption output", limits_count);
    logger.record(
        "ES consumption query returns a count",
        consumption_rc >= 0 && limits_count == 0,
    );
    IOS_RELOAD_RESET_MSR.store(u32::MAX, core::sync::atomic::Ordering::Release);
    ogc_ipc::set_debug_hook(Some(ios_reload_ipc_debug_hook));
    logger.note("IOS reload begins");
    let result = ogc_ios::reload_ios(version);
    ogc_ipc::set_debug_hook(None);
    logger.note_i32("IOS reload result", result);
    logger.record("IOS reload succeeds", result == 0);
    let reset_msr = IOS_RELOAD_RESET_MSR.load(core::sync::atomic::Ordering::Acquire);
    logger.note_u32("IOS reload IPC reset MSR", reset_msr);
    logger.record(
        "IOS reload masks interrupts during IPC reset",
        reset_msr & 0x8000 == 0,
    );
    logger.record(
        "IOS version is preserved",
        ogc_ios::get_version() == version,
    );
    let ipc_ok = match ogc_ipc::open_file(c"/dev/fs", ogc_ipc::IPC_OPEN_NONE) {
        Ok(fd) => fd.close().is_ok(),
        Err(_) => false,
    };
    logger.record("IPC opens and closes FS after reload", ipc_ok);
    #[cfg(feature = "dolphin-automation")]
    if result == 0 && ipc_ok {
        ios_reload_pending::run(&mut logger, version);
    }
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Tests Bluetooth startup, pairing, buttons, axes, and adapter shutdown.
pub fn run_bluetooth_probe_bin() -> i32 {
    configure_probe_app_dir("bluetooth-probe");
    let mut logger = init_probe_logger();
    bluetooth_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(all(target_arch = "powerpc", feature = "dolphin-automation"))]
pub fn run_bt_dispatch_probe_bin() -> i32 {
    configure_probe_app_dir("bt-dispatch-probe");
    let mut logger = init_probe_logger();
    logger.record(
        "lwbt dispatcher reserves distinct setup slots and routes peer events",
        lwbt_sys::dolphin_disconnect_response_order_probe(),
    );
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
/// Runs the Bluetooth controller synchronization menu.
pub fn run_bt_sync_menu_bin() -> i32 {
    configure_probe_app_dir("bt-sync-menu");
    let mut logger = init_probe_logger();
    bt_sync_menu::run(&mut logger)
}
