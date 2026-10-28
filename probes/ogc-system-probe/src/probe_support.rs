#[cfg(test)]
use alloc::vec::Vec;
#[path = "activity_indicator.rs"]
mod activity_indicator;
#[cfg(any(feature = "agent-report", feature = "dolphin-automation"))]
#[path = "framebuffer_copy.rs"]
mod framebuffer_copy;
#[cfg(feature = "agent-report")]
#[path = "framebuffer_selection.rs"]
mod framebuffer_selection;

use core::cell::Cell;
use core::ffi::{CStr, c_void};
use core::fmt::{self, Write};
use core::ptr::null_mut;
#[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
use core::sync::atomic::AtomicI32;
#[cfg(feature = "agent-report")]
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ogc_filesystem as filesystem;
use ogc_stm as stm;
use ogc_sysbase as sysbase;
#[cfg(any(feature = "agent-report", feature = "mounted-log"))]
use ogc_system::sys;
use ogc_system::{self as system};
use ogc_video as video;

#[cfg(feature = "agent-report")]
#[path = "input_identity.rs"]
mod input_identity;
#[cfg(feature = "agent-report")]
#[path = "report_exit.rs"]
mod report_exit;

#[cfg(feature = "mounted-log")]
const LOG_FILE_PREFIX: &[u8] = b"libogc-";
#[cfg(feature = "mounted-log")]
const LOG_FILE_SUFFIX: &[u8] = b".log";
const ISFS_FALLBACK_LOG_PATH: &[u8] = b"/tmp/ogc-system-probe.log\0";
const LINE_BUF_CAPACITY: usize = 256;
#[cfg(feature = "mounted-log")]
const FILE_PATH_CAPACITY: usize = 128;
const PROBE_BG: u32 = 0x1080_1080;
#[cfg(feature = "dolphin-automation")]
const DOLPHIN_PAGE_HOLD_FRAMES: u32 = 60;
const MOUNTED_LOG_WRITE_LIMIT: usize = 256 * 1024;
#[cfg(feature = "agent-report")]
const AGENT_SCREEN_CAPTURE_LIMIT: usize = 128 * 1024;
#[cfg(feature = "agent-report")]
const AGENT_ARGUMENT_COUNT_LIMIT: usize = 64;
#[cfg(feature = "agent-report")]
const AGENT_ARGUMENT_BYTES_LIMIT: usize = 4096;
const SAFE_MARGIN_X: i32 = 32;
const SAFE_MARGIN_Y: i32 = 32;

#[path = "log_state.rs"]
mod log_state;
use log_state::{LogError, LogState, finish_summary};
#[path = "failure_summary.rs"]
mod failure_summary;
use failure_summary::{FailureSummary, PAGE_SIZE};

static VSYNC_TIMEOUTS: AtomicU32 = AtomicU32::new(0);
static POWER_REQUESTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "dolphin-automation")]
static DOLPHIN_TRANSPORT_ERROR: AtomicI32 = AtomicI32::new(0);
#[cfg(feature = "agent-report")]
static AGENT_SCREEN_CAPTURE_LEN: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "agent-report")]
static AGENT_SCREEN_CAPTURE_LOCK: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "agent-report")]
static AGENT_SCREEN_CAPTURE_TRUNCATED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "agent-report")]
static mut AGENT_SCREEN_CAPTURE: [u8; AGENT_SCREEN_CAPTURE_LIMIT] = [0; AGENT_SCREEN_CAPTURE_LIMIT];
#[cfg(feature = "agent-report")]
static AGENT_PROGRESS_CURSOR: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "agent-report")]
static AGENT_PROGRESS_SEQUENCE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "agent-report")]
static AGENT_REPORT_NETWORK_READY: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "agent-report")]
static AGENT_REPORT_SOCKET: AtomicI32 = AtomicI32::new(-1);
#[cfg(feature = "agent-report")]
static WATCHDOG_LAST_HEARTBEAT_MS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "agent-report")]
static WATCHDOG_TIMEOUT_MS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "agent-report")]
const RESET_PROGRESS_RECORDS: usize = 24;
#[cfg(feature = "agent-report")]
#[unsafe(no_mangle)]
pub static OGC_PROBE_RESET_PROGRESS_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "agent-report")]
#[unsafe(no_mangle)]
pub static OGC_PROBE_RESET_PROGRESS: [AtomicU32; RESET_PROGRESS_RECORDS * 3] =
    [const { AtomicU32::new(0) }; RESET_PROGRESS_RECORDS * 3];

/// Exported early-boot breadcrumb for hardware debuggers and emulator memory
/// inspection. Values 1 through 3 match the visible `BOOT n/3` markers;
/// `u32::MAX` indicates the fixed panic path was entered.
#[unsafe(no_mangle)]
pub static OGC_PROBE_BOOT_STAGE: AtomicU32 = AtomicU32::new(0);
static mut PROBE_FRAMEBUFFER_A: *mut c_void = null_mut();
static mut PROBE_FRAMEBUFFER_B: *mut c_void = null_mut();
static mut PROBE_APP_DIR: Option<&'static str> = None;

/// Logger that mirrors probe output to the console and, when possible, ISFS.
///
/// Safe default:
/// - `ProbeLogger::new(...)` uses ISFS only.
///
/// Expert path:
/// - `ProbeLogger::with_options(...)` can opt into mounted-device logging when
///   the `mounted-log` feature is enabled.
pub struct ProbeLogger {
    file: Option<LogFile>,
    log_state: LogState,
    file_policy: LogFilePolicy,
    file_bytes_written: usize,
    display: ProbeDisplay,
    passes: usize,
    fails: usize,
    failures: FailureSummary,
    hardware_summary: Option<[LineBuf; 3]>,
    step_count: u32,
    last_step_frame: u32,
    #[cfg(feature = "agent-report")]
    started_ticks: u64,
    #[cfg(feature = "agent-report")]
    warmup_frames: u32,
    #[cfg(feature = "agent-report")]
    sample_frames: u32,
    #[cfg(feature = "agent-report")]
    recovery_alarm: Option<system::SysWd>,
}

#[cfg(feature = "agent-report")]
#[derive(Copy, Clone, Default)]
struct AgentPerformance {
    elapsed_ticks: u64,
    elapsed_us: u64,
    frame_time_average_ticks: u64,
    frame_time_min_ticks: u64,
    frame_time_max_ticks: u64,
    frame_count: u32,
    heap_used_bytes: u32,
    warmup_frames: u32,
    sample_frames: u32,
    frame_time_average_us: u32,
    frame_time_min_us: u32,
    frame_time_max_us: u32,
}

#[cfg(feature = "agent-report")]
static mut RECOVERY_ALARM: system::SysWd = 0;
#[cfg(feature = "agent-report")]
static AGENT_ARGC: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "agent-report")]
static AGENT_USE_SYSTEM_ARGUMENTS: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "agent-report")]
#[derive(Copy, Clone, Default)]
struct AgentArgumentRange {
    start: usize,
    length: usize,
}

#[cfg(feature = "agent-report")]
struct AgentArgumentStorage {
    bytes: [u8; AGENT_ARGUMENT_BYTES_LIMIT],
    ranges: [AgentArgumentRange; AGENT_ARGUMENT_COUNT_LIMIT],
}

#[cfg(feature = "agent-report")]
impl AgentArgumentStorage {
    const fn empty() -> Self {
        Self {
            bytes: [0; AGENT_ARGUMENT_BYTES_LIMIT],
            ranges: [AgentArgumentRange {
                start: 0,
                length: 0,
            }; AGENT_ARGUMENT_COUNT_LIMIT],
        }
    }
}

#[cfg(feature = "agent-report")]
static mut AGENT_ARGUMENT_STORAGE: AgentArgumentStorage = AgentArgumentStorage::empty();
#[cfg(feature = "agent-report")]
static mut SYSTEM_ARGUMENT_STORAGE: AgentArgumentStorage = AgentArgumentStorage::empty();

/// Stores the argument vector passed to the DOL entry point for agent reports.
///
/// # Safety
/// `argv` must point to at least `argc` readable pointer entries, and every
/// non-null entry must point to a readable NUL-terminated byte string. `argc`
/// must describe the live argument vector supplied to this DOL entry point.
pub unsafe fn set_agent_arguments(argc: i32, argv: *mut *mut u8) {
    #[cfg(feature = "agent-report")]
    {
        // SAFETY: the caller provides the live DOL entrypoint argument vector;
        // `copy_agent_arguments` checks argc and capacity before traversing it.
        let copied = unsafe {
            copy_agent_arguments(core::ptr::addr_of_mut!(AGENT_ARGUMENT_STORAGE), argc, argv)
        };
        AGENT_USE_SYSTEM_ARGUMENTS.store(false, Ordering::Release);
        AGENT_ARGC.store(copied.unwrap_or(0), Ordering::Release);
    }
    #[cfg(not(feature = "agent-report"))]
    let _ = (argc, argv);
}

/// Refreshes the agent argument vector after `system::check_argv` builds it.
pub fn capture_system_agent_arguments() {
    #[cfg(feature = "agent-report")]
    unsafe {
        if let Some(argv) = sys::current_system_argv().as_ref() {
            let copied = copy_agent_arguments(
                core::ptr::addr_of_mut!(SYSTEM_ARGUMENT_STORAGE),
                argv.argc,
                argv.argv.cast(),
            );
            if let Some(count) = copied {
                let storage = core::ptr::addr_of!(SYSTEM_ARGUMENT_STORAGE);
                let has_report_identity = storage_contains_argument(storage, count, b"--report=")
                    && storage_contains_argument(storage, count, b"--run-id=")
                    && storage_contains_argument(storage, count, b"--build-id=");
                if has_report_identity || AGENT_ARGC.load(Ordering::Acquire) == 0 {
                    AGENT_USE_SYSTEM_ARGUMENTS.store(true, Ordering::Release);
                    AGENT_ARGC.store(count, Ordering::Release);
                }
            }
        }
    }
}

#[cfg(feature = "agent-report")]
pub fn agent_argument_count() -> usize {
    AGENT_ARGC.load(Ordering::Acquire)
}

#[cfg(feature = "agent-report")]
fn agent_arguments_from_system() -> bool {
    AGENT_USE_SYSTEM_ARGUMENTS.load(Ordering::Acquire)
}

#[cfg(not(feature = "agent-report"))]
pub fn agent_argument_count() -> usize {
    0
}

#[cfg(not(feature = "agent-report"))]
fn agent_arguments_from_system() -> bool {
    false
}

/// Writes probe text to the console and captures the exact bytes for the
/// automated run report. The fixed buffer keeps early boot and callback paths
/// allocation-free.
pub fn screen_write(bytes: &[u8]) -> Result<usize, i32> {
    #[cfg(feature = "agent-report")]
    let captured = capture_screen_bytes(bytes);
    #[cfg(feature = "dolphin-automation")]
    dolphin_transport_write(bytes);
    let written = video::console_write(bytes);
    #[cfg(feature = "agent-report")]
    if !captured && written.is_ok() {
        return Err(-2);
    }
    written
}

/// Adds application text to the next Wiiload progress record without writing
/// it over an application's own framebuffer renderer.
pub fn agent_log(bytes: &[u8]) {
    #[cfg(feature = "agent-report")]
    let _ = capture_screen_bytes(bytes);
    #[cfg(not(feature = "agent-report"))]
    let _ = bytes;
}

/// Checked append to the agent report capture buffer.
///
/// Success means that all bytes were appended to the local capture buffer. It
/// does not mean that a network report was delivered.
#[cfg(feature = "agent-report")]
pub fn agent_log_checked(bytes: &[u8]) -> Result<(), AgentLogError> {
    append_agent_capture(bytes, CapturePolicy::WholeRecord).map(|_| ())
}

#[cfg(feature = "agent-report")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentLogError {
    CaptureFull,
    CaptureBusy,
}

