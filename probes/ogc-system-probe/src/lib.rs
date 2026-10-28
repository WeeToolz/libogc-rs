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
pub mod arena_probe;
#[cfg(target_arch = "powerpc")]
pub mod bluetooth_probe;
#[cfg(target_arch = "powerpc")]
pub mod bt_sync_menu;
#[cfg(target_arch = "powerpc")]
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
pub mod font_probe;
#[cfg(target_arch = "powerpc")]
pub mod general_probe;
#[cfg(target_arch = "powerpc")]
mod hid_descriptor_probe;
#[cfg(target_arch = "powerpc")]
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
#[cfg(target_arch = "powerpc")]
pub mod runtime_probe;
#[cfg(target_arch = "powerpc")]
pub mod sound_probe;
#[cfg(target_arch = "powerpc")]
pub mod sram_probe;
#[cfg(target_arch = "powerpc")]
pub mod startup_probe;
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
#[cfg(target_arch = "powerpc")]
mod usbstorage_transport_probe;
#[cfg(target_arch = "powerpc")]
pub mod video_probe;
#[cfg(target_arch = "powerpc")]
pub mod wii_remote_stability_probe;
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
    init_probe_logger_with_page_reports(false)
}

#[cfg(target_arch = "powerpc")]
fn init_probe_logger_with_page_reports(page_reports: bool) -> probe_support::ProbeLogger {
    use ogc_system::{self as system};

    static IPC_PROGRESS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

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
    display.wait_vsync();
    display.println("BOOT 2/3: entering IOS pre-main");
    display.wait_vsync();
    probe_support::OGC_PROBE_BOOT_STAGE.store(2, core::sync::atomic::Ordering::Release);
    ogc_ipc::set_debug_hook(Some(ipc_progress));
    system::sys::set_pre_main_progress_callback(Some(pre_main_progress));
    system::pre_main();
    system::sys::set_pre_main_progress_callback(None);
    ogc_ipc::set_debug_hook(None);
    probe_support::OGC_PROBE_BOOT_STAGE.store(3, core::sync::atomic::Ordering::Release);
    probe_support::send_agent_progress(3);
    if page_reports {
        display.enable_page_reports();
    }
    display.println("BOOT 3/3: IOS pre-main completed");
    if page_reports {
        display.capture_page();
    }
    display.wait_vsync();

    system::stdio_report(true);
    let mount_status = maybe_mount_external_logs();

    let mut logger = probe_support::ProbeLogger::with_options(display, log_options());
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
    logger.note("stm init deferred until shutdown");
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

#[cfg(all(target_arch = "powerpc", not(feature = "agent-report")))]
fn idle_forever() -> ! {
    loop {
        probe_support::wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
fn finish_probe() -> i32 {
    #[cfg(not(feature = "agent-report"))]
    idle_forever();
    #[cfg(feature = "agent-report")]
    {
        0
    }
}

#[cfg(target_arch = "powerpc")]
/// Runs the full hardware probe suite, then enters the live input monitor.
pub fn run_system_probe() -> i32 {
    configure_probe_app_dir("ogc-system-probe");
    let mut logger = init_probe_logger();

    general_probe::run(&mut logger);
    startup_probe::run(&mut logger);
    sram_probe::run(&mut logger);
    arena_probe::run(&mut logger);
    callback_probe::run(&mut logger);
    runtime_probe::run(&mut logger);
    font_probe::run(&mut logger);
    bus_probe::run(&mut logger);
    video_probe::run(&mut logger);
    sound_probe::run(&mut logger);
    network_probe::run(&mut logger);
    storage_probe::run(&mut logger, false);
    keyboard_probe::run(&mut logger);

    logger.summary();
    #[cfg(feature = "agent-report")]
    return finish_probe();
    #[cfg(not(feature = "agent-report"))]
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

#[cfg(target_arch = "powerpc")]
/// Runs only bus-related probes.
pub fn run_bus_probe_bin() -> i32 {
    configure_probe_app_dir("bus-probe");
    let mut logger = init_probe_logger();
    bus_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
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

#[cfg(target_arch = "powerpc")]
/// Runs only video probes.
pub fn run_video_probe_bin() -> i32 {
    configure_probe_app_dir("video-probe");
    let mut logger = init_probe_logger();
    video_probe::run(&mut logger);
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

#[cfg(target_arch = "powerpc")]
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
    let mut logger = init_probe_logger_with_page_reports(true);
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
/// Checks that ModPlay mixes in its LWP worker and stops that worker cleanly.
pub fn run_modplay_probe_bin() -> i32 {
    configure_probe_app_dir("modplay-probe");
    let mut logger = init_probe_logger();
    modplay_probe::run(&mut logger);
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
/// Checks two USB media through the real FAT and USB APIs without file writes.
pub fn run_usb_volumes_probe_bin() -> i32 {
    configure_probe_app_dir("usb-volumes-probe");
    let mut logger = init_probe_logger();
    usb_volumes_probe::run(&mut logger);
    logger.summary();
    finish_probe()
}

#[cfg(target_arch = "powerpc")]
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
/// Runs only debugger/USB Gecko probes.
pub fn run_debugger_probe_bin() -> i32 {
    configure_probe_app_dir("debugger-probe");
    let mut logger = init_probe_logger();
    debugger_probe::run(&mut logger);
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

#[cfg(target_arch = "powerpc")]
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
/// Tests Bluetooth startup, pairing, buttons, axes, and adapter shutdown.
pub fn run_bluetooth_probe_bin() -> i32 {
    configure_probe_app_dir("bluetooth-probe");
    let mut logger = init_probe_logger();
    bluetooth_probe::run(&mut logger);
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