/// Starts a periodic watchdog when the DOL receives
/// `--watchdog-timeout-ms=N` or Wiiload's `--recovery-reset-ms=N` argument. The
/// watchdog resets the Wii only when the application stops refreshing its
/// heartbeat. A timeout from 2 seconds to 24 days is supported.
pub fn start_agent_watchdog() -> bool {
    #[cfg(feature = "agent-report")]
    {
        let Some(timeout_ms) = agent_frame_option(b"--watchdog-timeout-ms=")
            .or_else(recovery_reset_delay_ms)
            .filter(|milliseconds| (2_000..=i32::MAX as u32).contains(milliseconds))
        else {
            return false;
        };
        let mut alarm: system::SysWd = 0;
        if system::create_alarm(&mut alarm) != 0 || alarm == 0 {
            return false;
        }
        let period = system::TimeSpec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        let now_ms = watchdog_now_ms();
        WATCHDOG_TIMEOUT_MS.store(timeout_ms, Ordering::Release);
        WATCHDOG_LAST_HEARTBEAT_MS.store(now_ms, Ordering::Release);
        // SAFETY: The callback reads only atomics and requests a documented
        // system restart. It does not use the callback argument.
        let result = unsafe {
            system::set_periodic_alarm(
                alarm,
                &period,
                &period,
                Some(watchdog_alarm_callback),
                core::ptr::null_mut(),
            )
        };
        if result != 0 {
            let _ = system::remove_alarm(alarm);
            WATCHDOG_TIMEOUT_MS.store(0, Ordering::Release);
            return false;
        }
        true
    }
    #[cfg(not(feature = "agent-report"))]
    {
        false
    }
}

/// Reports whether this DOL received a watchdog timeout argument.
pub fn agent_watchdog_requested() -> bool {
    #[cfg(feature = "agent-report")]
    {
        agent_frame_option(b"--watchdog-timeout-ms=").is_some()
            || recovery_reset_delay_ms().is_some()
    }
    #[cfg(not(feature = "agent-report"))]
    {
        false
    }
}

/// Refreshes the DOL heartbeat. Call this from its regular main loop.
pub fn heartbeat_agent_watchdog() {
    #[cfg(feature = "agent-report")]
    if WATCHDOG_TIMEOUT_MS.load(Ordering::Acquire) != 0 {
        WATCHDOG_LAST_HEARTBEAT_MS.store(watchdog_now_ms(), Ordering::Release);
    }
}

#[cfg(feature = "agent-report")]
fn watchdog_now_ms() -> u32 {
    agent_clock::timestamp_ms(system::time(), video::timer_clock_hz())
}

#[cfg(feature = "agent-report")]
#[path = "agent_clock.rs"]
mod agent_clock;

#[cfg(feature = "agent-report")]
#[path = "report_send.rs"]
mod report_send;

#[cfg(feature = "agent-report")]
fn capture_screen_bytes(bytes: &[u8]) -> bool {
    append_agent_capture(bytes, CapturePolicy::LegacyPrefix).unwrap_or(false)
}

#[cfg(feature = "agent-report")]
#[derive(Clone, Copy)]
enum CapturePolicy {
    WholeRecord,
    LegacyPrefix,
}

#[cfg(feature = "agent-report")]
fn append_agent_capture(bytes: &[u8], policy: CapturePolicy) -> Result<bool, AgentLogError> {
    let Some(_guard) = AgentCaptureGuard::try_lock() else {
        AGENT_SCREEN_CAPTURE_TRUNCATED.store(true, Ordering::Release);
        return Err(AgentLogError::CaptureBusy);
    };
    let start = AGENT_SCREEN_CAPTURE_LEN.load(Ordering::Relaxed);
    let remaining = AGENT_SCREEN_CAPTURE_LIMIT.saturating_sub(start);
    let count = bytes.len().min(remaining);
    if count < bytes.len() && matches!(policy, CapturePolicy::WholeRecord) {
        AGENT_SCREEN_CAPTURE_TRUNCATED.store(true, Ordering::Release);
        return Err(AgentLogError::CaptureFull);
    }
    // SAFETY: the capture guard serializes all writers. Readers only use the
    // Acquire-published length, which is updated after this append completes.
    let mut appended_len = 0;
    let result = unsafe {
        let destination = core::ptr::addr_of_mut!(AGENT_SCREEN_CAPTURE)
            .cast::<u8>()
            .add(start);
        let tail = core::slice::from_raw_parts_mut(destination, remaining);
        crate::capture_append::append(tail, &mut appended_len, &bytes[..count])
    };
    if result.is_err() {
        AGENT_SCREEN_CAPTURE_TRUNCATED.store(true, Ordering::Release);
        return Err(AgentLogError::CaptureFull);
    }
    let Some(committed_len) = start.checked_add(appended_len) else {
        AGENT_SCREEN_CAPTURE_TRUNCATED.store(true, Ordering::Release);
        return Err(AgentLogError::CaptureFull);
    };
    AGENT_SCREEN_CAPTURE_LEN.store(committed_len, Ordering::Release);
    if count < bytes.len() {
        AGENT_SCREEN_CAPTURE_TRUNCATED.store(true, Ordering::Release);
    }
    Ok(count == bytes.len())
}

#[cfg(feature = "agent-report")]
struct AgentCaptureGuard;

#[cfg(feature = "agent-report")]
impl AgentCaptureGuard {
    fn try_lock() -> Option<Self> {
        AGENT_SCREEN_CAPTURE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self)
    }
}

#[cfg(feature = "agent-report")]
impl Drop for AgentCaptureGuard {
    fn drop(&mut self) {
        AGENT_SCREEN_CAPTURE_LOCK.store(false, Ordering::Release);
    }
}

#[cfg(feature = "agent-report")]
fn captured_screen_text() -> &'static [u8] {
    let len = AGENT_SCREEN_CAPTURE_LEN
        .load(Ordering::Acquire)
        .min(AGENT_SCREEN_CAPTURE_LIMIT);
    // SAFETY: bytes below the published length were initialized by
    // capture_screen_bytes and the fixed buffer remains alive for the run.
    unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(AGENT_SCREEN_CAPTURE).cast::<u8>(), len)
    }
}

/// Console display handle used by probe binaries.
pub struct ProbeDisplay {
    console: *mut video::PrintConsole,
    console_ready: bool,
    framebuffer_bytes: usize,
    #[cfg(feature = "dolphin-automation")]
    framebuffer_address: usize,
    #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
    framebuffer_width: u32,
    #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
    framebuffer_height: u32,
    #[cfg(feature = "dolphin-automation")]
    framebuffer_stride: u32,
    page_index: Cell<u32>,
    captured_pages: Cell<u32>,
    #[cfg(feature = "agent-report")]
    agent_page_reports: Cell<bool>,
    #[cfg(feature = "agent-report")]
    page_report_error: Cell<Option<i32>>,
    #[cfg(feature = "agent-report")]
    initialized_ticks: u64,
    #[cfg(feature = "agent-report")]
    render_mode: *mut video::GxrModeObj,
    #[cfg(feature = "agent-report")]
    active_framebuffer_b: Cell<bool>,
}

enum LogFile {
    Isfs(filesystem::IsfsFile),
    Mounted(MountedLogFile),
}

/// Backend selection for the safe logger abstraction.
///
/// `IsfsOnly` is the conservative default.
/// `PreferMounted` and `MountedOnly` are intended for explicit Wii-side use
/// when external FAT-backed storage has been mounted on purpose.
#[derive(Copy, Clone)]
pub enum LogFilePolicy {
    Disabled,
    IsfsOnly,
    PreferMounted,
    MountedOnly,
}

/// Options for the safe probe logger abstraction.
///
/// This is the supported app-facing entry point. Developers who want direct
/// mounted-device access can still call `fat::mount_*` and
/// `ogc_sysbase::DeviceFile` themselves.
#[derive(Copy, Clone)]
pub struct ProbeLoggerOptions {
    pub file_policy: LogFilePolicy,
}

impl ProbeLoggerOptions {
    pub const fn disabled() -> Self {
        Self {
            file_policy: LogFilePolicy::Disabled,
        }
    }

    pub const fn isfs_only() -> Self {
        Self {
            file_policy: LogFilePolicy::IsfsOnly,
        }
    }

    pub const fn prefer_mounted() -> Self {
        Self {
            file_policy: LogFilePolicy::PreferMounted,
        }
    }

    pub const fn mounted_only() -> Self {
        Self {
            file_policy: LogFilePolicy::MountedOnly,
        }
    }
}

impl Default for ProbeLoggerOptions {
    fn default() -> Self {
        Self::isfs_only()
    }
}

impl LogFile {
    fn write_all(&self, bytes: &[u8]) -> Result<(), i32> {
        match self {
            Self::Isfs(file) => file.write_all(bytes),
            Self::Mounted(file) => file.write_all(bytes),
        }
    }

    fn backend_name(&self) -> &'static str {
        match self {
            Self::Isfs(_) => "ISFS",
            Self::Mounted(_) => "mounted-device",
        }
    }

    fn sync(&self) -> Result<(), i32> {
        match self {
            Self::Isfs(_) => Ok(()),
            Self::Mounted(file) => file.sync(),
        }
    }

    const fn write_limit(&self) -> Option<usize> {
        match self {
            Self::Isfs(_) => None,
            Self::Mounted(_) => Some(MOUNTED_LOG_WRITE_LIMIT),
        }
    }
}

struct MountedLogFile {
    file: sysbase::DeviceFile,
    // A path without a device prefix resolves through the current devoptab, so
    // retain "unknown" and conservatively treat it as USB-backed.
    usb_backed: Option<bool>,
}

impl MountedLogFile {
    #[cfg(feature = "mounted-log")]
    fn open(path: &CStr) -> Option<Self> {
        let path_bytes = path.to_bytes();
        Some(Self {
            file: sysbase::DeviceFile::open_truncate(path)?,
            usb_backed: path_bytes
                .starts_with(b"usb:")
                .then_some(true)
                .or_else(|| path_bytes.starts_with(b"sd:").then_some(false)),
        })
    }

    fn write_all(&self, bytes: &[u8]) -> Result<(), i32> {
        self.file.write_all(bytes)
    }

    fn sync(&self) -> Result<(), i32> {
        self.file.sync()
    }

    fn is_usb_or_unknown(&self) -> bool {
        self.usb_backed.unwrap_or(true)
    }
}

impl ProbeDisplay {
    #[cfg(feature = "agent-report")]
    fn vi_displayed_framebuffer(&self) -> Result<*mut c_void, i32> {
        let current = video::get_current_framebuffer();
        let console_target = if self.console.is_null() {
            null_mut()
        } else {
            // SAFETY: the console remains owned by ogc-video for this display.
            unsafe { (*self.console).destbuffer }
        };
        let (framebuffer_a, framebuffer_b) = unsafe { (PROBE_FRAMEBUFFER_A, PROBE_FRAMEBUFFER_B) };
        let selection = framebuffer_selection::displayed_buffer_index(
            current as usize,
            console_target as usize,
            framebuffer_a as usize,
            framebuffer_b as usize,
        );
        let Some(index) = selection else {
            let mut diagnostic = LineBuf::new();
            let _ = writeln!(
                &mut diagnostic,
                "INFO XFB mismatch current=0x{:08x} next=0x{:08x} console=0x{:08x} A=0x{:08x} B=0x{:08x}",
                current as usize,
                video::get_next_framebuffer() as usize,
                console_target as usize,
                framebuffer_a as usize,
                framebuffer_b as usize,
            );
            let _ = capture_screen_bytes(diagnostic.as_str().as_bytes());
            return Err(-22);
        };
        Ok([framebuffer_a, framebuffer_b][index])
    }

    pub fn init() -> Self {
        video::init();
        if let Some(framebuffer) = init_buffered_console() {
            video::console_clear();
            let _ = screen_write(b"\n\n");
            return Self {
                console: video::console_get_default(),
                console_ready: true,
                framebuffer_bytes: framebuffer.bytes,
                #[cfg(feature = "dolphin-automation")]
                framebuffer_address: framebuffer.address,
                #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
                framebuffer_width: framebuffer.width,
                #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
                framebuffer_height: framebuffer.height,
                #[cfg(feature = "dolphin-automation")]
                framebuffer_stride: framebuffer.stride,
                page_index: Cell::new(1),
                captured_pages: Cell::new(0),
                #[cfg(feature = "agent-report")]
                agent_page_reports: Cell::new(false),
                #[cfg(feature = "agent-report")]
                page_report_error: Cell::new(None),
                #[cfg(feature = "agent-report")]
                initialized_ticks: system::time(),
                #[cfg(feature = "agent-report")]
                render_mode: framebuffer.render_mode,
                #[cfg(feature = "agent-report")]
                active_framebuffer_b: Cell::new(false),
            };
        }

        let rmode = unsafe { video::console_init(null_mut()) };
        if !rmode.is_null() {
            video::console_clear();
            let _ = screen_write(b"\n\n");
        }
        Self {
            console: rmode,
            console_ready: !rmode.is_null(),
            framebuffer_bytes: 0,
            #[cfg(feature = "dolphin-automation")]
            framebuffer_address: 0,
            #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
            framebuffer_width: 0,
            #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
            framebuffer_height: 0,
            #[cfg(feature = "dolphin-automation")]
            framebuffer_stride: 0,
            page_index: Cell::new(1),
            captured_pages: Cell::new(0),
            #[cfg(feature = "agent-report")]
            agent_page_reports: Cell::new(false),
            #[cfg(feature = "agent-report")]
            page_report_error: Cell::new(None),
            #[cfg(feature = "agent-report")]
            initialized_ticks: system::time(),
            #[cfg(feature = "agent-report")]
            render_mode: null_mut(),
            #[cfg(feature = "agent-report")]
            active_framebuffer_b: Cell::new(false),
        }
    }

    pub const fn console_ready(&self) -> bool {
        self.console_ready
    }

    /// Enable page transmission after IOS and the report network are ready.
    pub fn enable_page_reports(&self) {
        #[cfg(feature = "agent-report")]
        self.agent_page_reports.set(true);
    }

    pub const fn console(&self) -> *mut video::PrintConsole {
        self.console
    }

    pub fn println(&self, msg: &str) {
        if self.console_ready {
            self.clear_if_near_bottom();
            let _ = screen_write(msg.as_bytes());
            let _ = screen_write(b"\n");
        }
    }

    pub fn print_startup_status(&self, status: &ogc_system::StartupSnapshot) {
        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "INFO startup stage={} completed_mask=0x{:03x} failed_stage={}",
            status.init_stage, status.init_completed_mask, status.init_failed_stage
        );
        self.println(line.as_str());
    }

    pub fn print_agent_argument_status(&self) {
        let source = if agent_arguments_from_system() {
            "system"
        } else {
            "entry"
        };
        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "INFO Wiiload arguments source={source} count={}",
            agent_argument_count()
        );
        self.println(line.as_str());
    }

    pub fn wait_vsync(&self) {
        wait_vsync();
    }

    pub fn clear_if_near_bottom(&self) {
        if !self.console_ready {
            return;
        }

        let mut col = 0;
        let mut row = 0;
        let mut cols = 0;
        let mut rows = 0;

        video::console_get_position(&mut col, &mut row);
        video::console_get_metrics(&mut cols, &mut rows);

        if rows > 0 && row >= rows - 1 {
            self.capture_page();
            self.page_index.set(self.page_index.get().saturating_add(1));
            #[cfg(feature = "agent-report")]
            match self.prepare_agent_report_page() {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => {
                    screen_report_error("console page buffer switch", error);
                    return;
                }
            }
            video::console_clear();
            self.write_page_marker();
            wait_vsync();
        }
    }

    fn write_page_marker(&self) {
        let mut line = LineBuf::new();
        let _ = write!(&mut line, "-- page {} --", self.page_index.get());
        let _ = screen_write(line.as_str().as_bytes());
        let _ = screen_write(b"\n");
    }

    #[cfg(feature = "agent-report")]
    fn prepare_agent_report_page(&self) -> Result<bool, i32> {
        if self.render_mode.is_null() || self.framebuffer_bytes == 0 {
            return Ok(false);
        }
        let next_is_b = !self.active_framebuffer_b.get();
        let next_framebuffer = unsafe {
            if next_is_b {
                PROBE_FRAMEBUFFER_B
            } else {
                PROBE_FRAMEBUFFER_A
            }
        };
        let width = unsafe { (*self.render_mode).fb_width as i32 } - SAFE_MARGIN_X * 2;
        let height = unsafe { (*self.render_mode).xfb_height as i32 } - SAFE_MARGIN_Y * 2;
        // The old page remains on the active XFB while the console clears and
        // draws the next page into the other buffer. VI changes only after the
        // new page marker is ready.
        let previous_callback = video::set_post_retrace_callback(None);
        let initialized = unsafe {
            video::console_init_framebuffer(
                self.render_mode,
                next_framebuffer,
                SAFE_MARGIN_X,
                SAFE_MARGIN_Y,
                width,
                height,
            )
        };
        video::set_post_retrace_callback(previous_callback);
        initialized?;
        unsafe { activity_indicator::set_framebuffer(next_framebuffer) };
        self.active_framebuffer_b.set(next_is_b);
        self.write_page_marker();
        // SAFETY: both probe XFBs were allocated for this mode and remain live
        // for the complete display session.
        unsafe { video::set_next_framebuffer(next_framebuffer) };
        video::flush();
        wait_vsync();
        Ok(true)
    }

    pub fn capture_page(&self) {
        if !self.console_ready
            || self.framebuffer_bytes == 0
            || unsafe { PROBE_FRAMEBUFFER_A.is_null() || PROBE_FRAMEBUFFER_B.is_null() }
        {
            return;
        }

        // Dolphin holds a stable copy while the host captures this page.
        #[cfg(feature = "dolphin-automation")]
        unsafe {
            framebuffer_copy::copy_words(
                PROBE_FRAMEBUFFER_A.cast::<u32>(),
                PROBE_FRAMEBUFFER_B.cast::<u32>(),
                self.framebuffer_bytes,
            );
            video::set_next_framebuffer(PROBE_FRAMEBUFFER_B);
            video::flush();
        }
        self.captured_pages
            .set(self.captured_pages.get().saturating_add(1));
        #[cfg(feature = "agent-report")]
        if self.agent_page_reports.get() {
            let frame = video::retrace_count();
            let mut marker = LineBuf::new();
            if writeln!(
                &mut marker,
                "INFO page={} capture={} vi_frame={frame}",
                self.page_index.get(),
                self.captured_pages.get(),
            )
            .is_err()
            {
                self.page_report_error.set(Some(-1));
                return;
            }
            // Keep page metadata in the transcript without scrolling the XFB.
            if !capture_screen_bytes(marker.as_str().as_bytes()) {
                self.page_report_error.set(Some(-1));
                screen_report_error("page transcript capacity", -1);
                return;
            }
            // Send a copy of the displayed page. The live frame counter
            // continues to update the displayed buffer during transmission.
            let framebuffer = unsafe {
                let source = match self.vi_displayed_framebuffer() {
                    Ok(source) => source,
                    Err(error) => {
                        self.page_report_error.set(Some(error));
                        screen_report_error("VI and console framebuffer mismatch", error);
                        return;
                    }
                };
                let snapshot = if source == PROBE_FRAMEBUFFER_A {
                    PROBE_FRAMEBUFFER_B
                } else {
                    PROBE_FRAMEBUFFER_A
                };
                framebuffer_copy::copy_words(
                    source.cast::<u32>(),
                    snapshot.cast::<u32>(),
                    self.framebuffer_bytes,
                );
                core::slice::from_raw_parts(snapshot.cast::<u8>(), self.framebuffer_bytes)
            };
            if let Err(error) = send_agent_framebuffer_report_kind(
                captured_screen_text(),
                framebuffer,
                self.framebuffer_width,
                self.framebuffer_height,
                AgentPerformance {
                    elapsed_us: ogc_video::ticks_to_microseconds(
                        system::time().wrapping_sub(self.initialized_ticks),
                        ogc_video::timer_clock_hz(),
                    ),
                    frame_count: frame,
                    heap_used_bytes: crate::alloc_support::used_bytes().min(u32::MAX as usize)
                        as u32,
                    ..AgentPerformance::default()
                },
                true,
            ) {
                if self.page_report_error.get().is_none() {
                    self.page_report_error.set(Some(error));
                }
                screen_report_error("page screenshot", error);
            }
        }
        #[cfg(feature = "dolphin-automation")]
        {
            let mut marker = LineBuf::new();
            let _ = write!(
                &mut marker,
                "OGC_DOLPHIN_PAGE_CAPTURE page={} capture={} addr=0x{:08x} width={} height={} stride={}",
                self.page_index.get(),
                self.captured_pages.get(),
                self.framebuffer_address,
                self.framebuffer_width,
                self.framebuffer_height,
                self.framebuffer_stride,
            );
            dolphin_transport_write(marker.as_str().as_bytes());
            dolphin_transport_write(b"\n");
        }

        // Keep the source page stable after its marker is complete. The host
        // can pause Dolphin and read buffer A while VI displays its copy in B.
        #[cfg(feature = "dolphin-automation")]
        wait_frames(DOLPHIN_PAGE_HOLD_FRAMES);

        #[cfg(feature = "dolphin-automation")]
        unsafe {
            video::set_next_framebuffer(PROBE_FRAMEBUFFER_A);
            video::flush();
            video::wait_vsync();
        }
    }
}

#[cfg(feature = "dolphin-automation")]
fn wait_frames(count: u32) {
    for _ in 0..count {
        wait_vsync();
    }
}

struct FramebufferInfo {
    bytes: usize,
    #[cfg(feature = "agent-report")]
    render_mode: *mut video::GxrModeObj,
    #[cfg(feature = "dolphin-automation")]
    address: usize,
    #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
    width: u32,
    #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
    height: u32,
    #[cfg(feature = "dolphin-automation")]
    stride: u32,
}

/// Return the complete regions allocated for the two probe framebuffers.
/// Call after console initialization on the initial thread.
pub fn framebuffer_regions() -> Option<[core::ops::Range<usize>; 2]> {
    let mode = unsafe { video::get_preferred_mode(null_mut()) };
    if mode.is_null() {
        return None;
    }
    let bytes = unsafe { video::get_frame_buffer_size(mode) } as usize;
    let (a, b) = unsafe { (PROBE_FRAMEBUFFER_A as usize, PROBE_FRAMEBUFFER_B as usize) };
    if a == 0 || b == 0 || bytes == 0 {
        return None;
    }
    Some([a..a.checked_add(bytes)?, b..b.checked_add(bytes)?])
}

fn init_buffered_console() -> Option<FramebufferInfo> {
    // SAFETY: null requests the library-owned preferred mode object; it is only
    // read while configuring the display below.
    let rmode = unsafe { video::get_preferred_mode(null_mut()) };
    if rmode.is_null() {
        return None;
    }

    unsafe {
        let framebuffer_bytes = video::get_frame_buffer_size(rmode);
        let framebuffer_a = system::allocate_framebuffer(rmode);
        let framebuffer_b = system::allocate_framebuffer(rmode);
        if framebuffer_a.is_null() || framebuffer_b.is_null() {
            return None;
        }

        PROBE_FRAMEBUFFER_A = uncached_framebuffer(framebuffer_a);
        PROBE_FRAMEBUFFER_B = uncached_framebuffer(framebuffer_b);

        video::clear_frame_buffer(rmode, PROBE_FRAMEBUFFER_A, PROBE_BG);
        video::clear_frame_buffer(rmode, PROBE_FRAMEBUFFER_B, PROBE_BG);
        video::configure(rmode);
        video::set_next_framebuffer(PROBE_FRAMEBUFFER_A);
        video::set_black(false);
        video::flush();
        video::wait_vsync();
        if ((*rmode).vi_tv_mode & 1) != 0 {
            video::wait_vsync();
        }

        let width = ((*rmode).fb_width as i32 - (SAFE_MARGIN_X * 2)).max(8);
        let height = ((*rmode).xfb_height as i32 - (SAFE_MARGIN_Y * 2)).max(16);
        let initialized = video::console_init_framebuffer(
            rmode,
            PROBE_FRAMEBUFFER_A,
            SAFE_MARGIN_X,
            SAFE_MARGIN_Y,
            width,
            height,
        )
        .is_ok();
        initialized.then(|| {
            let bytes = framebuffer_bytes as usize;
            FramebufferInfo {
                bytes,
                #[cfg(feature = "agent-report")]
                render_mode: rmode,
                #[cfg(feature = "dolphin-automation")]
                address: ogc_core::mem_k1_to_k0(PROBE_FRAMEBUFFER_A as usize),
                #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
                width: (*rmode).fb_width as u32,
                #[cfg(any(feature = "dolphin-automation", feature = "agent-report"))]
                height: (*rmode).xfb_height as u32,
                #[cfg(feature = "dolphin-automation")]
                stride: (bytes as u32) / ((*rmode).xfb_height as u32).saturating_mul(2).max(1),
            }
        })
    }
}

pub(crate) fn uncached_framebuffer(framebuffer: *mut c_void) -> *mut c_void {
    let address = framebuffer as usize;
    if address & 0xe000_0000 == system::SYS_BASE_CACHED as usize {
        ogc_core::mem_k0_to_k1(address) as *mut c_void
    } else {
        framebuffer
    }
}

/// Displays a fixed panic marker after the probe console has been initialized.
///
/// The panic path intentionally avoids formatting, allocation, IOS calls, and
/// scheduler operations. If the panic occurs before console initialization,
/// the loop still keeps VI active so an attached debugger can inspect state.
pub fn panic_screen() -> ! {
    OGC_PROBE_BOOT_STAGE.store(u32::MAX, Ordering::Release);
    video::set_black(false);
    let _ = screen_write(b"\n\nFATAL: probe panic\n");
    video::flush();
    loop {
        video::wait_vsync();
    }
}

pub fn set_probe_app_dir(app_dir: &'static str) {
    unsafe {
        PROBE_APP_DIR = Some(app_dir);
    }
}

#[cfg(feature = "mounted-log")]
pub fn argv0_bytes() -> Option<&'static [u8]> {
    derive_argv0_bytes()
}

#[cfg(feature = "mounted-log")]
#[derive(Copy, Clone)]
pub struct StartupArgvSummary {
    pub argv_magic: i32,
    pub length: i32,
    pub argc: i32,
    pub command_line: usize,
    pub argv: usize,
    pub end_argv: usize,
}

#[cfg(feature = "mounted-log")]
pub fn startup_argv_summary() -> Option<StartupArgvSummary> {
    let argv = unsafe { sys::current_system_argv().as_ref()? };
    Some(StartupArgvSummary {
        argv_magic: argv.argv_magic,
        length: argv.length,
        argc: argv.argc,
        command_line: argv.command_line as usize,
        argv: argv.argv as usize,
        end_argv: argv.end_argv as usize,
    })
}

/// Result of installing the probe power-button shutdown handlers.
pub struct PowerShutdownInstall {
    pub stm_init: i32,
    pub chained_previous_stm_handler: bool,
    pub had_previous_power_callback: bool,
}

pub fn install_power_shutdown() -> PowerShutdownInstall {
    POWER_REQUESTED.store(false, Ordering::Release);
    let previous_power = system::set_power_callback(Some(power_callback));

    PowerShutdownInstall {
        stm_init: 0,
        chained_previous_stm_handler: false,
        had_previous_power_callback: previous_power.is_some(),
    }
}

pub fn wait_vsync() {
    // Screen pagination must not prevent the probe from returning its failure
    // report. Poll the real VI counter with a bound and retain every timeout.
    if !video::wait_vsync_timeout_us(100_000) {
        VSYNC_TIMEOUTS.fetch_add(1, Ordering::AcqRel);
    }
    service_power_shutdown();
}

pub fn service_power_shutdown() {
    if !POWER_REQUESTED.swap(false, Ordering::AcqRel) {
        return;
    }

    let _ = stm::init_without_event_hook();
    let rc = stm::shutdown_to_standby();
    if rc < 0 {
        // SAFETY: the reset mode is a documented constant and no callback state
        // is supplied.
        unsafe { system::reset_system(system::sys::SYS_POWEROFF_STANDBY as i32, 0, 0) };
    }
}

#[cfg(feature = "agent-report")]
pub fn arm_agent_recovery_watchdog() -> bool {
    recovery_reset_delay_ms()
        .and_then(arm_recovery_reset)
        .is_some()
}

#[cfg(not(feature = "agent-report"))]
pub fn arm_agent_recovery_watchdog() -> bool {
    false
}

#[cfg(feature = "agent-report")]
fn recovery_watchdog_requested() -> bool {
    recovery_reset_delay_ms().is_some()
}

unsafe extern "C" fn power_callback() {
    POWER_REQUESTED.store(true, Ordering::Release);
}

impl ProbeLogger {
    pub fn new(display: ProbeDisplay) -> Self {
        Self::with_options(display, ProbeLoggerOptions::default())
    }

    pub fn with_options(display: ProbeDisplay, options: ProbeLoggerOptions) -> Self {
        #[cfg(feature = "agent-report")]
        let recovery_alarm = take_recovery_reset();
        Self {
            file: None,
            log_state: LogState::default(),
            file_policy: options.file_policy,
            file_bytes_written: 0,
            display,
            passes: 0,
            fails: 0,
            failures: FailureSummary::new(),
            hardware_summary: None,
            step_count: 0,
            last_step_frame: video::retrace_count(),
            #[cfg(feature = "agent-report")]
            started_ticks: system::time(),
            #[cfg(feature = "agent-report")]
            warmup_frames: agent_frame_option(b"--warmup-frames=").unwrap_or(0),
            #[cfg(feature = "agent-report")]
            sample_frames: agent_frame_option(b"--sample-frames=").unwrap_or(0),
            #[cfg(feature = "agent-report")]
            recovery_alarm,
        }
    }

    pub fn has_log_file(&self) -> bool {
        self.file.is_some() && self.log_state.is_healthy()
    }

    /// Opens the selected log backend now and flushes all boot records to it.
    pub fn start_logging(&mut self) -> bool {
        if self.file.is_none() && self.logging_requested() {
            self.file = open_log_file(self.file_policy);
            if self.file.is_some() {
                self.flush_pending_to_file();
            } else {
                self.log_state.fail(LogError::BackendUnavailable);
            }
        }
        self.file.is_some()
    }

    /// Writes a durable phase boundary for mounted-storage probe logs.
    pub fn checkpoint(&mut self, label: &str) -> bool {
        let mut line = LineBuf::new();
        let _ = write!(&mut line, "CHECKPOINT {}", label);
        self.write_line_raw(line.as_str());
        #[cfg(feature = "dolphin-automation")]
        {
            self.log_state.is_healthy()
        }
        #[cfg(not(feature = "dolphin-automation"))]
        {
            self.start_logging() && self.sync_log_file()
        }
    }

    pub const fn logging_requested(&self) -> bool {
        !matches!(self.file_policy, LogFilePolicy::Disabled)
    }

    pub const fn logging_disabled(&self) -> bool {
        matches!(self.file_policy, LogFilePolicy::Disabled)
    }

    pub fn log_backend_name(&self) -> &'static str {
        self.file
            .as_ref()
            .map_or("unavailable", LogFile::backend_name)
    }

    pub fn uses_mounted_log(&self) -> bool {
        matches!(self.file, Some(LogFile::Mounted(_)))
    }

    pub fn uses_usb_or_unknown_mounted_log(&self) -> bool {
        matches!(self.file.as_ref(), Some(LogFile::Mounted(file)) if file.is_usb_or_unknown())
    }

    pub fn set_console_ready(&mut self, ready: bool) {
        self.display.console_ready = ready;
    }

    pub const fn display(&self) -> &ProbeDisplay {
        &self.display
    }

    /// Enable host screenshots for each console page after network startup.
    pub fn enable_page_reports(&mut self) {
        self.display.enable_page_reports();
    }

    /// Publish the current operation and real VI progress before it can block.
    pub fn report_step(&mut self, label: &str) {
        self.step_count = self.step_count.saturating_add(1);
        let frame = video::retrace_count();
        self.note(&alloc::format!(
            "STEP {} {label}; vi_frame={frame} frame_increment={} passes={} fails={}",
            self.step_count,
            frame.wrapping_sub(self.last_step_frame),
            self.passes,
            self.fails,
        ));
        self.last_step_frame = frame;
        send_agent_progress(1000u32.saturating_add(self.step_count));
    }

    pub fn section(&mut self, name: &str) {
        self.write_line_raw("");
        let mut line = LineBuf::new();
        let _ = write!(&mut line, "== {} ==", name);
        self.write_line_raw(line.as_str());
    }

    pub fn record(&mut self, label: &str, ok: bool) {
        if ok {
            self.passes += 1;
        } else {
            self.fails += 1;
            self.failures.record(label);
        }

        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "{:<4} {}",
            if ok { "PASS" } else { "FAIL" },
            label
        );
        self.write_line_raw(line.as_str());
    }

    /// Record a failed check and keep its underlying error beside it on the
    /// final retained-failures screen.
    pub fn record_with_detail(&mut self, label: &str, ok: bool, detail: &str) {
        self.record(label, ok);
        if !ok {
            self.failures.detail_last(detail);
        }
    }

    /// Returns whether checks logged so far passed and the active log backend
    /// remains healthy. This is a pre-summary candidate; final sync can still
    /// fail during `summary`.
    /// Current completed check counts, for aggregate suite reports.
    /// Retain identity for the final screen and each failure page.
    pub fn set_hardware_summary(
        &mut self,
        model: &str,
        motherboard: &str,
        revision: u32,
    ) -> Result<(), &'static str> {
        let mut lines = [LineBuf::new(), LineBuf::new(), LineBuf::new()];
        write!(&mut lines[0], "Model: {model}").map_err(|_| "model label too long")?;
        write!(&mut lines[1], "MB (label): {motherboard}")
            .map_err(|_| "motherboard label too long")?;
        write!(&mut lines[2], "Hollywood revision: 0x{revision:08x}")
            .map_err(|_| "revision label too long")?;
        self.hardware_summary = Some(lines);
        Ok(())
    }
    fn show_hardware_summary(&self) {
        if let Some(lines) = &self.hardware_summary {
            for line in lines {
                self.display.println(line.as_str());
            }
        }
    }

    /// Add the VI heartbeat below the console text area.
    pub fn enable_activity_indicator(&mut self) {
        let mode = unsafe { video::get_preferred_mode(null_mut()) };
        let enabled = if mode.is_null() {
            false
        } else {
            unsafe {
                activity_indicator::start(
                    PROBE_FRAMEBUFFER_A,
                    self.display.framebuffer_bytes,
                    (*mode).fb_width as usize,
                    (*mode).xfb_height as usize,
                )
            }
        };
        self.record("activity indicator has a valid framebuffer", enabled);
        self.note("Corner: spinner means VI active; IDLE s is seconds since logged progress.");
    }

    pub fn check_counts(&self) -> (usize, usize) {
        (self.passes, self.fails)
    }

    /// Bounded failure pages remain available after runtime cleanup.
    pub fn failure_pages(&self) -> usize {
        self.failures.pages()
    }

    /// Render only through the native console; do not reopen a closed log.
    pub fn show_failure_page(&self, page: usize) {
        video::console_clear();
        let page = page % self.failure_pages();
        let mut title = LineBuf::new();
        if write!(
            &mut title,
            "FAILURES PAGE {}/{}",
            page + 1,
            self.failure_pages()
        )
        .is_err()
        {
            self.display.println("FAIL failure page title format");
        } else {
            self.display.println(title.as_str());
        }
        for (index, (label, detail)) in self.failures.page(page).enumerate() {
            let mut item = LineBuf::new();
            if write!(
                &mut item,
                "FAIL {}: {}",
                page * PAGE_SIZE + index + 1,
                label
            )
            .is_err()
            {
                self.display.println("FAIL failure label format");
            } else {
                self.display.println(item.as_str());
            }
            if let Some(detail) = detail {
                self.display.println(detail);
            }
        }
        let mut total = LineBuf::new();
        if write!(
            &mut total,
            "passes={} fails={} omitted={}",
            self.passes,
            self.fails,
            self.failures.omitted()
        )
        .is_err()
        {
            self.display.println("FAIL failure page counts format");
        } else {
            self.display.println(total.as_str());
        }
        self.show_hardware_summary();
        self.display
            .println("Press console RESET to read the next page.");
    }

    pub fn checks_passed(&self) -> bool {
        self.fails == 0 && self.log_state.is_healthy()
    }

    pub fn summary(&mut self) {
        self.summary_with_exit(true);
    }

    /// Send the normal diagnostic summary and let the caller perform its own
    /// explicit exit. Upload arguments cannot select a different exit route.
    #[cfg(feature = "agent-report")]
    pub fn summary_before_explicit_exit(&mut self) {
        self.summary_with_exit(false);
    }

    fn summary_with_exit(&mut self, allow_requested_exit: bool) {
        #[cfg(not(feature = "agent-report"))]
        assert!(
            allow_requested_exit,
            "explicit-exit reports require agent-report"
        );
        #[cfg(feature = "agent-report")]
        let page_error_at_summary = self.display.page_report_error.get();
        #[cfg(feature = "agent-report")]
        if self.display.agent_page_reports.get() {
            let error = self.display.page_report_error.get();
            self.record_with_detail(
                "all page screenshots sent to host",
                error.is_none(),
                &alloc::format!("first_error={error:?}"),
            );
        }
        let timeouts = VSYNC_TIMEOUTS.load(Ordering::Acquire);
        self.record_with_detail(
            "probe VI retraces complete within 100 ms",
            timeouts == 0,
            &alloc::format!("timeouts={timeouts}"),
        );
        #[cfg(feature = "agent-report")]
        let metrics = self.measure_agent_performance();
        #[cfg(feature = "agent-report")]
        {
            let watchdog_status = if self.recovery_alarm.is_some() {
                "armed"
            } else if recovery_watchdog_requested() {
                "arm_failed"
            } else {
                "not_requested"
            };
            let mut watchdog_line = LineBuf::new();
            let _ = write!(
                &mut watchdog_line,
                "INFO recovery_reset_watchdog={watchdog_status}"
            );
            self.write_line_raw(watchdog_line.as_str());

            let mut line = LineBuf::new();
            let _ = write!(
                &mut line,
                "INFO performance elapsed_us={} frame_count={} heap_used_bytes={} warmup_frames={} sample_frames={} frame_time_average_us={} frame_time_min_us={} frame_time_max_us={}",
                metrics.elapsed_us,
                metrics.frame_count,
                metrics.heap_used_bytes,
                metrics.warmup_frames,
                metrics.sample_frames,
                metrics.frame_time_average_us,
                metrics.frame_time_min_us,
                metrics.frame_time_max_us,
            );
            self.write_line_raw(line.as_str());
            let mut ticks = LineBuf::new();
            let _ = write!(
                &mut ticks,
                "INFO timebase elapsed_ticks={} frame_average_ticks={} frame_min_ticks={} frame_max_ticks={}",
                metrics.elapsed_ticks,
                metrics.frame_time_average_ticks,
                metrics.frame_time_min_ticks,
                metrics.frame_time_max_ticks,
            );
            self.write_line_raw(ticks.as_str());
            if metrics.sample_frames != 0 {
                self.record(
                    "frame waits take at least one millisecond",
                    metrics.frame_time_min_us >= 1_000 && metrics.frame_time_average_us >= 1_000,
                );
            }
        }
        #[cfg(feature = "dolphin-automation")]
        self.record(
            "Dolphin screen-data transport completed",
            DOLPHIN_TRANSPORT_ERROR.load(Ordering::Acquire) == 0,
        );
        finish_summary(
            self,
            |logger| {
                logger.finish_logging();
                let started = !logger.logging_requested() || logger.start_logging();
                if logger.logging_requested() {
                    logger.flush_pending_to_file();
                    let synced = logger.sync_log_file();
                    started && synced && logger.log_state.is_healthy()
                } else {
                    started && logger.log_state.is_healthy()
                }
            },
            |logger| {
                let mut line = LineBuf::new();
                let _ = write!(
                    &mut line,
                    "INFO summary_candidate passes={} fails={}",
                    logger.passes, logger.fails
                );
                logger.write_line_raw(line.as_str());
                logger.log_state.is_healthy()
            },
            |logger| {
                if logger.logging_requested() {
                    logger.flush_pending_to_file();
                    logger.sync_log_file()
                } else {
                    logger.log_state.is_healthy()
                }
            },
            |logger, summary_io_ok| {
                if !summary_io_ok || !logger.log_state.is_healthy() {
                    logger.fails = logger.fails.saturating_add(1);
                    logger.failures.record("probe log or screen output failed");
                }
                let mut summary = LineBuf::new();
                let _ = write!(
                    &mut summary,
                    "{:<4} summary passes={} fails={}",
                    if logger.fails == 0 && logger.log_state.is_healthy() {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    logger.passes,
                    logger.fails
                );
                // Rebuild the final screen from retained failures. Earlier
                // pages can pass too fast for a console photo.
                if logger.fails != 0 {
                    logger.display.capture_page();
                    video::console_clear();
                    logger
                        .display
                        .println("FAILED CHECKS (retained from all pages)");
                    for (label, detail) in logger.failures.entries() {
                        let mut line = LineBuf::new();
                        match write!(&mut line, "FAIL {}", label) {
                            Ok(()) => logger.display.println(line.as_str()),
                            Err(_) => logger.display.println("FAIL failure label format error"),
                        }
                        if let Some(detail) = detail {
                            let mut line = LineBuf::new();
                            match write!(&mut line, "  {}", detail) {
                                Ok(()) => logger.display.println(line.as_str()),
                                Err(_) => logger.display.println("  error detail format error"),
                            }
                        }
                    }
                    let omitted = logger.failures.omitted();
                    if omitted != 0 {
                        let mut line = LineBuf::new();
                        match write!(
                            &mut line,
                            "FAIL {} more checks; screen capacity reached",
                            omitted
                        ) {
                            Ok(()) => logger.display.println(line.as_str()),
                            Err(_) => logger.display.println("FAIL screen capacity exceeded"),
                        }
                    }
                }
                logger.show_hardware_summary();
                logger.display.println(summary.as_str());
                logger.display.capture_page();
            },
        );
        #[cfg(feature = "agent-report")]
        if page_error_at_summary.is_none()
            && let Some(error) = self.display.page_report_error.get()
        {
            self.record_with_detail(
                "final page screenshot sent to host",
                false,
                &alloc::format!("error={error}"),
            );
            self.note(&alloc::format!("final page screenshot error={error}"));
            self.write_line_raw(&alloc::format!(
                "FAIL summary passes={} fails={}",
                self.passes,
                self.fails,
            ));
        }
        #[cfg(feature = "dolphin-automation")]
        {
            dolphin_transport_write(b"OGC_DOLPHIN_CAPTURE_COMPLETE\n");
            if DOLPHIN_TRANSPORT_ERROR.load(Ordering::Acquire) != 0 {
                self.display
                    .println("FAIL Dolphin screen-data transport completed");
            }
        }
        if !self.log_state.is_healthy() {
            let mut error_line = LineBuf::new();
            let _ = write!(
                &mut error_line,
                "FAIL probe log status error={:?} lost_bytes={}",
                self.log_state.first_error(),
                self.log_state.lost_bytes(),
            );
            self.display.println(error_line.as_str());
        }
        #[cfg(feature = "agent-report")]
        {
            let restart_after_probe = allow_requested_exit && restart_after_probe_requested();
            let return_from_main = allow_requested_exit && return_from_main_requested();
            let return_to_menu = allow_requested_exit && return_to_system_menu_requested();
            let exit_requested = restart_after_probe || return_from_main || return_to_menu;
            if return_to_menu {
                self.write_line_raw("INFO exit_path=system_menu return=requested");
            } else if restart_after_probe || return_from_main {
                let stub_present = system::sys::sys_hbc_reload_stub_present();
                self.write_line_raw(if return_from_main && stub_present {
                    "INFO exit_path=hbc_reload_stub status=present return=main_dispatch"
                } else if return_from_main {
                    "INFO exit_path=system_restart status=stub_missing return=main_dispatch"
                } else if stub_present {
                    "INFO exit_path=hbc_reload_stub status=present return=requested"
                } else {
                    "INFO exit_path=system_restart status=stub_missing return=requested"
                });
            }
            let report_result = self.send_agent_report(metrics);
            if let Err(error) = report_result {
                self.record_with_detail(
                    "final report sent to host",
                    false,
                    &alloc::format!("error={error}"),
                );
                self.write_line_raw(&alloc::format!(
                    "FAIL summary passes={} fails={}",
                    self.passes,
                    self.fails
                ));
                if !report_exit::should_continue_after_report_failure(exit_requested) {
                    return;
                }
                self.display
                    .println("INFO continuing to requested exit after report failure");
            }
            if allow_requested_exit
                && report_result.is_ok()
                && verify_recovery_reset_requested()
                && self.recovery_alarm.is_some()
            {
                loop {
                    wait_vsync();
                }
            }
            let system_reset_requested = return_to_menu || restart_after_probe;
            if report_exit::should_cancel_recovery_watchdog(
                report_result.is_ok(),
                system_reset_requested,
            ) {
                self.display
                    .println("INFO exit report complete; cancel watchdog");
                cancel_recovery_reset(self.recovery_alarm);
            } else if self.recovery_alarm.is_some() {
                self.display
                    .println("INFO recovery watchdog remains armed for exit");
            }
            if return_to_menu {
                self.display.println("INFO exit entering System Menu reset");
                system::sys::set_reset_debug_hook(Some(probe_reset_progress));
                // SAFETY: this is the synchronous reset-entry record. It uses
                // only bounded atomic stores and does not dereference pointers.
                unsafe { probe_reset_progress(u32::MAX, 0, 0) };
                // SAFETY: normal return to the Wii Channels menu was requested
                // explicitly, after all probe work and report writes finished.
                unsafe { system::reset_system(system::SYS_RETURNTOMENU as i32, 0, 0) };
                self.display
                    .println("FAIL System Menu return request came back");
                loop {
                    core::hint::spin_loop();
                }
            }
            if restart_after_probe && !return_from_main {
                restart_to_hbc();
            }
        }
    }

    #[cfg(feature = "agent-report")]
    fn measure_agent_performance(&mut self) -> AgentPerformance {
        for _ in 0..self.warmup_frames {
            wait_vsync();
        }
        let mut total = 0u64;
        let mut minimum = u64::MAX;
        let mut maximum = 0u64;
        for _ in 0..self.sample_frames {
            let start = system::time();
            wait_vsync();
            let elapsed = system::time().wrapping_sub(start);
            total = total.saturating_add(elapsed);
            minimum = minimum.min(elapsed);
            maximum = maximum.max(elapsed);
        }
        let elapsed_ticks = system::time().wrapping_sub(self.started_ticks);
        let timer_clock_hz = ogc_video::timer_clock_hz();
        let ticks_to_us = |ticks: u64| ogc_video::ticks_to_microseconds(ticks, timer_clock_hz);
        AgentPerformance {
            elapsed_ticks,
            elapsed_us: ticks_to_us(elapsed_ticks),
            frame_time_average_ticks: if self.sample_frames == 0 {
                0
            } else {
                total / self.sample_frames as u64
            },
            frame_time_min_ticks: if minimum == u64::MAX { 0 } else { minimum },
            frame_time_max_ticks: maximum,
            frame_count: self.sample_frames,
            heap_used_bytes: crate::alloc_support::used_bytes().min(u32::MAX as usize) as u32,
            warmup_frames: self.warmup_frames,
            sample_frames: self.sample_frames,
            frame_time_average_us: if self.sample_frames == 0 {
                0
            } else {
                ticks_to_us(total / self.sample_frames as u64).min(u32::MAX as u64) as u32
            },
            frame_time_min_us: if minimum == u64::MAX {
                0
            } else {
                ticks_to_us(minimum).min(u32::MAX as u64) as u32
            },
            frame_time_max_us: ticks_to_us(maximum).min(u32::MAX as u64) as u32,
        }
    }

    #[cfg(feature = "agent-report")]
    fn send_agent_report(&mut self, metrics: AgentPerformance) -> Result<(), i32> {
        if report_identity().is_none() {
            self.display.println(
                "FAIL report identity missing; run with --report, --run-id, and --build-id",
            );
            return Err(-1);
        }
        let framebuffer = if self.display.framebuffer_bytes == 0 {
            &[]
        } else {
            let displayed = self.display.vi_displayed_framebuffer()?;
            // SAFETY: VI scans one of the display's permanent XFB allocations.
            unsafe {
                core::slice::from_raw_parts(displayed.cast::<u8>(), self.display.framebuffer_bytes)
            }
        };
        send_agent_framebuffer_report_kind(
            captured_screen_text(),
            framebuffer,
            self.display.framebuffer_width,
            self.display.framebuffer_height,
            metrics,
            false,
        )
    }

    pub fn note(&mut self, line: &str) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "INFO {}", line);
        self.write_line_raw(out.as_str());
    }

    /// Writes one caller-formatted event record without adding a text prefix.
    pub fn write_event_record(&mut self, record: &str) -> bool {
        self.display.clear_if_near_bottom();
        self.write_bytes(record.as_bytes());
        self.write_bytes(b"\n");
        self.log_state.is_healthy()
    }

    pub fn note_i32(&mut self, label: &str, value: i32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}={}", label, value);
        self.note(out.as_str());
    }

    pub fn note_u32(&mut self, label: &str, value: u32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}={}", label, value);
        self.note(out.as_str());
    }

    pub fn note_u64(&mut self, label: &str, value: u64) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}={}", label, value);
        self.note(out.as_str());
    }

    pub fn note_hex32(&mut self, label: &str, value: u32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}=0x{:08x}", label, value);
        self.note(out.as_str());
    }

    pub fn note_i32_index(&mut self, label: &str, index: i32, value: i32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}{}={}", label, index, value);
        self.note(out.as_str());
    }

    pub fn note_u32_index(&mut self, label: &str, index: i32, value: u32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}{}={}", label, index, value);
        self.note(out.as_str());
    }

    pub fn note_hex32_index(&mut self, label: &str, index: i32, value: u32) {
        let mut out = LineBuf::new();
        let _ = write!(&mut out, "{}{}=0x{:08x}", label, index, value);
        self.note(out.as_str());
    }

    fn write_line_raw(&mut self, line: &str) {
        activity_indicator::progress();
        self.display.clear_if_near_bottom();
        self.write_bytes(line.as_bytes());
        self.write_bytes(b"\n");
        self.display.wait_vsync();
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if self.logging_requested() {
            let limit = self
                .file
                .as_ref()
                .and_then(LogFile::write_limit)
                .or_else(|| self.file.is_none().then_some(MOUNTED_LOG_WRITE_LIMIT));
            self.log_state.append(bytes, limit, self.file_bytes_written);
        }
        if self.display.console_ready() {
            match screen_write(bytes) {
                Ok(written) if written == bytes.len() => {}
                Ok(written) => self.log_state.fail(LogError::ScreenWrite(written as i32)),
                Err(code) => self.log_state.fail(LogError::ScreenWrite(code)),
            }
        } else {
            #[cfg(feature = "dolphin-automation")]
            dolphin_transport_write(bytes);
        }
        if self.file.is_some() {
            self.flush_pending_to_file();
        }
    }

    fn finish_logging(&mut self) {
        if self.file.is_some() || !self.logging_requested() {
            return;
        }
        self.display.println("INFO opening deferred log backend");
        let started = self.start_logging();
        if started {
            self.display.println("INFO deferred log flushed");
        } else {
            self.display.println("INFO deferred log unavailable");
        }
    }

    fn flush_pending_to_file(&mut self) {
        let Some(file) = self.file.as_ref() else {
            return;
        };
        match self.log_state.flush_with(|bytes| file.write_all(bytes)) {
            Ok(written) => {
                self.file_bytes_written = self.file_bytes_written.saturating_add(written);
            }
            Err(code) => self.log_state.fail(LogError::FileWrite(code)),
        }
    }

    fn sync_log_file(&mut self) -> bool {
        let Some(file) = self.file.as_ref() else {
            if self.logging_requested() {
                self.log_state.fail(LogError::BackendUnavailable);
            }
            return !self.logging_requested();
        };
        match file.sync() {
            Ok(()) => self.log_state.is_healthy(),
            Err(code) => {
                self.log_state.fail(LogError::Sync(code));
                false
            }
        }
    }
}

#[cfg(feature = "agent-report")]
fn screen_report_error(stage: &str, code: i32) {
    let mut line = LineBuf::new();
    let _ = write!(&mut line, "FAIL report {stage} rc={code}");
    let _ = screen_write(line.as_str().as_bytes());
    let _ = screen_write(b"\n");
}

#[cfg(feature = "agent-report")]
#[derive(Clone, Copy)]
struct ReportIdentity {
    run_id: &'static [u8],
    build_id: &'static [u8],
}

#[cfg(feature = "agent-report")]
fn report_identity() -> Option<ReportIdentity> {
    report_target()?;
    let (run_id, build_id) = report_run_identity()?;
    Some(ReportIdentity { run_id, build_id })
}

#[cfg(feature = "agent-report")]
fn report_run_identity() -> Option<(&'static [u8], &'static [u8])> {
    let run_id = agent_arg(b"--run-id=")?;
    let build_id = agent_arg(b"--build-id=")?;
    let (run_id, build_id) = input_identity::validate_run_identity(run_id, build_id)?;
    Some((run_id.as_bytes(), build_id.as_bytes()))
}

/// Returns validated Wiiload run and DOL build identities for event records.
/// Returns `None` when report arguments are absent or invalid.
pub fn input_run_identity() -> Option<(&'static str, &'static str)> {
    #[cfg(feature = "agent-report")]
    {
        let (run_id, build_id) = report_run_identity()?;
        Some((
            core::str::from_utf8(run_id).ok()?,
            core::str::from_utf8(build_id).ok()?,
        ))
    }
    #[cfg(not(feature = "agent-report"))]
    {
        None
    }
}

#[cfg(feature = "agent-report")]
fn report_target() -> Option<([u8; 4], u16)> {
    let endpoint = agent_arg(b"--report=")?;
    let colon = endpoint.iter().position(|byte| *byte == b':')?;
    let ip = core::str::from_utf8(&endpoint[..colon]).ok()?;
    let mut octets = [0u8; 4];
    let mut parts = ip.split('.');
    for octet in &mut octets {
        *octet = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    let port = core::str::from_utf8(&endpoint[colon + 1..])
        .ok()?
        .parse()
        .ok()?;
    Some((octets, port))
}

#[cfg(feature = "agent-report")]
pub fn report_identity_available() -> bool {
    report_identity().is_some()
}

/// Returns to HBC when the Wiiload run requests it after a report.
#[cfg(feature = "agent-report")]
pub fn restart_after_agent_report_if_requested() {
    if restart_after_probe_requested() {
        restart_to_hbc();
    }
}

/// Sends an application log and its current GX framebuffer through the
/// Wiiload agent report listener. The caller supplies the real XFB bytes from
/// its displayed frame. The timing fields report elapsed run time, scan count,
/// and allocator use. Frame-latency fields stay zero because this report does
/// not sample frame latency.
#[cfg(feature = "agent-report")]
pub fn send_agent_framebuffer_report(
    log: &[u8],
    framebuffer: &[u8],
    width: u32,
    height: u32,
    elapsed_us: u64,
    frame_count: u32,
    heap_used_bytes: u32,
) -> Result<(), i32> {
    send_agent_framebuffer_report_kind(
        log,
        framebuffer,
        width,
        height,
        AgentPerformance {
            elapsed_us,
            frame_count,
            heap_used_bytes,
            ..AgentPerformance::default()
        },
        false,
    )
}

#[cfg(feature = "agent-report")]
fn send_agent_framebuffer_report_kind(
    log: &[u8],
    framebuffer: &[u8],
    width: u32,
    height: u32,
    metrics: AgentPerformance,
    checkpoint: bool,
) -> Result<(), i32> {
    use ogc_network as network;

    let identity = report_identity().ok_or(-1)?;
    let ReportIdentity {
        run_id, build_id, ..
    } = identity;
    if width == 0
        || height == 0
        || log.len() > 4 * 1024 * 1024
        || framebuffer.len() > 16 * 1024 * 1024
    {
        return Err(network::SOCKET_ERROR);
    }
    let sent = (|| {
        let socket = ensure_agent_report_socket()?;
        let mut header = [0u8; 156];
        header[..4].copy_from_slice(b"WIR2");
        header[4..6].copy_from_slice(&2u16.to_be_bytes());
        // The host saves checkpoint screenshots and waits for the final summary.
        header[6..8].copy_from_slice(
            &report_send::flags(
                checkpoint,
                AGENT_SCREEN_CAPTURE_TRUNCATED.load(Ordering::Acquire),
            )
            .to_be_bytes(),
        );
        header[8..12].copy_from_slice(&(log.len() as u32).to_be_bytes());
        header[12..16].copy_from_slice(&(framebuffer.len() as u32).to_be_bytes());
        header[16..20].copy_from_slice(&width.to_be_bytes());
        header[20..24].copy_from_slice(&height.to_be_bytes());
        header[24..28].copy_from_slice(&36u32.to_be_bytes());
        copy_ascii_field(&mut header[28..92], run_id);
        copy_ascii_field(&mut header[92..156], build_id);
        let performance = encode_performance(metrics);
        let started = system::time();
        let sent = report_write_all_result(socket, &header, started)
            .and_then(|()| report_write_all_result(socket, log, started))
            .and_then(|()| report_write_all_result(socket, framebuffer, started))
            .and_then(|()| report_write_all_result(socket, &performance, started));
        if !checkpoint || sent.is_err() {
            finish_agent_report_socket(sent)
        } else {
            sent
        }
    })();

    if checkpoint {
        return sent;
    }
    let cleanup_notice =
        screen_write(b"INFO report socket closed; network cleanup starting\n").map(|_| ());
    let cleanup = deinit_agent_report_network();
    let complete_notice = match cleanup {
        Ok(()) => screen_write(b"INFO report network cleanup complete\n").map(|_| ()),
        Err(error) => {
            screen_report_error("network cleanup", error);
            Ok(())
        }
    };
    sent.and(cleanup).and(cleanup_notice).and(complete_notice)
}

#[cfg(feature = "agent-report")]
fn ensure_agent_report_socket() -> Result<i32, i32> {
    let current = AGENT_REPORT_SOCKET.load(Ordering::Acquire);
    if current >= 0 {
        return Ok(current);
    }
    report_send::retry_ios_timeout(ensure_agent_report_socket_once, wait_vsync)
}

#[cfg(feature = "agent-report")]
fn ensure_agent_report_socket_once() -> Result<i32, i32> {
    use ogc_network as network;

    let current = AGENT_REPORT_SOCKET.load(Ordering::Acquire);
    if current >= 0 {
        return Ok(current);
    }
    ensure_agent_report_network()?;
    let (host, port) = report_target().ok_or(-1)?;
    let socket = network::socket(network::PF_INET, network::SOCK_STREAM, 0);
    if socket < 0 {
        return Err(socket);
    }
    let configured = (|| {
        let addr = network::SockaddrIn {
            sin_len: core::mem::size_of::<network::SockaddrIn>() as u8,
            sin_family: network::AF_INET as u8,
            sin_port: port.to_be(),
            sin_addr: network::InAddr {
                s_addr: network::ip4_addr(host[0], host[1], host[2], host[3]),
            },
            sin_zero: [0; 8],
        };
        let connected = network::connect_addr(socket, &addr);
        if connected < 0 {
            return Err(connected);
        }
        set_report_tcp_nodelay(socket)?;
        let status = network::fcntl(socket, network::sys::F_SETFL, network::O_NONBLOCK);
        if status < 0 { Err(status) } else { Ok(()) }
    })();
    if let Err(error) = configured {
        return close_report_socket(socket, Err(error)).map(|()| socket);
    }
    // Probe reports are sent in order from the main probe thread.
    AGENT_REPORT_SOCKET.store(socket, Ordering::Release);
    Ok(socket)
}

#[cfg(feature = "agent-report")]
fn finish_agent_report_socket(operation: Result<(), i32>) -> Result<(), i32> {
    let socket = AGENT_REPORT_SOCKET.swap(-1, Ordering::AcqRel);
    if socket < 0 {
        return operation;
    }
    close_report_socket(socket, operation)
}

#[cfg(feature = "agent-report")]
fn ensure_agent_report_network() -> Result<(), i32> {
    use ogc_network as network;

    if AGENT_REPORT_NETWORK_READY.load(Ordering::Acquire) {
        return Ok(());
    }
    // Wiiload launches this DOL from HBC with an active IOS network service.
    // `attach_active` waits for a nonzero host address, so a second DHCP
    // configuration request is not needed here.
    let initialized = network::attach_active();
    if initialized < 0 {
        return Err(initialized);
    }
    AGENT_REPORT_NETWORK_READY.store(true, Ordering::Release);
    Ok(())
}

#[cfg(feature = "agent-report")]
fn deinit_agent_report_network() -> Result<(), i32> {
    if !AGENT_REPORT_NETWORK_READY.load(Ordering::Acquire) {
        return Ok(());
    }
    let result = ogc_network::deinit();
    if result < 0 {
        Err(result)
    } else {
        AGENT_REPORT_NETWORK_READY.store(false, Ordering::Release);
        Ok(())
    }
}

#[cfg(feature = "agent-report")]
fn report_write_all_result(socket: i32, bytes: &[u8], started: u64) -> Result<(), i32> {
    use ogc_network as network;
    let timeout_ticks = u64::from(video::timer_clock_hz()) * 60;
    report_send::write_all(
        bytes,
        || system::time().wrapping_sub(started) >= timeout_ticks,
        |bytes| {
            let mut descriptors = [network::PollFd {
                fd: socket,
                events: network::POLLOUT,
                revents: 0,
            }];
            let ready = network::poll_slice(&mut descriptors, 100);
            if ready < 0 {
                return Err(ready);
            }
            if descriptors[0].revents & (network::POLLERR | network::POLLHUP | network::POLLNVAL)
                != 0
            {
                return Err(network::SOCKET_ERROR);
            }
            if ready == 0 || descriptors[0].revents & network::POLLOUT == 0 {
                return Ok(None);
            }
            let written = network::write_slice(socket, bytes);
            if matches!(written, -6 | -27) {
                // IOS EAGAIN or EINTR.
                return Ok(None);
            }
            if written < 0 {
                return Err(written);
            }
            Ok(Some(written as usize))
        },
    )
    .map_err(|error| match error {
        report_send::WriteError::Operation(code) => code,
        report_send::WriteError::Deadline => -76, // IOS ETIMEDOUT.
        report_send::WriteError::NoProgress | report_send::WriteError::InvalidCount => {
            network::SOCKET_ERROR
        }
    })
}

#[cfg(feature = "agent-report")]
fn close_report_socket(socket: i32, operation: Result<(), i32>) -> Result<(), i32> {
    let closed = ogc_network::close(socket);
    match (operation, closed) {
        (Err(error), cleanup) => {
            if cleanup < 0 {
                screen_report_error("socket cleanup", cleanup);
            }
            Err(error)
        }
        (Ok(()), status) if status < 0 => Err(status),
        (Ok(()), _) => Ok(()),
    }
}

#[cfg(feature = "agent-report")]
fn set_report_tcp_nodelay(socket: i32) -> Result<(), i32> {
    let no_delay = 1i32;
    let result = unsafe {
        ogc_network::setsockopt(
            socket,
            ogc_network::IPPROTO_TCP,
            ogc_network::TCP_NODELAY,
            (&no_delay as *const i32).cast(),
            core::mem::size_of::<i32>() as ogc_network::SockLenT,
        )
    };
    if result < 0 { Err(result) } else { Ok(()) }
}

#[cfg(not(feature = "agent-report"))]
pub fn send_agent_framebuffer_report(
    _log: &[u8],
    _framebuffer: &[u8],
    _width: u32,
    _height: u32,
    _elapsed_us: u64,
    _frame_count: u32,
    _heap_used_bytes: u32,
) -> Result<(), i32> {
    Err(-1)
}

/// Sends the newly captured screen bytes to the host before a startup phase
/// that can block. The checkpoint is best effort; it does not change probe
/// control flow when the host is unavailable.
pub fn send_agent_progress(stage: u32) {
    #[cfg(feature = "agent-report")]
    send_agent_progress_inner(stage);
    #[cfg(not(feature = "agent-report"))]
    let _ = stage;
}

#[cfg(feature = "agent-report")]
fn send_agent_progress_inner(stage: u32) {
    let identity = report_identity();
    let (run_id, build_id, identity_unavailable) = match identity {
        Some(identity) => (identity.run_id, identity.build_id, false),
        None => (&[][..], &[][..], true),
    };
    let text = captured_screen_text();
    let start = AGENT_PROGRESS_CURSOR
        .load(Ordering::Acquire)
        .min(text.len());
    let delta = &text[start..];
    let run_id_len = run_id.len();
    if run_id_len > 64
        || (!identity_unavailable && build_id.len() != 64)
        || delta.len() > AGENT_SCREEN_CAPTURE_LIMIT
    {
        return;
    }

    let socket = match ensure_agent_report_socket() {
        Ok(socket) => socket,
        Err(error) => {
            screen_report_error("progress connection", error);
            return;
        }
    };

    let sequence = AGENT_PROGRESS_SEQUENCE.fetch_add(1, Ordering::AcqRel);
    let mut header = [0u8; 24];
    header[..4].copy_from_slice(b"WIP1");
    header[4..6].copy_from_slice(&1u16.to_be_bytes());
    let flags = u16::from(AGENT_SCREEN_CAPTURE_TRUNCATED.load(Ordering::Acquire))
        | (u16::from(identity_unavailable) << 1);
    header[6..8].copy_from_slice(&flags.to_be_bytes());
    header[8..12].copy_from_slice(&sequence.to_be_bytes());
    header[12..16].copy_from_slice(&stage.to_be_bytes());
    header[16..18].copy_from_slice(&(run_id_len as u16).to_be_bytes());
    header[18..20].copy_from_slice(&(build_id.len() as u16).to_be_bytes());
    header[20..24].copy_from_slice(&(delta.len() as u32).to_be_bytes());
    let started = system::time();
    let sent = report_write_all_result(socket, &header, started)
        .and_then(|()| report_write_all_result(socket, run_id, started))
        .and_then(|()| report_write_all_result(socket, build_id, started))
        .and_then(|()| report_write_all_result(socket, delta, started));
    // Keep the initialized stack active through the final WIR2 report.
    match sent {
        Ok(()) => AGENT_PROGRESS_CURSOR.store(text.len(), Ordering::Release),
        Err(error) => {
            if let Err(error) = finish_agent_report_socket(Err(error)) {
                screen_report_error("progress write", error);
            }
        }
    }
}

#[cfg(not(feature = "agent-report"))]
pub fn report_identity_available() -> bool {
    false
}

#[cfg(feature = "agent-report")]
fn agent_arg(prefix: &[u8]) -> Option<&'static [u8]> {
    let argc = AGENT_ARGC.load(Ordering::Acquire);
    if argc == 0 || argc > AGENT_ARGUMENT_COUNT_LIMIT {
        return None;
    }

    let storage = if AGENT_USE_SYSTEM_ARGUMENTS.load(Ordering::Acquire) {
        core::ptr::addr_of!(SYSTEM_ARGUMENT_STORAGE)
    } else {
        core::ptr::addr_of!(AGENT_ARGUMENT_STORAGE)
    };
    for index in 0..argc {
        let arg = unsafe { stored_agent_argument(storage, index) }?;
        if let Some(value) = arg.strip_prefix(prefix) {
            return Some(value);
        }
    }
    None
}

#[cfg(feature = "agent-report")]
unsafe fn stored_agent_argument(
    storage: *const AgentArgumentStorage,
    index: usize,
) -> Option<&'static [u8]> {
    if storage.is_null() || index >= AGENT_ARGUMENT_COUNT_LIMIT {
        return None;
    }
    let ranges = unsafe { core::ptr::addr_of!((*storage).ranges).cast::<AgentArgumentRange>() };
    let bytes = unsafe { core::ptr::addr_of!((*storage).bytes).cast::<u8>() };
    let range = unsafe { ranges.add(index).read() };
    if range.start > AGENT_ARGUMENT_BYTES_LIMIT
        || range.length > AGENT_ARGUMENT_BYTES_LIMIT.saturating_sub(range.start)
    {
        return None;
    }
    Some(unsafe { core::slice::from_raw_parts(bytes.add(range.start), range.length) })
}

#[cfg(feature = "agent-report")]
unsafe fn storage_contains_argument(
    storage: *const AgentArgumentStorage,
    count: usize,
    prefix: &[u8],
) -> bool {
    (0..count).any(|index| {
        unsafe { stored_agent_argument(storage, index) }
            .is_some_and(|argument| argument.starts_with(prefix))
    })
}

#[cfg(feature = "agent-report")]
unsafe fn copy_agent_arguments(
    storage: *mut AgentArgumentStorage,
    argc: i32,
    argv: *mut *mut u8,
) -> Option<usize> {
    let count = usize::try_from(argc).ok()?;
    if storage.is_null() || count == 0 || count > AGENT_ARGUMENT_COUNT_LIMIT || argv.is_null() {
        return None;
    }

    unsafe { storage.write(AgentArgumentStorage::empty()) };
    let mut used = 0usize;
    for index in 0..count {
        let arg = unsafe { argv.add(index).read() };
        if arg.is_null() || used >= AGENT_ARGUMENT_BYTES_LIMIT {
            return None;
        }
        let length =
            unsafe { bounded_c_string_len(arg, AGENT_ARGUMENT_BYTES_LIMIT.saturating_sub(used)) }?;
        if length > AGENT_ARGUMENT_BYTES_LIMIT - used {
            return None;
        }
        let bytes = unsafe { core::ptr::addr_of_mut!((*storage).bytes).cast::<u8>() };
        unsafe { core::ptr::copy(arg, bytes.add(used), length) };
        let ranges =
            unsafe { core::ptr::addr_of_mut!((*storage).ranges).cast::<AgentArgumentRange>() };
        unsafe {
            ranges.add(index).write(AgentArgumentRange {
                start: used,
                length,
            })
        };
        used += length;
    }
    Some(count)
}

#[cfg(feature = "agent-report")]
#[cfg(test)]
fn find_agent_arg<'a>(args: impl IntoIterator<Item = &'a [u8]>, prefix: &[u8]) -> Option<&'a [u8]> {
    const MAX_AGENT_ARGC: usize = 64;
    const MAX_AGENT_ARGUMENT_BYTES: usize = 4096;

    for (index, arg) in args.into_iter().enumerate() {
        if index >= MAX_AGENT_ARGC || arg.len() > MAX_AGENT_ARGUMENT_BYTES {
            return None;
        }
        if let Some(value) = arg.strip_prefix(prefix) {
            return Some(value);
        }
    }
    None
}

#[cfg(feature = "agent-report")]
unsafe fn bounded_c_string_len(arg: *const u8, maximum: usize) -> Option<usize> {
    (0..=maximum).find(|&length| {
        // SAFETY: the caller guarantees a valid NUL-terminated C string. This
        // reads through the first NUL or inspects at most `maximum + 1` bytes.
        unsafe { arg.add(length).read() == 0 }
    })
}

#[cfg(feature = "agent-report")]
fn agent_frame_option(prefix: &[u8]) -> Option<u32> {
    let bytes = agent_arg(prefix)?;
    core::str::from_utf8(bytes).ok()?.parse().ok()
}

#[cfg(feature = "agent-report")]
fn recovery_reset_delay_ms() -> Option<u32> {
    agent_frame_option(b"--recovery-reset-ms=").filter(|milliseconds| *milliseconds > 0)
}

#[cfg(feature = "agent-report")]
fn verify_recovery_reset_requested() -> bool {
    agent_arg(b"--verify-recovery-reset").is_some()
}

#[cfg(feature = "agent-report")]
fn restart_after_probe_requested() -> bool {
    agent_arg(b"--restart-after-probe").is_some()
}

#[cfg(feature = "agent-report")]
fn return_to_system_menu_requested() -> bool {
    agent_arg(b"--return-to-system-menu").is_some_and(|suffix| suffix.is_empty())
}

#[cfg(feature = "agent-report")]
fn return_from_main_requested() -> bool {
    agent_arg(b"--return-from-main").is_some()
}

#[cfg(feature = "agent-report")]
pub fn failure_injection_requested() -> bool {
    agent_arg(b"--inject-probe-failure").is_some()
}

#[cfg(not(feature = "agent-report"))]
pub fn failure_injection_requested() -> bool {
    false
}

#[cfg(feature = "agent-report")]
fn restart_to_hbc() -> ! {
    system::sys::sys_exit_to_loader(0)
}

#[cfg(all(test, feature = "agent-report"))]
mod agent_arg_tests {
    use super::{copy_agent_arguments, find_agent_arg};

    #[test]
    fn finds_a_wiiload_argument_from_the_dol_argument_vector() {
        let args: [&[u8]; 3] = [
            b"video_probe.dol",
            b"--report=10.0.0.238:4298",
            b"--run-id=run-1",
        ];
        assert_eq!(
            find_agent_arg(args, b"--report="),
            Some(&b"10.0.0.238:4298"[..])
        );
    }

    #[test]
    fn absent_optional_argument_is_ignored() {
        let args: [&[u8]; 2] = [b"video_probe.dol", b"--run-id=run-1"];
        assert_eq!(find_agent_arg(args, b"--recovery-reset-ms="), None);
    }

    #[test]
    fn rejects_oversized_argument_values() {
        let oversized = alloc::vec![b'a'; 4097];
        assert_eq!(find_agent_arg([oversized.as_slice()], b"--report="), None);
    }

    #[test]
    fn copies_entry_arguments_before_the_startup_storage_can_change() {
        use super::AgentArgumentStorage;

        let mut report = b"--report=10.0.0.238:4298\0".to_vec();
        let mut argv = [report.as_mut_ptr()];
        let mut storage = AgentArgumentStorage::empty();
        let count = unsafe { copy_agent_arguments(&mut storage, 1, argv.as_mut_ptr()) }.unwrap();
        report.fill(0);

        let range = storage.ranges[0];
        let copied = &storage.bytes[range.start..range.start + range.length];
        assert_eq!(count, 1);
        assert_eq!(copied, b"--report=10.0.0.238:4298");
    }
}

#[cfg(feature = "agent-report")]
unsafe extern "C" fn recovery_alarm_callback(_alarm: system::SysWd, _argument: *mut c_void) {
    // SAFETY: This callback is the requested last resort after the configured
    // probe deadline expires. SYS_RESTART is a documented libogc reset mode.
    unsafe { system::reset_system(system::SYS_RESTART as i32, 0, 0) };
}

#[cfg(feature = "agent-report")]
unsafe extern "C" fn watchdog_alarm_callback(_alarm: system::SysWd, _argument: *mut c_void) {
    let timeout_ms = WATCHDOG_TIMEOUT_MS.load(Ordering::Acquire);
    if timeout_ms == 0 {
        return;
    }
    let last_heartbeat = WATCHDOG_LAST_HEARTBEAT_MS.load(Ordering::Acquire);
    if watchdog_now_ms().wrapping_sub(last_heartbeat) >= timeout_ms {
        // Prefer HBC's installed return stub. If the DOL has overwritten it,
        // restart the Wii so Priiloader or the system startup setting can act.
        if system::sys::sys_hbc_reload_stub_present() {
            system::sys::sys_exit_to_loader(0);
        } else {
            // SAFETY: This is the configured last-resort recovery action after
            // the application heartbeat has stopped. SYS_RESTART is documented.
            unsafe { system::reset_system(system::SYS_RESTART as i32, 0, 0) };
        }
    }
}

#[cfg(feature = "agent-report")]
fn arm_recovery_reset(milliseconds: u32) -> Option<system::SysWd> {
    let mut alarm: system::SysWd = 0;
    if system::create_alarm(&mut alarm) != 0 || alarm == 0 {
        return None;
    }
    let time = system::TimeSpec {
        tv_sec: (milliseconds / 1000) as i64,
        tv_nsec: ((milliseconds % 1000) as i32) * 1_000_000,
    };
    // SAFETY: The callback is static, and it does not use the callback argument.
    let result = unsafe {
        system::set_alarm(
            alarm,
            &time,
            Some(recovery_alarm_callback),
            core::ptr::null_mut(),
        )
    };
    if result != 0 {
        let _ = system::remove_alarm(alarm);
        return None;
    }
    unsafe { core::ptr::addr_of_mut!(RECOVERY_ALARM).write(alarm) };
    Some(alarm)
}

#[cfg(feature = "agent-report")]
fn take_recovery_reset() -> Option<system::SysWd> {
    let alarm = unsafe { core::ptr::addr_of_mut!(RECOVERY_ALARM).read() };
    if alarm == 0 { None } else { Some(alarm) }
}

#[cfg(feature = "agent-report")]
unsafe fn probe_reset_progress(stage: u32, priority: u32, result: i32) {
    let record = OGC_PROBE_RESET_PROGRESS_COUNT.fetch_add(1, Ordering::AcqRel) as usize;
    if record >= RESET_PROGRESS_RECORDS {
        return;
    }
    let offset = record * 3;
    OGC_PROBE_RESET_PROGRESS[offset].store(stage, Ordering::Relaxed);
    OGC_PROBE_RESET_PROGRESS[offset + 1].store(priority, Ordering::Relaxed);
    OGC_PROBE_RESET_PROGRESS[offset + 2].store(result as u32, Ordering::Release);
}

#[cfg(feature = "agent-report")]
fn cancel_recovery_reset(alarm: Option<system::SysWd>) {
    let Some(alarm) = alarm else { return };
    let cancelled = system::cancel_alarm(alarm);
    if cancelled < 0 {
        screen_report_error("watchdog cancel", cancelled);
    }
    let removed = system::remove_alarm(alarm);
    if removed < 0 {
        screen_report_error("watchdog remove", removed);
        return;
    }
    unsafe { core::ptr::addr_of_mut!(RECOVERY_ALARM).write(0) };
}

#[cfg(feature = "agent-report")]
fn copy_ascii_field(destination: &mut [u8], value: &[u8]) {
    destination.fill(0);
    let count = value.len().min(destination.len());
    destination[..count].copy_from_slice(&value[..count]);
}

#[cfg(feature = "agent-report")]
fn encode_performance(metrics: AgentPerformance) -> [u8; 36] {
    let mut bytes = [0u8; 36];
    bytes[..8].copy_from_slice(&metrics.elapsed_us.to_be_bytes());
    for (index, value) in [
        metrics.frame_count,
        metrics.heap_used_bytes,
        metrics.warmup_frames,
        metrics.sample_frames,
        metrics.frame_time_average_us,
        metrics.frame_time_min_us,
        metrics.frame_time_max_us,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 8 + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_be_bytes());
    }
    bytes
}

#[cfg(feature = "dolphin-automation")]
fn dolphin_transport_write(bytes: &[u8]) {
    if let Some(error) = send_transport_bytes(bytes, |byte| unsafe {
        ogc_usbgecko::sendbuffer_safe_ex(1, &byte as *const u8 as *const _, 1, 1000)
    }) {
        DOLPHIN_TRANSPORT_ERROR.store(error, Ordering::Release);
    }
}

#[cfg(any(feature = "dolphin-automation", test))]
fn send_transport_bytes(bytes: &[u8], mut send_byte: impl FnMut(u8) -> i32) -> Option<i32> {
    let mut first_error = None;
    for byte in bytes {
        let sent = send_byte(*byte);
        if sent != 1 && first_error.is_none() {
            first_error = Some(if sent < 0 { sent } else { -1 });
        }
    }
    first_error
}

#[cfg(test)]
mod dolphin_transport_tests {
    use super::send_transport_bytes;

    #[test]
    fn transport_sends_all_bytes_and_reports_the_first_short_send() {
        let mut transmitted = Vec::new();
        let error = send_transport_bytes(b"screen data", |byte| {
            transmitted.push(byte);
            if byte == b'c' { 0 } else { 1 }
        });

        assert_eq!(transmitted, b"screen data");
        assert_eq!(error, Some(-1));
    }

    #[test]
    fn transport_reports_success_only_when_every_byte_is_sent() {
        let mut transmitted = Vec::new();
        let error = send_transport_bytes(b"capture", |byte| {
            transmitted.push(byte);
            1
        });

        assert_eq!(transmitted, b"capture");
        assert_eq!(error, None);
    }
}

struct LineBuf {
    bytes: [u8; LINE_BUF_CAPACITY],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; LINE_BUF_CAPACITY],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl fmt::Write for LineBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let remaining = self.bytes.len().saturating_sub(self.len);
        if s.len() > remaining {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}

fn open_log_file(policy: LogFilePolicy) -> Option<LogFile> {
    match policy {
        LogFilePolicy::Disabled => None,
        LogFilePolicy::IsfsOnly => open_isfs_log_file().map(LogFile::Isfs),
        LogFilePolicy::PreferMounted => open_mounted_log_file()
            .map(LogFile::Mounted)
            .or_else(|| open_isfs_log_file().map(LogFile::Isfs)),
        LogFilePolicy::MountedOnly => open_mounted_log_file().map(LogFile::Mounted),
    }
}

#[cfg(feature = "mounted-log")]
// This only succeeds when some filesystem layer has already registered a
// devoptab for `sd:`/`usb:` style paths. The safe logger keeps this behind an
// explicit feature flag and policy opt-in. Advanced callers can still use the
// lower-level `fat::mount_*` and `ogc_sysbase::DeviceFile` APIs directly.
fn open_mounted_log_file() -> Option<MountedLogFile> {
    derive_argv0_bytes()
        .and_then(derive_mounted_path_from_bytes)
        .and_then(|candidate| candidate.try_open())
}

#[cfg(not(feature = "mounted-log"))]
fn open_mounted_log_file() -> Option<MountedLogFile> {
    None
}

fn open_isfs_log_file() -> Option<filesystem::IsfsFile> {
    if filesystem::initialize() != filesystem::ISFS_OK {
        return None;
    }

    let path_cstr = unsafe { CStr::from_bytes_with_nul_unchecked(ISFS_FALLBACK_LOG_PATH) };
    let file = match filesystem::open(path_cstr, filesystem::ISFS_OPEN_RW) {
        Ok(file) => file,
        Err(_) => {
            let _ = filesystem::create_file(path_cstr, 0, 0, 0, 0);
            filesystem::open(path_cstr, filesystem::ISFS_OPEN_RW).ok()?
        }
    };

    let _ = file.seek(0, 2);
    Some(file)
}

#[cfg(feature = "mounted-log")]
fn derive_argv0_bytes() -> Option<&'static [u8]> {
    let argv = unsafe { sys::current_system_argv().as_ref()? };
    if argv.argc <= 0 || argv.argv.is_null() {
        return None;
    }
    let arg0 = unsafe { *argv.argv };
    if arg0.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(arg0) }.to_bytes())
}

#[cfg(feature = "mounted-log")]
fn derive_mounted_path_from_bytes(bytes: &[u8]) -> Option<PathCandidate> {
    if !bytes.contains(&b':') {
        return None;
    }
    build_path_from_dir(bytes)
}

#[cfg(feature = "mounted-log")]
fn build_path_from_dir(bytes: &[u8]) -> Option<PathCandidate> {
    let slash = bytes.iter().rposition(|&b| b == b'/')?;
    let prefix_len = slash + 1;
    let app_dir = unsafe { PROBE_APP_DIR }.unwrap_or("probe").as_bytes();
    if app_dir.is_empty()
        || !app_dir
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }
    let total_len = prefix_len + LOG_FILE_PREFIX.len() + app_dir.len() + LOG_FILE_SUFFIX.len() + 1;
    if total_len > FILE_PATH_CAPACITY {
        return None;
    }
    let mut path = [0u8; FILE_PATH_CAPACITY];
    path[..prefix_len].copy_from_slice(&bytes[..prefix_len]);
    let mut cursor = prefix_len;
    path[cursor..cursor + LOG_FILE_PREFIX.len()].copy_from_slice(LOG_FILE_PREFIX);
    cursor += LOG_FILE_PREFIX.len();
    path[cursor..cursor + app_dir.len()].copy_from_slice(app_dir);
    cursor += app_dir.len();
    path[cursor..cursor + LOG_FILE_SUFFIX.len()].copy_from_slice(LOG_FILE_SUFFIX);
    cursor += LOG_FILE_SUFFIX.len();
    path[cursor] = 0;
    Some(PathCandidate {
        bytes: path,
        len: total_len,
    })
}

#[cfg(feature = "mounted-log")]
struct PathCandidate {
    bytes: [u8; FILE_PATH_CAPACITY],
    len: usize,
}

#[cfg(feature = "mounted-log")]
impl PathCandidate {
    fn try_open(self) -> Option<MountedLogFile> {
        if !self.is_safe_mounted_log_path() {
            return None;
        }
        let path_cstr = unsafe { CStr::from_bytes_with_nul_unchecked(&self.bytes[..self.len]) };
        MountedLogFile::open(path_cstr)
    }

    fn is_safe_mounted_log_path(&self) -> bool {
        let path = &self.bytes[..self.len.saturating_sub(1)];
        (path.starts_with(b"sd:/") || path.starts_with(b"usb:/"))
            && path.ends_with(LOG_FILE_SUFFIX)
            && path
                .rsplit(|byte| *byte == b'/')
                .next()
                .is_some_and(|name| name.starts_with(LOG_FILE_PREFIX))
            && !path.windows(2).any(|window| window == b"..")
            && !path.windows(2).any(|window| window == b"//")
            && !path.contains(&b'\\')
            && path.iter().all(|byte| matches!(byte, 0x20..=0x7e))
    }
}
