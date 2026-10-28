#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(not(target_arch = "powerpc"))]
extern crate std;

use core::ffi::c_void;
use core::fmt;
use core::ptr::{addr_of, addr_of_mut, read_unaligned, write_unaligned};
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicU8, AtomicU16, AtomicU32, AtomicUsize, Ordering,
};

pub use ogc_system_sys as sys;
pub mod startup;
pub use sys::{
    AlarmCallback, Bool, DietPrintFn, GxrModeObj, LwpNode, PowerCallback, ResetCallback,
    ResetFunction, SYS_BASE_CACHED, SYS_BASE_UNCACHED, SYS_FONTSIZE_ANSI, SYS_FONTSIZE_SJIS,
    SYS_HOTRESET, SYS_LANG_DUTCH, SYS_LANG_ENGLISH, SYS_LANG_FRENCH, SYS_LANG_GERMAN,
    SYS_LANG_ITALIAN, SYS_LANG_SPANISH, SYS_POWEROFF, SYS_POWEROFF_IDLE, SYS_POWEROFF_STANDBY,
    SYS_PRE_MAIN_ESTM, SYS_PROTECTCHAN0, SYS_PROTECTCHAN1, SYS_PROTECTCHAN2, SYS_PROTECTCHAN3,
    SYS_PROTECTCHANMAX, SYS_PROTECTNONE, SYS_PROTECTRDWR, SYS_PROTECTREAD, SYS_PROTECTWRITE,
    SYS_RESTART, SYS_RETURNTOMENU, SYS_SHUTDOWN, SYS_SOUND_MONO, SYS_SOUND_STEREO, SYS_VIDEO_MPAL,
    SYS_VIDEO_NTSC, SYS_VIDEO_PAL, SYS_WD_NULL, SysFontHeader, SysResetInfo, SysSram, SysSramEx,
    SysWd, TimeSpec,
};

#[macro_export]
macro_rules! sys_report {
    ($($arg:tt)*) => {{
        $crate::report(core::format_args!($($arg)*))
    }};
}

const FRAMEBUFFER_ALIGNMENT: usize = 32;
#[cfg(target_arch = "powerpc")]
const FRAMEBUFFER_POOL_BYTES: usize = 2 * 1024 * 1024;

const SRAM_VIDEO_MODE_BITS: u8 = 0x03;
const SRAM_SOUND_MODE_BIT: u8 = 0x04;
const SRAM_PROGSCAN_BIT: u8 = 0x80;

static LAST_FRAMEBUFFER_BYTES: AtomicUsize = AtomicUsize::new(0);
static ARENA1_LO: AtomicUsize = AtomicUsize::new(0);
static ARENA1_HI: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(gamecube))]
static ARENA2_LO: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(gamecube))]
static ARENA2_HI: AtomicUsize = AtomicUsize::new(0);
static COUNTER_BIAS: AtomicU32 = AtomicU32::new(0);
static DISPLAY_OFFSET_H: AtomicU8 = AtomicU8::new(0);
static EU_RGB60: AtomicU8 = AtomicU8::new(0);
static LANGUAGE: AtomicU8 = AtomicU8::new(0);
static PROGRESSIVE_SCAN: AtomicU8 = AtomicU8::new(0);
static SOUND_MODE: AtomicU8 = AtomicU8::new(0);
static VIDEO_MODE: AtomicU8 = AtomicU8::new(0);
static WIRELESS_ID0: AtomicU16 = AtomicU16::new(0);
static WIRELESS_ID1: AtomicU16 = AtomicU16::new(0);
static WIRELESS_ID2: AtomicU16 = AtomicU16::new(0);
static WIRELESS_ID3: AtomicU16 = AtomicU16::new(0);
static GBS_MODE: AtomicU16 = AtomicU16::new(0);
static STDIO_REPORT_USE_STDOUT: AtomicBool = AtomicBool::new(false);
static LAST_REPORT_SUCCEEDED: AtomicBool = AtomicBool::new(true);
static LAST_CONFIG_WRITE_RESULT: AtomicI32 = AtomicI32::new(0);
static RESET_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
static POWER_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
static DIET_PRINT_FN_PTR: AtomicUsize = AtomicUsize::new(0);
static LAST_ALARM_PTR: AtomicUsize = AtomicUsize::new(0);
static LAST_ALARM_CB_PTR: AtomicUsize = AtomicUsize::new(0);
static LAST_PERIODIC_ALARM_CB_PTR: AtomicUsize = AtomicUsize::new(0);
static FONT_ENCODING: AtomicU32 = AtomicU32::new(0);
static RESET_BUTTON_DOWN: AtomicU32 = AtomicU32::new(0);
static HOLLYWOOD_REVISION: AtomicU32 = AtomicU32::new(0);
static LAST_TIME_READ_LO: AtomicU32 = AtomicU32::new(0);
static LAST_TIME_READ_HI: AtomicU32 = AtomicU32::new(0);
static LAST_SRAM_SYNC_CALLED: AtomicBool = AtomicBool::new(false);
static LAST_SRAM_SYNC_RESULT: AtomicBool = AtomicBool::new(false);
static LAST_SRAM_UNLOCK_RESULT: AtomicBool = AtomicBool::new(false);
static LAST_SRAM_EX_UNLOCK_RESULT: AtomicBool = AtomicBool::new(false);
static SYSTEM_INIT_CALLED: AtomicBool = AtomicBool::new(false);
static SYSTEM_PRE_MAIN_CALLED: AtomicBool = AtomicBool::new(false);
static SYSTEM_MAIN_LOOP_CALLED: AtomicBool = AtomicBool::new(false);
static SYSTEM_MAIN_LOOP_RESULT: AtomicBool = AtomicBool::new(false);
static LAST_RESET_SYSTEM_CALLED: AtomicBool = AtomicBool::new(false);
static LAST_RESET_SYSTEM_RESET: AtomicU32 = AtomicU32::new(0);
static LAST_RESET_SYSTEM_CODE: AtomicU32 = AtomicU32::new(0);
static LAST_RESET_SYSTEM_FORCE_MENU: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct FramebufferPool([u8; FRAMEBUFFER_POOL_BYTES]);

#[cfg(target_arch = "powerpc")]
static mut FRAMEBUFFER_POOL: FramebufferPool = FramebufferPool([0; FRAMEBUFFER_POOL_BYTES]);
#[cfg(target_arch = "powerpc")]
static FRAMEBUFFER_POOL_NEXT: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SystemInitStep {
    PreInit,
    InstallPanicHandler,
    MarkInitialized,
    LowMemInit,
    IpcBufferInit,
    LwpWorkspaceInit,
    ResetQueueInit,
    ExiInit,
    SramInit,
    SiInit,
    DspBootstrap,
    MemProtectInit,
    InitCallbacks,
    IpcClientInit,
    ResetIrqInit,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SystemPreMainStep {
    CloseIosFdRange,
    LoadStartupIos,
    InitializeIosSubsystems,
    RegisterStmEventHandler,
    ConfInit,
    WiiInitialize,
    SetBootTime,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResetSystemTerminal {
    HotReset,
    Reboot,
    ReturnToMenu,
    Shutdown,
    ShutdownToStandby,
    ShutdownToIdle,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MainLoopExitReason {
    AlreadyDone,
    ResetButton,
    PowerButton,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MainLoopDecision {
    Continue,
    Exit(MainLoopExitReason),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ResetSystemPlan {
    pub terminal: Option<ResetSystemTerminal>,
    pub sets_boot_ipl: bool,
    pub disables_pad_recalibration: bool,
    pub clears_active_input_list: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResetSystemAction {
    DspShutdown,
    DisablePadRecalibration,
    ClearActiveInputList,
    RunResetCallbacksBeforeShutdown,
    SetBootIplFlag,
    ShutdownIrqs,
    RunResetCallbacksAfterShutdown,
    DisableL2Cache,
    InvokeGcHotReset,
    InvokeGcReboot,
    InvokeWiiReboot,
    InvokeWiiShutdownToStandby,
    InvokeWiiShutdownToIdle,
    InvokeWiiReturnToMenu,
    ShutdownIosSubsystems,
    ClearLowMemory,
    RestorePadRecalibration,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResetSystemStep {
    DspShutdown,
    DisablePadRecalibration,
    ClearActiveInputList,
    RunResetCallbacksBeforeShutdown,
    SetBootIplFlag,
    InvokeTerminal,
    ShutdownIosSubsystems,
    ShutdownIrqs,
    RunResetCallbacksAfterShutdown,
    DisableL2Cache,
    ClearLowMemory,
    RestorePadRecalibration,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PowerButtonState {
    Idle,
    ResetPressed,
    PowerPressed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StmEvent {
    Reset,
    Power,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SystemRuntimeState {
    pub is_wii: bool,
    pub main_loop_done: bool,
    pub power_button_state: PowerButtonState,
    pub power_enable_time: u64,
}

impl SystemRuntimeState {
    pub const fn new(is_wii: bool) -> Self {
        Self {
            is_wii,
            main_loop_done: false,
            power_button_state: PowerButtonState::Idle,
            power_enable_time: 0,
        }
    }

    pub fn note_stm_event(&mut self, event: StmEvent, reset_button_down: bool) {
        match event {
            StmEvent::Reset
                if reset_button_down && self.power_button_state == PowerButtonState::Idle =>
            {
                self.power_button_state = PowerButtonState::ResetPressed;
            }
            StmEvent::Power if self.power_button_state == PowerButtonState::Idle => {
                self.power_button_state = PowerButtonState::PowerPressed;
            }
            _ => {}
        }
    }

    pub fn main_loop_step(&mut self, reset_pressed: bool) -> MainLoopDecision {
        let decision = main_loop_decision(
            self.is_wii,
            self.main_loop_done,
            reset_pressed,
            self.power_button_state != PowerButtonState::Idle,
        );
        if matches!(decision, MainLoopDecision::Exit(_)) {
            self.main_loop_done = true;
        }
        decision
    }

    pub fn power_callback_attempt(&mut self, now: u64, cooldown_ticks: u64) -> bool {
        let due = power_callback_due(now, self.power_enable_time);
        self.power_enable_time = next_power_enable_time(now, cooldown_ticks);
        due
    }

    pub fn reset_plan(
        &self,
        reset: u32,
        force_menu: bool,
        shutdown_mode_idle: bool,
    ) -> ResetSystemPlan {
        reset_system_plan(self.is_wii, reset, force_menu, shutdown_mode_idle)
    }
}

pub fn init() {
    SYSTEM_INIT_CALLED.store(true, Ordering::Release);
    unsafe { sys::sys_init() }
}

/// Initialize the system and return any recorded production startup failure.
/// A call while initialization is in progress returns NotCompleted.
pub fn init_checked() -> Result<(), startup::InitError> {
    init();
    let status = sys::snapshot_startup();
    startup::validate(status.init_completed, status.init_failed_stage)
}

pub use sys::{ClockError, ClockSnapshot, RtcStage};

/// Elapsed time from the stable Wii timebase origin. No RTC is required.
pub fn monotonic_time() -> Result<core::time::Duration, ClockError> {
    sys::sys_monotonic_time()
}

/// UTC time from the actual RTC sample, console bias, and explicit UTC offset.
pub fn wall_time() -> Result<core::time::Duration, ClockError> {
    sys::sys_wall_time()
}

/// Set seconds east of UTC for the console's local calendar in application
/// memory. Refresh this setting when the local UTC offset changes.
pub fn set_clock_utc_offset(seconds: i32) -> Result<(), ClockError> {
    sys::sys_set_clock_utc_offset(seconds)
}

/// Refresh the real RTC and counter bias after console configuration is ready.
pub fn synchronize_clock() -> Result<(), ClockError> {
    sys::sys_synchronize_clock()
}

pub fn clock_snapshot() -> ClockSnapshot {
    sys::sys_clock_snapshot()
}

pub fn runtime_fp_disabled_stage() -> u32 {
    sys::sys_runtime_fp_disabled_stage()
}

/// Exit the application and return to the Homebrew Channel loader when it is
/// available. If the loader stub is absent, use the system restart fallback.
pub fn exit(status: i32) -> ! {
    sys::sys_exit_to_loader(status)
}

pub fn pre_main() {
    SYSTEM_PRE_MAIN_CALLED.store(true, Ordering::Release);
    unsafe { sys::sys_pre_main() }
}

pub fn main_loop() -> bool {
    SYSTEM_MAIN_LOOP_CALLED.store(true, Ordering::Release);
    let result = unsafe { sys::sys_main_loop() != 0 };
    SYSTEM_MAIN_LOOP_RESULT.store(result, Ordering::Release);
    result
}

pub fn main_loop_decision(
    is_wii: bool,
    already_done: bool,
    reset_pressed: bool,
    power_state_active: bool,
) -> MainLoopDecision {
    if already_done {
        return MainLoopDecision::Exit(MainLoopExitReason::AlreadyDone);
    }

    if is_wii {
        if power_state_active {
            MainLoopDecision::Exit(MainLoopExitReason::PowerButton)
        } else {
            MainLoopDecision::Continue
        }
    } else if reset_pressed {
        MainLoopDecision::Exit(MainLoopExitReason::ResetButton)
    } else {
        MainLoopDecision::Continue
    }
}

/// # Safety
///
/// `rmode` must point to a valid video mode object. The returned framebuffer
/// contents are uninitialized; clear them before reading or displaying them.
/// The buffer is reserved for the runtime and must not be passed to C `free`.
pub unsafe fn allocate_framebuffer(rmode: *mut GxrModeObj) -> *mut c_void {
    unsafe {
        if rmode.is_null() {
            return core::ptr::null_mut();
        }
        let size = ogc_video::get_frame_buffer_size(rmode) as usize;
        let ptr = sys::sys_allocate_framebuffer(rmode);
        if !ptr.is_null() {
            LAST_FRAMEBUFFER_BYTES.store(size, Ordering::Release);
            return ptr;
        }
        let ptr = framebuffer_pool_alloc(size);
        if !ptr.is_null() {
            LAST_FRAMEBUFFER_BYTES.store(size, Ordering::Release);
        }
        ptr
    }
}

/// # Safety
///
/// Memory protection programming is hardware-sensitive and must only be used
/// with valid channel/address/range values.
pub unsafe fn protect_range(chan: u32, addr: *mut c_void, bytes: u32, cntrl: u32) {
    unsafe {
        let _ = (chan, addr, bytes, cntrl);
        sys::sys_protect_range(chan, addr, bytes, cntrl)
    }
}

pub fn last_framebuffer_bytes() -> usize {
    LAST_FRAMEBUFFER_BYTES.load(Ordering::Acquire)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct FramebufferSnapshot {
    pub last_bytes: usize,
    pub last_request_rmode: usize,
    pub last_request_bytes: u32,
}

pub fn snapshot_framebuffer() -> FramebufferSnapshot {
    let host = sys::snapshot_framebuffer();
    FramebufferSnapshot {
        last_bytes: last_framebuffer_bytes(),
        last_request_rmode: host.rmode,
        last_request_bytes: host.bytes,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemSnapshot {
    pub config: SystemConfigSnapshot,
    pub arenas: ArenaSnapshot,
    pub callbacks: CallbackSnapshot,
    pub alarms: AlarmSnapshot,
    pub hardware: HardwareSnapshot,
    pub font: FontSnapshot,
    pub startup: StartupSnapshot,
    pub power_dispatch: PowerDispatchSnapshot,
    pub reset_registration: ResetRegistrationSnapshot,
    pub runtime_calls: RuntimeCallSnapshot,
    pub pre_main: PreMainSnapshot,
    pub reset_system: ResetSystemCallSnapshot,
    pub protect_range: ProtectRangeSnapshot,
    pub pmc: PmcSnapshot,
    pub time: TimeSnapshot,
    pub sram_sync: SramSyncSnapshot,
    pub framebuffer: FramebufferSnapshot,
}

pub fn snapshot() -> SystemSnapshot {
    SystemSnapshot {
        config: snapshot_config(),
        arenas: snapshot_arenas(),
        callbacks: snapshot_callbacks(),
        alarms: snapshot_alarms(),
        hardware: snapshot_hardware(),
        font: snapshot_font(),
        startup: snapshot_startup(),
        power_dispatch: snapshot_power_dispatch(),
        reset_registration: snapshot_reset_registration(),
        runtime_calls: snapshot_runtime_calls(),
        pre_main: snapshot_pre_main(),
        reset_system: snapshot_reset_system_call(),
        protect_range: snapshot_protect_range(),
        pmc: snapshot_pmc(),
        time: snapshot_time(),
        sram_sync: snapshot_sram_sync(),
        framebuffer: snapshot_framebuffer(),
    }
}

pub fn start_pmc(mcr0val: u32, mcr1val: u32) {
    let _ = (mcr0val, mcr1val);
    unsafe { sys::sys_start_pmc(mcr0val, mcr1val) }
}

pub fn dump_pmc() {
    unsafe { sys::sys_dump_pmc() };
    let counters = sys::snapshot_pmc();
    report(core::format_args!(
        "<{} load/stores / {} miss cycles / {} cycles / {} instructions>\n",
        counters.pmc1,
        counters.pmc2,
        counters.pmc3,
        counters.pmc4
    ));
}

pub fn stop_pmc() {
    unsafe { sys::sys_stop_pmc() }
}

pub fn reset_pmc() {
    unsafe { sys::sys_reset_pmc() }
}

#[cfg(target_arch = "powerpc")]
unsafe fn framebuffer_pool_alloc(size: usize) -> *mut c_void {
    unsafe {
        let size = size.max(1);
        let base = core::ptr::addr_of_mut!(FRAMEBUFFER_POOL.0) as *mut u8 as usize;
        let mut offset = FRAMEBUFFER_POOL_NEXT.load(Ordering::Acquire);
        loop {
            let aligned =
                (base + offset + (FRAMEBUFFER_ALIGNMENT - 1)) & !(FRAMEBUFFER_ALIGNMENT - 1);
            let next = aligned.saturating_add(size).saturating_sub(base);
            if next > FRAMEBUFFER_POOL_BYTES {
                return core::ptr::null_mut();
            }
            match FRAMEBUFFER_POOL_NEXT.compare_exchange(
                offset,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return aligned as *mut c_void;
                }
                Err(current) => offset = current,
            }
        }
    }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn framebuffer_pool_alloc(size: usize) -> *mut c_void {
    use std::alloc::{Layout, alloc};

    let size = size.max(1);
    match Layout::from_size_align(size, FRAMEBUFFER_ALIGNMENT) {
        Ok(layout) => alloc(layout).cast(),
        Err(_) => core::ptr::null_mut(),
    }
}

pub fn create_alarm(thealarm: &mut SysWd) -> i32 {
    let _ = thealarm;
    let result = unsafe { sys::sys_create_alarm(thealarm) };
    LAST_ALARM_PTR.store(*thealarm as usize, Ordering::Release);
    result
}

/// # Safety
///
/// `cbarg` is passed through to the alarm callback and must remain valid for
/// the callback's expectations.
pub unsafe fn set_alarm(
    thealarm: SysWd,
    tp: &TimeSpec,
    cb: AlarmCallback,
    cbarg: *mut c_void,
) -> i32 {
    unsafe {
        let _ = (thealarm, tp, cb, cbarg);
        LAST_ALARM_CB_PTR.store(cb.map_or(0, |func| func as usize), Ordering::Release);
        sys::sys_set_alarm(thealarm, tp, cb, cbarg)
    }
}

/// # Safety
///
/// `cbarg` is passed through to the alarm callback and must remain valid for
/// the callback's expectations.
pub unsafe fn set_periodic_alarm(
    thealarm: SysWd,
    tp_start: &TimeSpec,
    tp_period: &TimeSpec,
    cb: AlarmCallback,
    cbarg: *mut c_void,
) -> i32 {
    unsafe {
        let _ = (thealarm, tp_start, tp_period, cb, cbarg);
        LAST_PERIODIC_ALARM_CB_PTR.store(cb.map_or(0, |func| func as usize), Ordering::Release);
        sys::sys_set_periodic_alarm(thealarm, tp_start, tp_period, cb, cbarg)
    }
}

pub fn remove_alarm(thealarm: SysWd) -> i32 {
    let _ = thealarm;
    unsafe { sys::sys_remove_alarm(thealarm) }
}

pub fn cancel_alarm(thealarm: SysWd) -> i32 {
    let _ = thealarm;
    unsafe { sys::sys_cancel_alarm(thealarm) }
}

pub fn stdio_report(use_stdout: bool) {
    STDIO_REPORT_USE_STDOUT.store(use_stdout, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    sys::sys_stdio_report(use_stdout as Bool)
}

pub fn stdio_report_uses_stdout() -> bool {
    #[cfg(target_arch = "powerpc")]
    {
        let use_stdout = sys::sys_stdio_report_uses_stdout();
        STDIO_REPORT_USE_STDOUT.store(use_stdout, Ordering::Release);
        use_stdout
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        STDIO_REPORT_USE_STDOUT.load(Ordering::Acquire)
    }
}

struct ReportCallbackSink;

impl fmt::Write for ReportCallbackSink {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            if let Some(cb) = sys::DIET_PRINT_FN {
                cb(s.as_ptr().cast(), s.len());
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "powerpc"))]
struct ReportIoSink<W>(W);

#[cfg(not(target_arch = "powerpc"))]
impl<W: std::io::Write> fmt::Write for ReportIoSink<W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        std::io::Write::write_all(&mut self.0, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

pub fn try_report(args: fmt::Arguments) -> fmt::Result {
    #[cfg(ogc_dev)]
    sys::capture_dev_report(args);
    let has_callback =
        unsafe { core::ptr::read(core::ptr::addr_of!(sys::DIET_PRINT_FN)) }.is_some();
    if has_callback {
        let mut sink = ReportCallbackSink;
        fmt::Write::write_fmt(&mut sink, args)
    } else {
        #[cfg(not(target_arch = "powerpc"))]
        {
            if stdio_report_uses_stdout() {
                let mut sink = ReportIoSink(std::io::stdout());
                fmt::Write::write_fmt(&mut sink, args)
            } else {
                let mut sink = ReportIoSink(std::io::stderr());
                fmt::Write::write_fmt(&mut sink, args)
            }
        }
        #[cfg(target_arch = "powerpc")]
        {
            Ok(())
        }
    }
}

pub fn report(args: fmt::Arguments) {
    LAST_REPORT_SUCCEEDED.store(try_report(args).is_ok(), Ordering::Release);
}

pub fn last_report_succeeded() -> bool {
    LAST_REPORT_SUCCEEDED.load(Ordering::Acquire)
}

#[cfg(target_arch = "powerpc")]
pub fn last_config_write_result() -> i32 {
    LAST_CONFIG_WRITE_RESULT.load(Ordering::Acquire)
}

pub fn check_argv() {
    sys::_check_argv()
}

pub fn get_counter_bias() -> u32 {
    let bias = unsafe { sys::sys_get_counter_bias() };
    COUNTER_BIAS.store(bias, Ordering::Release);
    bias
}

pub fn set_counter_bias(bias: u32) {
    unsafe { sys::sys_set_counter_bias(bias) };
    COUNTER_BIAS.store(unsafe { sys::sys_get_counter_bias() }, Ordering::Release);
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_display_offset_h() -> i8 {
    let offset = unsafe { sys::sys_get_display_offset_h() };
    DISPLAY_OFFSET_H.store(offset as u8, Ordering::Release);
    offset
}

pub fn set_display_offset_h(offset: i8) {
    unsafe { sys::sys_set_display_offset_h(offset) };
    DISPLAY_OFFSET_H.store(
        unsafe { sys::sys_get_display_offset_h() } as u8,
        Ordering::Release,
    );
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_eu_rgb60() -> u8 {
    let enable = unsafe { sys::sys_get_eu_rgb60() };
    EU_RGB60.store(enable, Ordering::Release);
    enable
}

pub fn set_eu_rgb60(enable: u8) {
    unsafe { sys::sys_set_eu_rgb60(enable) };
    EU_RGB60.store(unsafe { sys::sys_get_eu_rgb60() }, Ordering::Release);
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_language() -> u8 {
    let lang = unsafe { sys::sys_get_language() };
    LANGUAGE.store(lang, Ordering::Release);
    lang
}

pub fn set_language(lang: u8) {
    unsafe { sys::sys_set_language(lang) };
    LANGUAGE.store(unsafe { sys::sys_get_language() }, Ordering::Release);
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_progressive_scan() -> u8 {
    let enable = unsafe { sys::sys_get_progressive_scan() };
    PROGRESSIVE_SCAN.store(enable, Ordering::Release);
    enable
}

pub fn set_progressive_scan(enable: u8) {
    unsafe { sys::sys_set_progressive_scan(enable) };
    PROGRESSIVE_SCAN.store(
        unsafe { sys::sys_get_progressive_scan() },
        Ordering::Release,
    );
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_sound_mode() -> u8 {
    let mode = unsafe { sys::sys_get_sound_mode() };
    SOUND_MODE.store(mode, Ordering::Release);
    mode
}

pub fn set_sound_mode(mode: u8) {
    unsafe { sys::sys_set_sound_mode(mode) };
    SOUND_MODE.store(unsafe { sys::sys_get_sound_mode() }, Ordering::Release);
    LAST_CONFIG_WRITE_RESULT.store(0, Ordering::Release);
}

pub fn get_video_mode() -> u8 {
    #[cfg(target_arch = "powerpc")]
    {
        let mode = unsafe { sys::sys_get_video_mode() };
        VIDEO_MODE.store(mode & 3, Ordering::Release);
        mode
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        VIDEO_MODE.load(Ordering::Acquire)
    }
}

pub fn set_video_mode(mode: u8) {
    let mode = mode & 3;
    VIDEO_MODE.store(mode, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    unsafe {
        sys::sys_set_video_mode(mode)
    }
}

pub fn get_wireless_id(chan: u32) -> u16 {
    #[cfg(target_arch = "powerpc")]
    {
        let id = unsafe { sys::sys_get_wireless_id(chan) };
        match chan {
            0 => WIRELESS_ID0.store(id, Ordering::Release),
            1 => WIRELESS_ID1.store(id, Ordering::Release),
            2 => WIRELESS_ID2.store(id, Ordering::Release),
            3 => WIRELESS_ID3.store(id, Ordering::Release),
            _ => {}
        }
        id
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        match chan {
            0 => WIRELESS_ID0.load(Ordering::Acquire),
            1 => WIRELESS_ID1.load(Ordering::Acquire),
            2 => WIRELESS_ID2.load(Ordering::Acquire),
            3 => WIRELESS_ID3.load(Ordering::Acquire),
            _ => 0,
        }
    }
}

pub fn set_wireless_id(chan: u32, id: u16) {
    match chan {
        0 => WIRELESS_ID0.store(id, Ordering::Release),
        1 => WIRELESS_ID1.store(id, Ordering::Release),
        2 => WIRELESS_ID2.store(id, Ordering::Release),
        3 => WIRELESS_ID3.store(id, Ordering::Release),
        _ => {}
    }
    #[cfg(target_arch = "powerpc")]
    unsafe {
        sys::sys_set_wireless_id(chan, id)
    }
}

pub fn get_gbs_mode() -> u16 {
    #[cfg(target_arch = "powerpc")]
    {
        let mode = unsafe { sys::sys_get_gbs_mode() };
        GBS_MODE.store(mode, Ordering::Release);
        mode
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        GBS_MODE.load(Ordering::Acquire)
    }
}

pub fn set_gbs_mode(mode: u16) {
    let mode = sanitize_gbs_mode(mode);
    GBS_MODE.store(mode, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    unsafe {
        sys::sys_set_gbs_mode(mode)
    }
}

pub fn get_font_encoding() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let encoding = unsafe { sys::sys_get_font_encoding() };
        FONT_ENCODING.store(encoding, Ordering::Release);
        encoding
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let encoding = sys::sys_get_font_encoding();
        FONT_ENCODING.store(encoding, Ordering::Release);
        encoding
    }
}

/// # Safety
///
/// `font_data` must point to a writable buffer of `SYS_FONTSIZE_ANSI` or
/// `SYS_FONTSIZE_SJIS` bytes, matching libogc's font initialization contract.
pub unsafe fn init_font(font_data: *mut SysFontHeader) -> u32 {
    unsafe {
        let _ = font_data;
        sys::sys_init_font(font_data)
    }
}

/// # Safety
///
/// Output pointers must be valid for writes and the font subsystem must have
/// been initialized successfully.
pub unsafe fn get_font_texture(
    c: i32,
    image: *mut *mut c_void,
    xpos: *mut i32,
    ypos: *mut i32,
    width: *mut i32,
) {
    unsafe {
        let _ = (c, image, xpos, ypos, width);
        sys::sys_get_font_texture(c, image, xpos, ypos, width)
    }
}

/// # Safety
///
/// `image` and `width` must be valid for the requested font copy operation.
pub unsafe fn get_font_texel(c: i32, image: *mut c_void, pos: i32, stride: i32, width: *mut i32) {
    unsafe {
        let _ = (c, image, pos, stride, width);
        sys::sys_get_font_texel(c, image, pos, stride, width)
    }
}

/// # Safety
///
/// This may reset or power off hardware depending on `reset`.
pub unsafe fn reset_system(reset: i32, reset_code: u32, force_menu: i32) {
    unsafe {
        LAST_RESET_SYSTEM_CALLED.store(true, Ordering::Release);
        LAST_RESET_SYSTEM_RESET.store(reset as u32, Ordering::Release);
        LAST_RESET_SYSTEM_CODE.store(reset_code, Ordering::Release);
        LAST_RESET_SYSTEM_FORCE_MENU.store(force_menu != 0, Ordering::Release);
        sys::sys_reset_system(reset, reset_code, force_menu)
    }
}

/// # Safety
///
/// `info` must remain valid while registered with libogc.
pub unsafe fn register_reset_func(info: *mut SysResetInfo) {
    unsafe {
        let _ = info;
        sys::sys_register_reset_func(info)
    }
}

/// # Safety
///
/// `info` must be a reset function record currently registered with libogc.
pub unsafe fn unregister_reset_func(info: *mut SysResetInfo) {
    unsafe {
        let _ = info;
        sys::sys_unregister_reset_func(info)
    }
}

pub fn get_arena1_lo() -> *mut c_void {
    #[cfg(target_arch = "powerpc")]
    {
        let lo = unsafe { sys::sys_get_arena1_lo() };
        ARENA1_LO.store(lo as usize, Ordering::Release);
        lo
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ARENA1_LO.load(Ordering::Acquire) as *mut c_void
    }
}

/// # Safety
///
/// `new_lo` must be a valid arena boundary for MEM1.
pub unsafe fn set_arena1_lo(new_lo: *mut c_void) {
    unsafe {
        ARENA1_LO.store(new_lo as usize, Ordering::Release);
        sys::sys_set_arena1_lo(new_lo)
    }
}

pub fn get_arena1_hi() -> *mut c_void {
    #[cfg(target_arch = "powerpc")]
    {
        let hi = unsafe { sys::sys_get_arena1_hi() };
        ARENA1_HI.store(hi as usize, Ordering::Release);
        hi
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ARENA1_HI.load(Ordering::Acquire) as *mut c_void
    }
}

/// # Safety
///
/// `new_hi` must be a valid arena boundary for MEM1.
pub unsafe fn set_arena1_hi(new_hi: *mut c_void) {
    unsafe {
        ARENA1_HI.store(new_hi as usize, Ordering::Release);
        sys::sys_set_arena1_hi(new_hi)
    }
}

pub fn get_arena1_size() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        unsafe { sys::sys_get_arena1_size() }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        sys::sys_get_arena1_size()
    }
}

pub fn alloc_arena1_mem_lo(size: u32, align: u32) -> *mut c_void {
    let lo = get_arena1_lo() as usize;
    let (ptr, next_lo) = arena_low_alloc_from(lo, size as usize, align as usize);
    unsafe { set_arena1_lo(next_lo as *mut c_void) };
    ptr as *mut c_void
}

pub fn set_reset_callback(cb: ResetCallback) -> ResetCallback {
    #[cfg(target_arch = "powerpc")]
    {
        let old = unsafe { sys::sys_set_reset_callback(cb) };
        RESET_CALLBACK_PTR.store(cb.map_or(0, |func| func as usize), Ordering::Release);
        old
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        take_and_store_reset_callback(cb)
    }
}

pub fn reset_button_down() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = unsafe { sys::sys_reset_button_down() };
        RESET_BUTTON_DOWN.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let value = sys::sys_reset_button_down();
        RESET_BUTTON_DOWN.store(value, Ordering::Release);
        value
    }
}

pub fn get_hollywood_revision() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = unsafe { sys::sys_get_hollywood_revision() };
        HOLLYWOOD_REVISION.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let value = sys::sys_get_hollywood_revision();
        HOLLYWOOD_REVISION.store(value, Ordering::Release);
        value
    }
}

#[cfg(not(gamecube))]
pub fn get_arena2_lo() -> *mut c_void {
    #[cfg(target_arch = "powerpc")]
    {
        let lo = unsafe { sys::sys_get_arena2_lo() };
        ARENA2_LO.store(lo as usize, Ordering::Release);
        lo
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ARENA2_LO.load(Ordering::Acquire) as *mut c_void
    }
}

/// # Safety
///
/// `new_lo` must be a valid arena boundary for MEM2.
#[cfg(not(gamecube))]
pub unsafe fn set_arena2_lo(new_lo: *mut c_void) {
    unsafe {
        ARENA2_LO.store(new_lo as usize, Ordering::Release);
        sys::sys_set_arena2_lo(new_lo)
    }
}

#[cfg(not(gamecube))]
pub fn get_arena2_hi() -> *mut c_void {
    #[cfg(target_arch = "powerpc")]
    {
        let hi = unsafe { sys::sys_get_arena2_hi() };
        ARENA2_HI.store(hi as usize, Ordering::Release);
        hi
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ARENA2_HI.load(Ordering::Acquire) as *mut c_void
    }
}

/// # Safety
///
/// `new_hi` must be a valid arena boundary for MEM2.
#[cfg(not(gamecube))]
pub unsafe fn set_arena2_hi(new_hi: *mut c_void) {
    unsafe {
        ARENA2_HI.store(new_hi as usize, Ordering::Release);
        sys::sys_set_arena2_hi(new_hi)
    }
}

#[cfg(not(gamecube))]
pub fn get_arena2_size() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        unsafe { sys::sys_get_arena2_size() }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        sys::sys_get_arena2_size()
    }
}

#[cfg(not(gamecube))]
pub fn alloc_arena2_mem_lo(size: u32, align: u32) -> *mut c_void {
    let lo = get_arena2_lo() as usize;
    let (ptr, next_lo) = arena_low_alloc_from(lo, size as usize, align as usize);
    unsafe { set_arena2_lo(next_lo as *mut c_void) };
    ptr as *mut c_void
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemConfigSnapshot {
    pub counter_bias: u32,
    pub counter_bias_get_called: bool,
    pub counter_bias_set_called: bool,
    pub display_offset_h: i8,
    pub display_offset_get_called: bool,
    pub display_offset_set_called: bool,
    pub eu_rgb60: u8,
    pub eu_rgb60_get_called: bool,
    pub eu_rgb60_set_called: bool,
    pub language: u8,
    pub language_get_called: bool,
    pub language_set_called: bool,
    pub progressive_scan: u8,
    pub progressive_scan_get_called: bool,
    pub progressive_scan_set_called: bool,
    pub sound_mode: u8,
    pub video_mode: u8,
    pub wireless_id: [u16; 4],
    pub gbs_mode: u16,
    pub stdout_report: bool,
}

pub fn snapshot_config() -> SystemConfigSnapshot {
    let counter_bias = get_counter_bias();
    let counter_bias_probe = sys::snapshot_counter_bias_probe();
    let display_offset_h = get_display_offset_h();
    let display_offset_probe = sys::snapshot_display_offset_probe();
    let eu_rgb60 = get_eu_rgb60();
    let eu_rgb60_probe = sys::snapshot_eu_rgb60_probe();
    let language = get_language();
    let language_probe = sys::snapshot_language_probe();
    let progressive_scan = get_progressive_scan();
    let progressive_scan_probe = sys::snapshot_progressive_scan_probe();
    SystemConfigSnapshot {
        counter_bias,
        counter_bias_get_called: counter_bias_probe.get_called,
        counter_bias_set_called: counter_bias_probe.set_called,
        display_offset_h,
        display_offset_get_called: display_offset_probe.get_called,
        display_offset_set_called: display_offset_probe.set_called,
        eu_rgb60,
        eu_rgb60_get_called: eu_rgb60_probe.get_called,
        eu_rgb60_set_called: eu_rgb60_probe.set_called,
        language,
        language_get_called: language_probe.get_called,
        language_set_called: language_probe.set_called,
        progressive_scan,
        progressive_scan_get_called: progressive_scan_probe.get_called,
        progressive_scan_set_called: progressive_scan_probe.set_called,
        sound_mode: get_sound_mode(),
        video_mode: get_video_mode(),
        wireless_id: [
            get_wireless_id(0),
            get_wireless_id(1),
            get_wireless_id(2),
            get_wireless_id(3),
        ],
        gbs_mode: get_gbs_mode(),
        stdout_report: stdio_report_uses_stdout(),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ArenaSnapshot {
    pub arena1_lo: *mut c_void,
    pub arena1_hi: *mut c_void,
    #[cfg(not(gamecube))]
    pub arena2_lo: *mut c_void,
    #[cfg(not(gamecube))]
    pub arena2_hi: *mut c_void,
}

pub fn snapshot_arenas() -> ArenaSnapshot {
    ArenaSnapshot {
        arena1_lo: get_arena1_lo(),
        arena1_hi: get_arena1_hi(),
        #[cfg(not(gamecube))]
        arena2_lo: get_arena2_lo(),
        #[cfg(not(gamecube))]
        arena2_hi: get_arena2_hi(),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct CallbackSnapshot {
    pub reset_callback: usize,
    pub power_callback: usize,
    pub diet_print_fn: usize,
}

pub fn snapshot_callbacks() -> CallbackSnapshot {
    CallbackSnapshot {
        reset_callback: RESET_CALLBACK_PTR.load(Ordering::Acquire),
        power_callback: POWER_CALLBACK_PTR.load(Ordering::Acquire),
        diet_print_fn: DIET_PRINT_FN_PTR.load(Ordering::Acquire),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct AlarmSnapshot {
    pub alarm: usize,
    pub callback: usize,
    pub periodic_callback: usize,
}

pub fn snapshot_alarms() -> AlarmSnapshot {
    AlarmSnapshot {
        alarm: LAST_ALARM_PTR.load(Ordering::Acquire),
        callback: LAST_ALARM_CB_PTR.load(Ordering::Acquire),
        periodic_callback: LAST_PERIODIC_ALARM_CB_PTR.load(Ordering::Acquire),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct HardwareSnapshot {
    pub font_encoding: u32,
    pub reset_button_down: u32,
    pub hollywood_revision: u32,
    pub font_encoding_read_called: bool,
    pub reset_button_read_called: bool,
    pub hollywood_revision_read_called: bool,
}

pub fn snapshot_hardware() -> HardwareSnapshot {
    let font_probe = sys::snapshot_font();
    let probe = sys::snapshot_hardware_probe();
    HardwareSnapshot {
        font_encoding: FONT_ENCODING.load(Ordering::Acquire),
        reset_button_down: RESET_BUTTON_DOWN.load(Ordering::Acquire),
        hollywood_revision: HOLLYWOOD_REVISION.load(Ordering::Acquire),
        font_encoding_read_called: font_probe.encoding_queried,
        reset_button_read_called: probe.reset_button_called,
        hollywood_revision_read_called: probe.hollywood_revision_called,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct HardwareProof {
    pub font_encoding: u32,
    pub reset_button_down: u32,
    pub hollywood_revision: u32,
    pub time_before: u64,
    pub time_after: u64,
    pub time_monotonic: bool,
}

/// Reads hardware-observable system values through their runtime entry points.
///
/// This is intentionally separate from snapshots, which are for host-side
/// bookkeeping and unit tests.
pub fn hardware_proof() -> HardwareProof {
    let time_before = time();
    let font_encoding = get_font_encoding();
    let reset_button_down = reset_button_down();
    let hollywood_revision = get_hollywood_revision();
    let time_after = time();

    HardwareProof {
        font_encoding,
        reset_button_down,
        hollywood_revision,
        time_before,
        time_after,
        time_monotonic: time_after >= time_before,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct FontSnapshot {
    pub encoding_queried: bool,
    pub init_called: bool,
    pub init_ptr: usize,
    pub texture_called: bool,
    pub texture_c: u32,
    pub texel_called: bool,
    pub texel_c: u32,
}

pub fn snapshot_font() -> FontSnapshot {
    let snap = sys::snapshot_font();
    FontSnapshot {
        encoding_queried: snap.encoding_queried,
        init_called: snap.init_called,
        init_ptr: snap.init_ptr,
        texture_called: snap.texture_called,
        texture_c: snap.texture_c,
        texel_called: snap.texel_called,
        texel_c: snap.texel_c,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeCallSnapshot {
    pub init_called: bool,
    pub pre_main_called: bool,
    pub main_loop_called: bool,
    pub main_loop_result: bool,
}

pub fn snapshot_runtime_calls() -> RuntimeCallSnapshot {
    let snap = sys::snapshot_runtime_calls();
    RuntimeCallSnapshot {
        init_called: snap.init_called,
        pre_main_called: snap.pre_main_called,
        main_loop_called: snap.main_loop_called,
        main_loop_result: snap.main_loop_result,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PreMainSnapshot {
    pub called: bool,
    pub completed: bool,
    /// The failed operation number from the pre-main progress callback, or zero.
    pub failed_stage: u32,
    pub result: i32,
}

pub fn snapshot_pre_main() -> PreMainSnapshot {
    let snap = sys::snapshot_pre_main();
    PreMainSnapshot {
        called: snap.called,
        completed: snap.completed,
        failed_stage: snap.failed_stage,
        result: snap.result,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct StartupSnapshot {
    pub init_called: bool,
    pub low_mem_init_called: bool,
    pub init_started: bool,
    pub init_completed: bool,
    pub init_stage: u32,
    pub init_completed_mask: u32,
    pub init_failed_stage: u32,
}

pub fn snapshot_startup() -> StartupSnapshot {
    let snap = sys::snapshot_startup();
    StartupSnapshot {
        init_called: snap.init_called,
        low_mem_init_called: snap.low_mem_init_called,
        init_started: snap.init_started,
        init_completed: snap.init_completed,
        init_stage: snap.init_stage,
        init_completed_mask: snap.init_completed_mask,
        init_failed_stage: snap.init_failed_stage,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ResetSystemCallSnapshot {
    pub called: bool,
    pub reset: u32,
    pub reset_code: u32,
    pub force_menu: bool,
    pub terminal_hint: u32,
}

pub fn snapshot_reset_system_call() -> ResetSystemCallSnapshot {
    let snap = sys::snapshot_reset_system();
    ResetSystemCallSnapshot {
        called: snap.called,
        reset: snap.reset,
        reset_code: snap.reset_code,
        force_menu: snap.force_menu,
        terminal_hint: snap.terminal_hint,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ProtectRangeSnapshot {
    pub called: bool,
    pub chan: u32,
    pub addr: *mut c_void,
    pub bytes: u32,
    pub cntrl: u32,
}

pub fn snapshot_protect_range() -> ProtectRangeSnapshot {
    let snap = sys::snapshot_protect_range();
    ProtectRangeSnapshot {
        called: snap.called,
        chan: snap.chan,
        addr: snap.addr,
        bytes: snap.bytes,
        cntrl: snap.cntrl,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PmcSnapshot {
    pub started: bool,
    pub dumped: bool,
    pub stopped: bool,
    pub reset: bool,
    pub mcr0val: u32,
    pub mcr1val: u32,
}

pub fn snapshot_pmc() -> PmcSnapshot {
    let snap = sys::snapshot_pmc();
    PmcSnapshot {
        started: snap.started,
        dumped: snap.dumped,
        stopped: snap.stopped,
        reset: snap.reset,
        mcr0val: snap.mcr0val,
        mcr1val: snap.mcr1val,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PowerDispatchSnapshot {
    pub called: bool,
    pub result: bool,
}

pub fn snapshot_power_dispatch() -> PowerDispatchSnapshot {
    let snap = sys::snapshot_power_dispatch();
    PowerDispatchSnapshot {
        called: snap.called,
        result: snap.result,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ResetRegistrationSnapshot {
    pub register_called: bool,
    pub unregister_called: bool,
    pub register_ptr: usize,
    pub unregister_ptr: usize,
}

pub fn snapshot_reset_registration() -> ResetRegistrationSnapshot {
    let snap = sys::snapshot_reset_registration();
    ResetRegistrationSnapshot {
        register_called: snap.register_called,
        unregister_called: snap.unregister_called,
        register_ptr: snap.register_ptr,
        unregister_ptr: snap.unregister_ptr,
    }
}

pub fn set_power_callback(cb: PowerCallback) -> PowerCallback {
    #[cfg(target_arch = "powerpc")]
    {
        let old = unsafe { sys::sys_set_power_callback(cb) };
        POWER_CALLBACK_PTR.store(cb.map_or(0, |func| func as usize), Ordering::Release);
        old
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let old = unsafe { sys::sys_set_power_callback(cb) };
        POWER_CALLBACK_PTR.store(cb.map_or(0, |func| func as usize), Ordering::Release);
        old
    }
}

pub fn do_power_cb() {
    unsafe { sys::sys_do_power_cb() }
}

pub fn time() -> u64 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = unsafe { sys::sys_time() };
        store_last_time_read(value);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let value = sys::sys_time();
        store_last_time_read(value);
        value
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TimeSnapshot {
    pub last_time_read: u64,
    pub time_read_called: bool,
}

pub fn snapshot_time() -> TimeSnapshot {
    let probe = sys::snapshot_time_probe();
    TimeSnapshot {
        last_time_read: load_last_time_read(),
        time_read_called: probe.called,
    }
}

fn store_last_time_read(value: u64) {
    LAST_TIME_READ_LO.store(value as u32, Ordering::Release);
    LAST_TIME_READ_HI.store((value >> 32) as u32, Ordering::Release);
}

fn load_last_time_read() -> u64 {
    let lo = LAST_TIME_READ_LO.load(Ordering::Acquire) as u64;
    let hi = LAST_TIME_READ_HI.load(Ordering::Acquire) as u64;
    (hi << 32) | lo
}

pub fn diet_print_set_func(fn_: DietPrintFn) {
    DIET_PRINT_FN_PTR.store(fn_.map_or(0, |func| func as usize), Ordering::Release);
    unsafe { sys::DIET_PRINT_FN = fn_ }
}

#[cfg(not(target_arch = "powerpc"))]
fn take_and_store_reset_callback(cb: ResetCallback) -> ResetCallback {
    load_reset_callback(
        RESET_CALLBACK_PTR.swap(cb.map_or(0, |func| func as usize), Ordering::AcqRel),
    )
}

#[cfg(not(target_arch = "powerpc"))]
fn load_reset_callback(ptr: usize) -> ResetCallback {
    if ptr == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe fn(u32, *mut c_void)>(ptr) })
    }
}

/// # Safety
///
/// The returned pointer aliases libogc's shared SRAM buffer and must be
/// unlocked exactly once with [`unlock_sram`].
pub unsafe fn lock_sram() -> *mut SysSram {
    unsafe {
        #[cfg(target_arch = "powerpc")]
        {
            sys::_sys_lock_sram()
        }
        #[cfg(not(target_arch = "powerpc"))]
        {
            sys::_sys_lock_sram()
        }
    }
}

/// # Safety
///
/// Must pair with a successful [`lock_sram`] call.
pub unsafe fn unlock_sram(write: bool) -> bool {
    #[cfg(target_arch = "powerpc")]
    {
        unsafe { sys::_sys_unlock_sram(write as u32) != 0 }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        unsafe { sys::_sys_unlock_sram(write as u32) != 0 }
    }
}

/// # Safety
///
/// The returned pointer aliases libogc's shared extended SRAM buffer and must
/// be unlocked exactly once with [`unlock_sram_ex`].
pub unsafe fn lock_sram_ex() -> *mut SysSramEx {
    unsafe {
        #[cfg(target_arch = "powerpc")]
        {
            sys::_sys_lock_sram_ex()
        }
        #[cfg(not(target_arch = "powerpc"))]
        {
            sys::_sys_lock_sram_ex()
        }
    }
}

/// # Safety
///
/// Must pair with a successful [`lock_sram_ex`] call.
pub unsafe fn unlock_sram_ex(write: bool) -> bool {
    #[cfg(target_arch = "powerpc")]
    {
        unsafe { sys::_sys_unlock_sram_ex(write as u32) != 0 }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        unsafe { sys::_sys_unlock_sram_ex(write as u32) != 0 }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SramUnlockSnapshot {
    pub standard: bool,
    pub extended: bool,
}

pub fn snapshot_sram_unlock() -> SramUnlockSnapshot {
    SramUnlockSnapshot {
        standard: LAST_SRAM_UNLOCK_RESULT.load(Ordering::Acquire),
        extended: LAST_SRAM_EX_UNLOCK_RESULT.load(Ordering::Acquire),
    }
}

pub fn sync_sram() -> bool {
    LAST_SRAM_SYNC_CALLED.store(true, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    {
        let result = unsafe { sys::_sys_sync_sram() != 0 };
        LAST_SRAM_SYNC_RESULT.store(result, Ordering::Release);
        result
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let result = unsafe { sys::_sys_sync_sram() != 0 };
        LAST_SRAM_SYNC_RESULT.store(result, Ordering::Release);
        result
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SramSyncSnapshot {
    pub called: bool,
    pub last_result: bool,
}

pub fn snapshot_sram_sync() -> SramSyncSnapshot {
    SramSyncSnapshot {
        called: LAST_SRAM_SYNC_CALLED.load(Ordering::Acquire),
        last_result: LAST_SRAM_SYNC_RESULT.load(Ordering::Acquire),
    }
}

pub fn snapshot_sram() -> Option<SysSram> {
    let sram = unsafe { lock_sram() };
    if sram.is_null() {
        return None;
    }
    let snapshot = unsafe { *sram };
    LAST_SRAM_UNLOCK_RESULT.store(unsafe { unlock_sram(false) }, Ordering::Release);
    Some(snapshot)
}

pub fn snapshot_sram_ex() -> Option<SysSramEx> {
    let sram_ex = unsafe { lock_sram_ex() };
    if sram_ex.is_null() {
        return None;
    }
    let snapshot = unsafe { *sram_ex };
    LAST_SRAM_EX_UNLOCK_RESULT.store(unsafe { unlock_sram_ex(false) }, Ordering::Release);
    Some(snapshot)
}

pub fn sram_checksum(sram: &SysSram) -> (u16, u16) {
    let words = [
        (sram.counter_bias >> 16) as u16,
        sram.counter_bias as u16,
        ((sram.display_offset_h as u8 as u16) << 8) | u16::from(sram.ntd),
        (u16::from(sram.lang) << 8) | u16::from(sram.flags),
    ];
    sys::sram_checksum_words(words)
}

pub fn normalize_sram_flags(flags: u8) -> u8 {
    if (flags & SRAM_VIDEO_MODE_BITS) > SYS_VIDEO_MPAL as u8 {
        flags & !SRAM_VIDEO_MODE_BITS
    } else {
        flags
    }
}

pub fn refresh_sram_checksum(sram: &mut SysSram) {
    sram.flags = normalize_sram_flags(sram.flags);
    let (checksum, checksum_inv) = sram_checksum(sram);
    sram.checksum = checksum;
    sram.checksum_inv = checksum_inv;
}

pub fn sram_counter_bias(sram: &SysSram) -> u32 {
    sram.counter_bias
}

pub fn set_sram_counter_bias(sram: &mut SysSram, bias: u32) -> bool {
    if sram.counter_bias == bias {
        return false;
    }
    sram.counter_bias = bias;
    true
}

pub fn sram_display_offset_h(sram: &SysSram) -> i8 {
    sram.display_offset_h
}

pub fn set_sram_display_offset_h(sram: &mut SysSram, offset: i8) -> bool {
    if sram.display_offset_h == offset {
        return false;
    }
    sram.display_offset_h = offset;
    true
}

pub fn sram_eu_rgb60(sram: &SysSram) -> u8 {
    (sram.ntd >> 6) & 0x01
}

pub fn set_sram_eu_rgb60(sram: &mut SysSram, enable: u8) -> bool {
    let enable = enable & 0x01;
    if sram_eu_rgb60(sram) == enable {
        return false;
    }
    sram.ntd = (sram.ntd & !0x40) | (enable << 6);
    true
}

pub fn sram_language(sram: &SysSram) -> u8 {
    sram.lang
}

pub fn set_sram_language(sram: &mut SysSram, lang: u8) -> bool {
    if sram.lang == lang {
        return false;
    }
    sram.lang = lang;
    true
}

pub fn sram_progressive_scan(sram: &SysSram) -> u8 {
    ((sram.flags & SRAM_PROGSCAN_BIT) != 0) as u8
}

pub fn set_sram_progressive_scan(sram: &mut SysSram, enable: u8) -> bool {
    let enable = enable & 0x01;
    if sram_progressive_scan(sram) == enable {
        return false;
    }
    sram.flags = (sram.flags & !SRAM_PROGSCAN_BIT) | (enable << 7);
    true
}

pub fn sram_sound_mode(sram: &SysSram) -> u8 {
    ((sram.flags & SRAM_SOUND_MODE_BIT) != 0) as u8
}

pub fn set_sram_sound_mode(sram: &mut SysSram, mode: u8) -> bool {
    let mode = mode & 0x01;
    if sram_sound_mode(sram) == mode {
        return false;
    }
    sram.flags = (sram.flags & !SRAM_SOUND_MODE_BIT) | (mode << 2);
    true
}

pub fn sram_video_mode(sram: &SysSram) -> u8 {
    sram.flags & SRAM_VIDEO_MODE_BITS
}

pub fn set_sram_video_mode(sram: &mut SysSram, mode: u8) -> bool {
    let mode = mode & SRAM_VIDEO_MODE_BITS;
    if sram_video_mode(sram) == mode {
        return false;
    }
    sram.flags = (sram.flags & !SRAM_VIDEO_MODE_BITS) | mode;
    true
}

pub fn sram_ex_wireless_id(sram_ex: &SysSramEx, chan: u32) -> u16 {
    let slot = addr_of!(sram_ex.wireless_pad_id[chan as usize]);
    unsafe { read_unaligned(slot) }
}

pub fn set_sram_ex_wireless_id(sram_ex: &mut SysSramEx, chan: u32, id: u16) -> bool {
    let slot = addr_of_mut!(sram_ex.wireless_pad_id[chan as usize]);
    if unsafe { read_unaligned(slot) } == id {
        return false;
    }
    unsafe { write_unaligned(slot, id) };
    true
}

pub fn sanitize_gbs_mode(mode: u16) -> u16 {
    if ((mode >> 10) & 0x1f) >= 20 || ((mode >> 6) & 0x03) == 0x03 || (mode & 0x3f) >= 60 {
        0
    } else {
        mode
    }
}

pub fn sram_ex_gbs_mode(sram_ex: &SysSramEx) -> u16 {
    unsafe { read_unaligned(addr_of!(sram_ex.gbs)) }
}

pub fn set_sram_ex_gbs_mode(sram_ex: &mut SysSramEx, mode: u16) -> bool {
    let mode = sanitize_gbs_mode(mode);
    let gbs = addr_of_mut!(sram_ex.gbs);
    if unsafe { read_unaligned(gbs) } == mode {
        return false;
    }
    unsafe { write_unaligned(gbs, mode) };
    true
}

pub fn arena_low_alloc_from(current_lo: usize, size: usize, align: usize) -> (usize, usize) {
    let mask = align.wrapping_sub(1);
    let ptr = current_lo.wrapping_add(mask) & !mask;
    let next_lo = ptr.wrapping_add(size).wrapping_add(mask) & !mask;
    (ptr, next_lo)
}

pub fn reset_queue_insert_position(priorities: &[u32], new_priority: u32) -> usize {
    priorities
        .iter()
        .position(|priority| new_priority < *priority)
        .unwrap_or(priorities.len())
}

pub fn call_reset_functions(functions: &[ResetFunction], final_: i32, sram_sync_ok: bool) -> bool {
    let mut ret = 1_i32;
    for function in functions.iter().flatten() {
        let result = unsafe { function(final_) };
        if result == 0 {
            ret |= ret << 1;
        }
    }
    if !sram_sync_ok {
        ret |= ret << 1;
    }
    (ret & !1) == 0
}

pub fn reset_system_terminal(
    is_wii: bool,
    reset: u32,
    shutdown_mode_idle: bool,
) -> Option<ResetSystemTerminal> {
    if is_wii {
        match reset {
            SYS_RESTART => Some(ResetSystemTerminal::Reboot),
            SYS_HOTRESET | SYS_RETURNTOMENU => Some(ResetSystemTerminal::ReturnToMenu),
            SYS_SHUTDOWN => Some(ResetSystemTerminal::Shutdown),
            SYS_POWEROFF => {
                if shutdown_mode_idle {
                    Some(ResetSystemTerminal::ShutdownToIdle)
                } else {
                    Some(ResetSystemTerminal::ShutdownToStandby)
                }
            }
            SYS_POWEROFF_STANDBY => Some(ResetSystemTerminal::ShutdownToStandby),
            SYS_POWEROFF_IDLE => Some(ResetSystemTerminal::ShutdownToIdle),
            _ => None,
        }
    } else {
        match reset {
            SYS_HOTRESET => Some(ResetSystemTerminal::HotReset),
            SYS_RESTART => Some(ResetSystemTerminal::Reboot),
            SYS_SHUTDOWN => Some(ResetSystemTerminal::Shutdown),
            _ => None,
        }
    }
}

pub fn reset_system_plan(
    is_wii: bool,
    reset: u32,
    force_menu: bool,
    shutdown_mode_idle: bool,
) -> ResetSystemPlan {
    let terminal = reset_system_terminal(is_wii, reset, shutdown_mode_idle);
    let sets_boot_ipl = !is_wii && reset == SYS_HOTRESET && force_menu;
    let disables_pad_recalibration = reset == SYS_SHUTDOWN;
    let clears_active_input_list = is_wii
        && matches!(
            reset,
            SYS_POWEROFF | SYS_POWEROFF_STANDBY | SYS_POWEROFF_IDLE
        );

    ResetSystemPlan {
        terminal,
        sets_boot_ipl,
        disables_pad_recalibration,
        clears_active_input_list,
    }
}

const RESET_ACTIONS_GC_HOTRESET: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::InvokeGcHotReset,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_GC_HOTRESET_FORCE_MENU: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::SetBootIplFlag,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::InvokeGcHotReset,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_GC_RESTART: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::InvokeGcReboot,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_GC_SHUTDOWN: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::DisablePadRecalibration,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_WII_RESTART: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::InvokeWiiReboot,
    ResetSystemAction::ShutdownIosSubsystems,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_WII_RETURN_TO_MENU: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::InvokeWiiReturnToMenu,
    ResetSystemAction::ShutdownIosSubsystems,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_WII_POWEROFF: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::ClearActiveInputList,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::InvokeWiiShutdownToStandby,
    ResetSystemAction::ShutdownIosSubsystems,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_WII_POWEROFF_IDLE: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::ClearActiveInputList,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::InvokeWiiShutdownToIdle,
    ResetSystemAction::ShutdownIosSubsystems,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

const RESET_ACTIONS_WII_SHUTDOWN: &[ResetSystemAction] = &[
    ResetSystemAction::DspShutdown,
    ResetSystemAction::DisablePadRecalibration,
    ResetSystemAction::RunResetCallbacksBeforeShutdown,
    ResetSystemAction::ShutdownIosSubsystems,
    ResetSystemAction::ShutdownIrqs,
    ResetSystemAction::RunResetCallbacksAfterShutdown,
    ResetSystemAction::DisableL2Cache,
    ResetSystemAction::ClearLowMemory,
    ResetSystemAction::RestorePadRecalibration,
];

pub fn reset_system_actions(
    is_wii: bool,
    reset: u32,
    force_menu: bool,
    shutdown_mode_idle: bool,
) -> &'static [ResetSystemAction] {
    if is_wii {
        if reset == SYS_SHUTDOWN {
            return RESET_ACTIONS_WII_SHUTDOWN;
        }
        match reset_system_terminal(is_wii, reset, shutdown_mode_idle) {
            Some(ResetSystemTerminal::Reboot) => RESET_ACTIONS_WII_RESTART,
            Some(ResetSystemTerminal::ReturnToMenu) => RESET_ACTIONS_WII_RETURN_TO_MENU,
            Some(ResetSystemTerminal::ShutdownToStandby) => RESET_ACTIONS_WII_POWEROFF,
            Some(ResetSystemTerminal::ShutdownToIdle) => RESET_ACTIONS_WII_POWEROFF_IDLE,
            Some(ResetSystemTerminal::Shutdown) | Some(ResetSystemTerminal::HotReset) | None => {
                RESET_ACTIONS_WII_RETURN_TO_MENU
            }
        }
    } else {
        match reset {
            SYS_HOTRESET if force_menu => RESET_ACTIONS_GC_HOTRESET_FORCE_MENU,
            SYS_HOTRESET => RESET_ACTIONS_GC_HOTRESET,
            SYS_RESTART => RESET_ACTIONS_GC_RESTART,
            SYS_SHUTDOWN => RESET_ACTIONS_GC_SHUTDOWN,
            _ => RESET_ACTIONS_GC_RESTART,
        }
    }
}

const RESET_STEPS_GC_HOTRESET: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_GC_HOTRESET_FORCE_MENU: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::SetBootIplFlag,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_GC_RESTART: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_GC_SHUTDOWN: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::DisablePadRecalibration,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_WII_RESTART: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ShutdownIosSubsystems,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_WII_RETURN_TO_MENU: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ShutdownIosSubsystems,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_WII_POWEROFF: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::ClearActiveInputList,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ShutdownIosSubsystems,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_WII_POWEROFF_IDLE: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::ClearActiveInputList,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::InvokeTerminal,
    ResetSystemStep::ShutdownIosSubsystems,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

const RESET_STEPS_WII_SHUTDOWN: &[ResetSystemStep] = &[
    ResetSystemStep::DspShutdown,
    ResetSystemStep::DisablePadRecalibration,
    ResetSystemStep::RunResetCallbacksBeforeShutdown,
    ResetSystemStep::ShutdownIosSubsystems,
    ResetSystemStep::ShutdownIrqs,
    ResetSystemStep::RunResetCallbacksAfterShutdown,
    ResetSystemStep::DisableL2Cache,
    ResetSystemStep::ClearLowMemory,
    ResetSystemStep::RestorePadRecalibration,
];

pub fn reset_system_steps(
    is_wii: bool,
    reset: u32,
    force_menu: bool,
    shutdown_mode_idle: bool,
) -> &'static [ResetSystemStep] {
    if is_wii {
        if reset == SYS_SHUTDOWN {
            return RESET_STEPS_WII_SHUTDOWN;
        }
        match reset_system_terminal(is_wii, reset, shutdown_mode_idle) {
            Some(ResetSystemTerminal::Reboot) => RESET_STEPS_WII_RESTART,
            Some(ResetSystemTerminal::ReturnToMenu) => RESET_STEPS_WII_RETURN_TO_MENU,
            Some(ResetSystemTerminal::ShutdownToStandby) => RESET_STEPS_WII_POWEROFF,
            Some(ResetSystemTerminal::ShutdownToIdle) => RESET_STEPS_WII_POWEROFF_IDLE,
            Some(ResetSystemTerminal::Shutdown) | Some(ResetSystemTerminal::HotReset) | None => {
                RESET_STEPS_WII_RETURN_TO_MENU
            }
        }
    } else {
        match reset {
            SYS_HOTRESET if force_menu => RESET_STEPS_GC_HOTRESET_FORCE_MENU,
            SYS_HOTRESET => RESET_STEPS_GC_HOTRESET,
            SYS_RESTART => RESET_STEPS_GC_RESTART,
            SYS_SHUTDOWN => RESET_STEPS_GC_SHUTDOWN,
            _ => RESET_STEPS_GC_RESTART,
        }
    }
}

pub fn power_callback_due(now: u64, power_enable_time: u64) -> bool {
    now >= power_enable_time
}

pub fn next_power_enable_time(now: u64, cooldown_ticks: u64) -> u64 {
    now.wrapping_add(cooldown_ticks)
}

const INIT_STEPS_GC: &[SystemInitStep] = &[
    SystemInitStep::PreInit,
    SystemInitStep::InstallPanicHandler,
    SystemInitStep::MarkInitialized,
    SystemInitStep::LowMemInit,
    SystemInitStep::LwpWorkspaceInit,
    SystemInitStep::ResetQueueInit,
    SystemInitStep::ExiInit,
    SystemInitStep::SramInit,
    SystemInitStep::SiInit,
    SystemInitStep::DspBootstrap,
    SystemInitStep::MemProtectInit,
    SystemInitStep::InitCallbacks,
    SystemInitStep::ResetIrqInit,
];

const INIT_STEPS_GC_IPL: &[SystemInitStep] = &[
    SystemInitStep::PreInit,
    SystemInitStep::InstallPanicHandler,
    SystemInitStep::MarkInitialized,
    SystemInitStep::LowMemInit,
    SystemInitStep::LwpWorkspaceInit,
    SystemInitStep::ResetQueueInit,
    SystemInitStep::ExiInit,
    SystemInitStep::SramInit,
    SystemInitStep::SiInit,
    SystemInitStep::DspBootstrap,
    SystemInitStep::InitCallbacks,
    SystemInitStep::ResetIrqInit,
];

const INIT_STEPS_WII: &[SystemInitStep] = &[
    SystemInitStep::PreInit,
    SystemInitStep::InstallPanicHandler,
    SystemInitStep::MarkInitialized,
    SystemInitStep::LowMemInit,
    SystemInitStep::IpcBufferInit,
    SystemInitStep::LwpWorkspaceInit,
    SystemInitStep::ResetQueueInit,
    SystemInitStep::ExiInit,
    SystemInitStep::SramInit,
    SystemInitStep::SiInit,
    SystemInitStep::DspBootstrap,
    SystemInitStep::MemProtectInit,
    SystemInitStep::InitCallbacks,
    SystemInitStep::IpcClientInit,
];

const INIT_STEPS_WII_IPL: &[SystemInitStep] = &[
    SystemInitStep::PreInit,
    SystemInitStep::InstallPanicHandler,
    SystemInitStep::MarkInitialized,
    SystemInitStep::LowMemInit,
    SystemInitStep::IpcBufferInit,
    SystemInitStep::LwpWorkspaceInit,
    SystemInitStep::ResetQueueInit,
    SystemInitStep::ExiInit,
    SystemInitStep::SramInit,
    SystemInitStep::SiInit,
    SystemInitStep::DspBootstrap,
    SystemInitStep::InitCallbacks,
    SystemInitStep::IpcClientInit,
];

const INIT_STEPS_ALREADY_INITIALIZED: &[SystemInitStep] =
    &[SystemInitStep::PreInit, SystemInitStep::InstallPanicHandler];

const PREMAIN_STEPS_GC: &[SystemPreMainStep] = &[SystemPreMainStep::SetBootTime];

const PREMAIN_STEPS_WII: &[SystemPreMainStep] = &[
    SystemPreMainStep::CloseIosFdRange,
    SystemPreMainStep::LoadStartupIos,
    SystemPreMainStep::InitializeIosSubsystems,
    SystemPreMainStep::RegisterStmEventHandler,
    SystemPreMainStep::ConfInit,
    SystemPreMainStep::WiiInitialize,
    SystemPreMainStep::SetBootTime,
];

pub fn system_init_sequence(
    is_wii: bool,
    in_ipl: bool,
    already_initialized: bool,
) -> &'static [SystemInitStep] {
    if already_initialized {
        return INIT_STEPS_ALREADY_INITIALIZED;
    }
    match (is_wii, in_ipl) {
        (false, false) => INIT_STEPS_GC,
        (false, true) => INIT_STEPS_GC_IPL,
        (true, false) => INIT_STEPS_WII,
        (true, true) => INIT_STEPS_WII_IPL,
    }
}

pub fn system_premain_sequence(is_wii: bool) -> &'static [SystemPreMainStep] {
    if is_wii {
        PREMAIN_STEPS_WII
    } else {
        PREMAIN_STEPS_GC
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};

    unsafe extern "C" fn reset_ok(_final_: i32) -> i32 {
        1
    }

    unsafe extern "C" fn reset_fail(_final_: i32) -> i32 {
        0
    }

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        TestGuard
    }

    const FONT_TEST_BYTES: usize = SYS_FONTSIZE_SJIS as usize;

    #[repr(align(32))]
    struct FontBuffer([u8; FONT_TEST_BYTES]);

    static mut FONT_BUFFER: FontBuffer = FontBuffer([0; FONT_TEST_BYTES]);

    #[test]
    fn public_layout_matches_c() {
        assert_eq!(core::mem::size_of::<SysSram>(), 20);
        assert_eq!(core::mem::align_of::<SysSram>(), 4);
        assert_eq!(core::mem::size_of::<SysSramEx>(), 44);
        assert_eq!(core::mem::align_of::<SysSramEx>(), 1);
        assert_eq!(core::mem::size_of::<SysFontHeader>(), 48);
        assert_eq!(core::mem::align_of::<SysFontHeader>(), 1);
        assert_eq!(
            core::mem::size_of::<LwpNode>(),
            core::mem::size_of::<usize>() * 2
        );
        let reset_info_payload =
            core::mem::size_of::<LwpNode>() + core::mem::size_of::<ResetFunction>() + 4;
        let reset_info_align = core::mem::align_of::<SysResetInfo>();
        let reset_info_size = reset_info_payload.next_multiple_of(reset_info_align);
        assert_eq!(core::mem::size_of::<SysResetInfo>(), reset_info_size);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn();
        let _ = pre_main as fn();
        let _ = main_loop as fn() -> bool;
        let _ = main_loop_decision as fn(bool, bool, bool, bool) -> MainLoopDecision;
        let _ = get_counter_bias as fn() -> u32;
        let _ = set_counter_bias as fn(u32);
        let _ = get_display_offset_h as fn() -> i8;
        let _ = set_display_offset_h as fn(i8);
        let _ = snapshot_config as fn() -> SystemConfigSnapshot;
        let _ = get_language as fn() -> u8;
        let _ = set_language as fn(u8);
        let _ = get_font_encoding as fn() -> u32;
        let _ = get_arena1_lo as fn() -> *mut c_void;
        let _ = set_arena1_lo as unsafe fn(*mut c_void);
        let _ = get_arena1_size as fn() -> u32;
        let _ = alloc_arena1_mem_lo as fn(u32, u32) -> *mut c_void;
        let _ = snapshot_framebuffer as fn() -> FramebufferSnapshot;
        let _ = snapshot_arenas as fn() -> ArenaSnapshot;
        let _ = snapshot_callbacks as fn() -> CallbackSnapshot;
        let _ = snapshot_alarms as fn() -> AlarmSnapshot;
        let _ = snapshot_hardware as fn() -> HardwareSnapshot;
        let _ = snapshot_startup as fn() -> StartupSnapshot;
        let _ = snapshot_power_dispatch as fn() -> PowerDispatchSnapshot;
        let _ = snapshot_reset_registration as fn() -> ResetRegistrationSnapshot;
        let _ = snapshot_runtime_calls as fn() -> RuntimeCallSnapshot;
        let _ = snapshot as fn() -> SystemSnapshot;
        let _ = set_reset_callback as fn(ResetCallback) -> ResetCallback;
        let _ = get_hollywood_revision as fn() -> u32;
        #[cfg(not(gamecube))]
        let _ = alloc_arena2_mem_lo as fn(u32, u32) -> *mut c_void;
        let _ = set_power_callback as fn(PowerCallback) -> PowerCallback;
        let _ = time as fn() -> u64;
        let _ = snapshot_time as fn() -> TimeSnapshot;
        let _ = diet_print_set_func as fn(DietPrintFn);
        let _ = stdio_report as fn(bool);
        let _ = stdio_report_uses_stdout as fn() -> bool;
        let _ = sync_sram as fn() -> bool;
        let _ = snapshot_sram_sync as fn() -> SramSyncSnapshot;
        let _ = snapshot_sram as fn() -> Option<SysSram>;
        let _ = snapshot_sram_ex as fn() -> Option<SysSramEx>;
        let _ = reset_system_plan as fn(bool, u32, bool, bool) -> ResetSystemPlan;
        let _ = reset_system_actions as fn(bool, u32, bool, bool) -> &'static [ResetSystemAction];
        let _ = reset_system_steps as fn(bool, u32, bool, bool) -> &'static [ResetSystemStep];
        let _ = SystemRuntimeState::new as fn(bool) -> SystemRuntimeState;
    }

    #[test]
    fn checksum_matches_big_endian_word_layout() {
        let sram = SysSram {
            checksum: 0,
            checksum_inv: 0,
            ead0: 0,
            ead1: 0,
            counter_bias: 0x1122_3344,
            display_offset_h: -2,
            ntd: 0x40,
            lang: 5,
            flags: 0x87,
        };
        assert_eq!(sram_checksum(&sram), (0x482d, 0xb7cf));
    }

    #[test]
    fn refresh_checksum_normalizes_invalid_video_mode_bits() {
        assert_eq!(normalize_sram_flags(0x07), 0x04);
        let mut sram = SysSram {
            checksum: 0,
            checksum_inv: 0,
            ead0: 0,
            ead1: 0,
            counter_bias: 0x0102_0304,
            display_offset_h: 1,
            ntd: 0,
            lang: 2,
            flags: 0x07,
        };
        refresh_sram_checksum(&mut sram);
        assert_eq!(sram_video_mode(&sram), 0);
        assert_eq!(sram_sound_mode(&sram), 1);
        assert_eq!((sram.checksum, sram.checksum_inv), sram_checksum(&sram));
    }

    #[test]
    fn sram_setters_only_mark_changes_and_apply_bit_rules() {
        let mut sram = SysSram::default();
        assert!(!set_sram_counter_bias(&mut sram, 0));
        assert!(set_sram_counter_bias(&mut sram, 7));
        assert!(set_sram_display_offset_h(&mut sram, -4));
        assert!(set_sram_eu_rgb60(&mut sram, 3));
        assert_eq!(sram_eu_rgb60(&sram), 1);
        assert!(set_sram_progressive_scan(&mut sram, 1));
        assert_eq!(sram_progressive_scan(&sram), 1);
        assert!(set_sram_sound_mode(&mut sram, 1));
        assert_eq!(sram_sound_mode(&sram), 1);
        assert!(set_sram_video_mode(&mut sram, 7));
        assert_eq!(sram_video_mode(&sram), 3);
        assert!(set_sram_language(&mut sram, SYS_LANG_DUTCH as u8));
        assert_eq!(sram_language(&sram), SYS_LANG_DUTCH as u8);
    }

    #[test]
    fn extended_sram_helpers_match_c_rules() {
        let mut sram_ex = SysSramEx::default();
        assert!(set_sram_ex_wireless_id(&mut sram_ex, 2, 0x55aa));
        assert_eq!(sram_ex_wireless_id(&sram_ex, 2), 0x55aa);
        assert_eq!(sanitize_gbs_mode(0b10100 << 10), 0);
        assert!(set_sram_ex_gbs_mode(&mut sram_ex, 0x1234));
        assert_eq!(sram_ex_gbs_mode(&sram_ex), sanitize_gbs_mode(0x1234));
        let current_mode = sram_ex_gbs_mode(&sram_ex);
        assert!(!set_sram_ex_gbs_mode(&mut sram_ex, current_mode));
    }

    #[test]
    fn stdio_report_tracks_stdout_flag() {
        stdio_report(false);
        assert!(!stdio_report_uses_stdout());
        stdio_report(true);
        assert!(stdio_report_uses_stdout());
    }

    #[test]
    fn sram_sync_snapshot_tracks_last_attempt() {
        let _ = sync_sram();
        assert_eq!(
            snapshot_sram_sync(),
            SramSyncSnapshot {
                called: true,
                last_result: false,
            }
        );
    }

    #[test]
    fn sram_snapshots_track_host_buffers() {
        let _guard = test_guard();
        assert_eq!(snapshot_sram(), Some(SysSram::default()));
        assert_eq!(snapshot_sram_ex(), Some(SysSramEx::default()));
    }

    #[test]
    fn system_config_snapshot_tracks_rust_owned_state() {
        let _ = get_counter_bias();
        stdio_report(true);
        set_counter_bias(0x1122_3344);
        let _ = get_display_offset_h();
        set_display_offset_h(-3);
        let _ = get_eu_rgb60();
        set_eu_rgb60(1);
        let _ = get_language();
        set_language(SYS_LANG_GERMAN as u8);
        let _ = get_progressive_scan();
        set_progressive_scan(1);
        set_sound_mode(1);
        set_video_mode(2);
        set_wireless_id(0, 0x1111);
        set_wireless_id(1, 0x2222);
        set_wireless_id(2, 0x3333);
        set_wireless_id(3, 0x4444);
        set_gbs_mode(0x1234);

        assert_eq!(
            snapshot_config(),
            SystemConfigSnapshot {
                counter_bias: 0x1122_3344,
                counter_bias_get_called: true,
                counter_bias_set_called: true,
                display_offset_h: -3,
                display_offset_get_called: true,
                display_offset_set_called: true,
                eu_rgb60: 1,
                eu_rgb60_get_called: true,
                eu_rgb60_set_called: true,
                language: SYS_LANG_GERMAN as u8,
                language_get_called: true,
                language_set_called: true,
                progressive_scan: 1,
                progressive_scan_get_called: true,
                progressive_scan_set_called: true,
                sound_mode: 1,
                video_mode: 2,
                wireless_id: [0x1111, 0x2222, 0x3333, 0x4444],
                gbs_mode: sanitize_gbs_mode(0x1234),
                stdout_report: true,
            }
        );
    }

    #[test]
    fn arena_low_allocation_matches_libogc_math() {
        let (ptr, next_lo) = arena_low_alloc_from(0x1003, 0x25, 0x20);
        assert_eq!(ptr, 0x1020);
        assert_eq!(next_lo, 0x1060);

        assert_eq!(arena_low_alloc_from(usize::MAX - 3, 16, 16), (0, 16));
        assert_eq!(arena_low_alloc_from(0x1234, 16, 0), (0, 0));
    }

    #[test]
    fn framebuffer_snapshot_defaults_to_zero() {
        assert_eq!(snapshot_framebuffer(), FramebufferSnapshot::default());
    }

    #[test]
    fn framebuffer_snapshot_tracks_host_calls() {
        let _guard = test_guard();
        let previous_bytes = LAST_FRAMEBUFFER_BYTES.load(Ordering::Acquire);
        unsafe {
            let mut rmode: GxrModeObj = core::mem::zeroed();
            let _ = allocate_framebuffer(&mut rmode);
            let snap = snapshot_framebuffer();
            assert_eq!(
                snap.last_request_rmode,
                core::ptr::addr_of_mut!(rmode) as usize
            );
            assert_eq!(snap.last_request_bytes, 0);
        }
        LAST_FRAMEBUFFER_BYTES.store(previous_bytes, Ordering::Release);
        let _ = unsafe { sys::sys_allocate_framebuffer(core::ptr::null_mut()) };
    }

    #[test]
    fn arena_snapshot_defaults_to_zero() {
        let _guard = test_guard();
        assert_eq!(snapshot_arenas(), ArenaSnapshot::default());
    }

    #[test]
    fn callback_snapshot_tracks_rust_owned_registration_state() {
        unsafe extern "C" fn reset_cb(_irq: u32, _ctx: *mut c_void) {}
        unsafe extern "C" fn power_cb() {}
        unsafe extern "C" fn diet_cb(_buf: *const core::ffi::c_char, _size: usize) {}
        unsafe extern "C" fn alarm_cb(_alarm: SysWd, _cb_arg: *mut c_void) {}
        let reset_cb_ptr = reset_cb as *const () as usize;
        let power_cb_ptr = power_cb as *const () as usize;

        assert!(set_reset_callback(Some(reset_cb)).is_none());
        assert_eq!(
            set_reset_callback(None).map(|cb| cb as *const () as usize),
            Some(reset_cb_ptr)
        );
        assert!(set_power_callback(Some(power_cb)).is_none());
        assert_eq!(
            set_power_callback(None).map(|cb| cb as *const () as usize),
            Some(power_cb_ptr)
        );
        let _ = set_reset_callback(Some(reset_cb));
        let _ = set_power_callback(Some(power_cb));
        diet_print_set_func(Some(diet_cb));
        let mut alarm = 0;
        let _ = create_alarm(&mut alarm);
        let _ = unsafe {
            set_alarm(
                alarm,
                &TimeSpec {
                    tv_sec: 1,
                    tv_nsec: 2,
                },
                Some(alarm_cb),
                core::ptr::null_mut(),
            )
        };
        let _ = unsafe {
            set_periodic_alarm(
                alarm,
                &TimeSpec {
                    tv_sec: 3,
                    tv_nsec: 4,
                },
                &TimeSpec {
                    tv_sec: 5,
                    tv_nsec: 6,
                },
                Some(alarm_cb),
                core::ptr::null_mut(),
            )
        };

        let snap = snapshot_callbacks();
        assert_ne!(snap.reset_callback, 0);
        assert_ne!(snap.power_callback, 0);
        assert_ne!(snap.diet_print_fn, 0);

        let alarm_snap = snapshot_alarms();
        assert_ne!(alarm_snap.alarm, 0);
        assert_ne!(alarm_snap.callback, 0);
        assert_ne!(alarm_snap.periodic_callback, 0);

        let _ = set_reset_callback(None);
        let _ = set_power_callback(None);
        diet_print_set_func(None);
        let cleared = snapshot_callbacks();
        assert_eq!(cleared.reset_callback, 0);
        assert_eq!(cleared.power_callback, 0);
        assert_eq!(cleared.diet_print_fn, 0);
    }

    #[test]
    fn hardware_snapshot_tracks_recent_readbacks() {
        let _ = get_font_encoding();
        let _ = reset_button_down();
        let _ = get_hollywood_revision();
        let snap = snapshot_hardware();
        assert_eq!(snap.font_encoding, get_font_encoding());
        assert_eq!(snap.reset_button_down, reset_button_down());
        assert_eq!(snap.hollywood_revision, get_hollywood_revision());
        assert!(snap.font_encoding_read_called);
        assert!(snap.reset_button_read_called);
        assert!(snap.hollywood_revision_read_called);
    }

    #[test]
    fn font_snapshot_tracks_host_calls() {
        let _guard = test_guard();
        let _ = get_font_encoding();
        unsafe {
            let font_ptr = core::ptr::addr_of_mut!(FONT_BUFFER.0).cast::<SysFontHeader>();
            let init_rc = init_font(font_ptr);
            assert!(init_rc <= 1);
            let mut image: *mut c_void = core::ptr::null_mut();
            let mut xpos = 0;
            let mut ypos = 0;
            let mut width = 0;
            get_font_texture(7, &mut image, &mut xpos, &mut ypos, &mut width);
            if init_rc == 1 {
                assert!(!image.is_null());
                assert!(width > 0);
            } else {
                assert!(image.is_null());
                assert_eq!(xpos, 0);
                assert_eq!(ypos, 0);
                assert_eq!(width, 0);
            }

            let mut texel = [0u8; 32 * 1024];
            let mut texel_width = 0;
            get_font_texel(9, texel.as_mut_ptr().cast(), 0, 64, &mut texel_width);
            if init_rc == 1 {
                assert!(texel_width > 0);
                assert!(texel.iter().any(|&byte| byte != 0));
            } else {
                assert_eq!(texel_width, 0);
            }
        }
        assert_eq!(
            snapshot_font(),
            FontSnapshot {
                encoding_queried: true,
                init_called: true,
                init_ptr: unsafe { core::ptr::addr_of_mut!(FONT_BUFFER.0) as usize },
                texture_called: true,
                texture_c: 7,
                texel_called: true,
                texel_c: 9,
            }
        );
    }

    #[test]
    fn time_snapshot_tracks_recent_readback() {
        let value = time();
        assert_eq!(
            snapshot_time(),
            TimeSnapshot {
                last_time_read: value,
                time_read_called: true,
            }
        );
    }

    #[test]
    fn runtime_call_snapshot_tracks_entry_points() {
        init();
        pre_main();
        let _ = main_loop();
        let snap = snapshot_runtime_calls();
        assert!(snap.init_called);
        assert!(snap.pre_main_called);
        assert!(snap.main_loop_called);
        assert!(!snap.main_loop_result);
    }

    #[test]
    fn startup_snapshot_tracks_low_mem_init() {
        init();
        let startup = snapshot_startup();
        assert!(startup.init_called);
        assert!(startup.low_mem_init_called);
        assert!(startup.init_started);
        assert!(startup.init_completed);
    }

    #[test]
    fn power_dispatch_snapshot_tracks_host_calls() {
        let _guard = test_guard();
        unsafe extern "C" fn power_cb() {}
        let _ = set_power_callback(Some(power_cb));
        do_power_cb();
        assert_eq!(
            snapshot_power_dispatch(),
            PowerDispatchSnapshot {
                called: true,
                result: true,
            }
        );
        let _ = set_power_callback(None);
    }

    #[test]
    fn reset_registration_snapshot_tracks_host_calls() {
        let _guard = test_guard();
        let mut info: SysResetInfo = unsafe { core::mem::zeroed() };
        unsafe {
            register_reset_func(&mut info);
            unregister_reset_func(&mut info);
        }
        assert_eq!(
            snapshot_reset_registration(),
            ResetRegistrationSnapshot {
                register_called: true,
                unregister_called: true,
                register_ptr: core::ptr::addr_of_mut!(info) as usize,
                unregister_ptr: core::ptr::addr_of_mut!(info) as usize,
            }
        );
    }

    #[test]
    fn reset_system_snapshot_tracks_host_calls() {
        unsafe {
            reset_system(SYS_RESTART as i32, 0x1234_5678, 1);
        }
        assert_eq!(
            snapshot_reset_system_call(),
            ResetSystemCallSnapshot {
                called: true,
                reset: SYS_RESTART,
                reset_code: 0x1234_5678,
                force_menu: true,
                terminal_hint: 1,
            }
        );
    }

    #[test]
    fn protect_range_snapshot_tracks_host_calls() {
        unsafe {
            protect_range(
                SYS_PROTECTCHAN2,
                0x1234 as *mut c_void,
                0x80,
                SYS_PROTECTRDWR,
            );
        }
        assert_eq!(
            snapshot_protect_range(),
            ProtectRangeSnapshot {
                called: true,
                chan: SYS_PROTECTCHAN2,
                addr: 0x1234 as *mut c_void,
                bytes: 0x80,
                cntrl: SYS_PROTECTRDWR,
            }
        );
    }

    #[test]
    fn pmc_snapshot_tracks_host_calls() {
        start_pmc(0xA5A5_0001, 0x5A5A_0002);
        dump_pmc();
        stop_pmc();
        reset_pmc();
        assert_eq!(
            snapshot_pmc(),
            PmcSnapshot {
                started: false,
                dumped: true,
                stopped: true,
                reset: true,
                mcr0val: 0xA5A5_0001,
                mcr1val: 0x5A5A_0002,
            }
        );
    }

    #[test]
    #[cfg(not(gamecube))]
    fn arena_sizes_track_host_bounds() {
        let _guard = test_guard();
        let original = snapshot_arenas();
        unsafe {
            set_arena1_lo(0x1000 as *mut c_void);
            set_arena1_hi(0x1800 as *mut c_void);
            set_arena2_lo(0x2000 as *mut c_void);
            set_arena2_hi(0x2800 as *mut c_void);
        }
        assert_eq!(get_arena1_size(), 0x800);
        assert_eq!(get_arena2_size(), 0x800);
        unsafe {
            set_arena1_lo(original.arena1_lo);
            set_arena1_hi(original.arena1_hi);
            set_arena2_lo(original.arena2_lo);
            set_arena2_hi(original.arena2_hi);
        }
    }

    #[test]
    #[cfg(not(gamecube))]
    fn arena_allocation_updates_host_bounds() {
        let _guard = test_guard();
        let original = snapshot_arenas();
        unsafe {
            set_arena1_lo(0x1000 as *mut c_void);
            set_arena1_hi(0x1800 as *mut c_void);
            set_arena2_lo(0x2000 as *mut c_void);
            set_arena2_hi(0x2800 as *mut c_void);
        }

        let ptr1 = alloc_arena1_mem_lo(0x100, 0x20) as usize;
        let ptr2 = alloc_arena2_mem_lo(0x180, 0x40) as usize;
        assert_eq!(ptr1, 0x1000);
        assert_eq!(ptr2, 0x2000);
        assert_eq!(get_arena1_lo() as usize, 0x1100);
        assert_eq!(get_arena2_lo() as usize, 0x2180);

        unsafe {
            set_arena1_lo(original.arena1_lo);
            set_arena1_hi(original.arena1_hi);
            set_arena2_lo(original.arena2_lo);
            set_arena2_hi(original.arena2_hi);
        }
    }

    #[test]
    fn composite_snapshot_tracks_all_owned_state_layers() {
        let snap = snapshot();
        assert_eq!(snap.config, snapshot_config());
        assert_eq!(snap.arenas, snapshot_arenas());
        assert_eq!(snap.callbacks, snapshot_callbacks());
        assert_eq!(snap.alarms, snapshot_alarms());
        assert_eq!(snap.hardware, snapshot_hardware());
        assert_eq!(snap.font, snapshot_font());
        assert_eq!(snap.startup, snapshot_startup());
        assert_eq!(snap.power_dispatch, snapshot_power_dispatch());
        assert_eq!(snap.reset_registration, snapshot_reset_registration());
        assert_eq!(snap.runtime_calls, snapshot_runtime_calls());
        assert_eq!(snap.reset_system, snapshot_reset_system_call());
        assert_eq!(snap.time, snapshot_time());
        assert_eq!(snap.sram_sync, snapshot_sram_sync());
        assert_eq!(snap.framebuffer, snapshot_framebuffer());
    }

    #[test]
    fn reset_registration_uses_ascending_priority_with_stable_equal_order() {
        let priorities = [16, 32, 32, 48];
        assert_eq!(reset_queue_insert_position(&priorities, 8), 0);
        assert_eq!(reset_queue_insert_position(&priorities, 32), 3);
        assert_eq!(reset_queue_insert_position(&priorities, 80), 4);
    }

    #[test]
    fn reset_function_result_and_sram_sync_gate_final_outcome() {
        let callbacks = [Some(reset_ok as unsafe extern "C" fn(i32) -> i32), None];
        assert!(call_reset_functions(&callbacks, 0, true));

        let callbacks = [
            Some(reset_ok as unsafe extern "C" fn(i32) -> i32),
            Some(reset_fail),
        ];
        assert!(!call_reset_functions(&callbacks, 1, true));
        assert!(!call_reset_functions(&[Some(reset_ok)], 1, false));
    }

    #[test]
    fn reset_terminal_matches_gc_and_wii_branching() {
        assert_eq!(
            reset_system_terminal(false, SYS_HOTRESET, false),
            Some(ResetSystemTerminal::HotReset)
        );
        assert_eq!(
            reset_system_terminal(false, SYS_RESTART, false),
            Some(ResetSystemTerminal::Reboot)
        );
        assert_eq!(
            reset_system_terminal(false, SYS_SHUTDOWN, false),
            Some(ResetSystemTerminal::Shutdown)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_HOTRESET, false),
            Some(ResetSystemTerminal::ReturnToMenu)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_SHUTDOWN, false),
            Some(ResetSystemTerminal::Shutdown)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_POWEROFF, false),
            Some(ResetSystemTerminal::ShutdownToStandby)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_POWEROFF, true),
            Some(ResetSystemTerminal::ShutdownToIdle)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_POWEROFF_STANDBY, false),
            Some(ResetSystemTerminal::ShutdownToStandby)
        );
        assert_eq!(
            reset_system_terminal(true, SYS_POWEROFF_IDLE, false),
            Some(ResetSystemTerminal::ShutdownToIdle)
        );
    }

    #[test]
    fn main_loop_decision_matches_gc_and_wii_exit_rules() {
        assert_eq!(
            main_loop_decision(false, false, false, false),
            MainLoopDecision::Continue
        );
        assert_eq!(
            main_loop_decision(false, false, true, false),
            MainLoopDecision::Exit(MainLoopExitReason::ResetButton)
        );
        assert_eq!(
            main_loop_decision(true, false, false, true),
            MainLoopDecision::Exit(MainLoopExitReason::PowerButton)
        );
        assert_eq!(
            main_loop_decision(true, true, false, false),
            MainLoopDecision::Exit(MainLoopExitReason::AlreadyDone)
        );
    }

    #[test]
    fn runtime_state_tracks_main_loop_and_power_transitions() {
        let mut gc = SystemRuntimeState::new(false);
        assert_eq!(gc.main_loop_step(false), MainLoopDecision::Continue);
        assert_eq!(
            gc.main_loop_step(true),
            MainLoopDecision::Exit(MainLoopExitReason::ResetButton)
        );
        assert_eq!(
            gc.main_loop_step(false),
            MainLoopDecision::Exit(MainLoopExitReason::AlreadyDone)
        );

        let mut wii = SystemRuntimeState::new(true);
        assert_eq!(wii.main_loop_step(false), MainLoopDecision::Continue);
        wii.note_stm_event(StmEvent::Reset, true);
        assert_eq!(
            wii.main_loop_step(false),
            MainLoopDecision::Exit(MainLoopExitReason::PowerButton)
        );
    }

    #[test]
    fn runtime_state_power_callback_attempt_updates_cooldown_every_time() {
        let mut state = SystemRuntimeState::new(true);
        assert!(state.power_callback_attempt(100, 50));
        assert_eq!(state.power_enable_time, 150);
        assert!(!state.power_callback_attempt(120, 50));
        assert_eq!(state.power_enable_time, 170);
        assert!(state.power_callback_attempt(170, 50));
        assert_eq!(state.power_enable_time, 220);
    }

    #[test]
    fn runtime_state_stm_events_only_set_power_state_once() {
        let mut state = SystemRuntimeState::new(true);
        state.note_stm_event(StmEvent::Power, false);
        assert_eq!(state.power_button_state, PowerButtonState::PowerPressed);
        state.note_stm_event(StmEvent::Reset, true);
        assert_eq!(state.power_button_state, PowerButtonState::PowerPressed);
        let mut reset_state = SystemRuntimeState::new(false);
        reset_state.note_stm_event(StmEvent::Reset, true);
        assert_eq!(
            reset_state.power_button_state,
            PowerButtonState::ResetPressed
        );
    }

    #[test]
    fn reset_plan_tracks_boot_ipl_and_input_shutdown_paths() {
        assert_eq!(
            reset_system_plan(false, SYS_HOTRESET, true, false),
            ResetSystemPlan {
                terminal: Some(ResetSystemTerminal::HotReset),
                sets_boot_ipl: true,
                disables_pad_recalibration: false,
                clears_active_input_list: false,
            }
        );
        assert_eq!(
            reset_system_plan(true, SYS_POWEROFF, false, true),
            ResetSystemPlan {
                terminal: Some(ResetSystemTerminal::ShutdownToIdle),
                sets_boot_ipl: false,
                disables_pad_recalibration: false,
                clears_active_input_list: true,
            }
        );
        assert_eq!(
            reset_system_plan(true, SYS_SHUTDOWN, false, false),
            ResetSystemPlan {
                terminal: Some(ResetSystemTerminal::Shutdown),
                sets_boot_ipl: false,
                disables_pad_recalibration: true,
                clears_active_input_list: false,
            }
        );
    }

    #[test]
    fn reset_action_sequences_track_gc_and_wii_reset_flow() {
        assert_eq!(
            reset_system_actions(false, SYS_HOTRESET, false, false),
            RESET_ACTIONS_GC_HOTRESET
        );
        assert_eq!(
            reset_system_actions(false, SYS_HOTRESET, true, false),
            RESET_ACTIONS_GC_HOTRESET_FORCE_MENU
        );
        assert_eq!(
            reset_system_actions(false, SYS_SHUTDOWN, false, false),
            RESET_ACTIONS_GC_SHUTDOWN
        );
        assert_eq!(
            reset_system_actions(true, SYS_POWEROFF, false, false),
            RESET_ACTIONS_WII_POWEROFF
        );
        assert_eq!(
            reset_system_actions(true, SYS_POWEROFF, false, true),
            RESET_ACTIONS_WII_POWEROFF_IDLE
        );
        assert_eq!(
            reset_system_actions(true, SYS_RESTART, false, false),
            RESET_ACTIONS_WII_RESTART
        );
        assert_eq!(
            reset_system_actions(true, SYS_RETURNTOMENU, false, false),
            RESET_ACTIONS_WII_RETURN_TO_MENU
        );
        assert_eq!(
            reset_system_actions(true, SYS_POWEROFF_STANDBY, false, false),
            RESET_ACTIONS_WII_POWEROFF
        );
        assert_eq!(
            reset_system_actions(true, SYS_POWEROFF_IDLE, false, true),
            RESET_ACTIONS_WII_POWEROFF_IDLE
        );
        assert_eq!(
            reset_system_actions(true, SYS_SHUTDOWN, false, false),
            RESET_ACTIONS_WII_SHUTDOWN
        );
    }

    #[test]
    fn reset_step_sequences_track_gc_and_wii_reset_flow() {
        assert_eq!(
            reset_system_steps(false, SYS_HOTRESET, false, false),
            RESET_STEPS_GC_HOTRESET
        );
        assert_eq!(
            reset_system_steps(false, SYS_HOTRESET, true, false),
            RESET_STEPS_GC_HOTRESET_FORCE_MENU
        );
        assert_eq!(
            reset_system_steps(false, SYS_SHUTDOWN, false, false),
            RESET_STEPS_GC_SHUTDOWN
        );
        assert_eq!(
            reset_system_steps(true, SYS_POWEROFF, false, false),
            RESET_STEPS_WII_POWEROFF
        );
        assert_eq!(
            reset_system_steps(true, SYS_POWEROFF, false, true),
            RESET_STEPS_WII_POWEROFF_IDLE
        );
        assert_eq!(
            reset_system_steps(true, SYS_RESTART, false, false),
            RESET_STEPS_WII_RESTART
        );
        assert_eq!(
            reset_system_steps(true, SYS_RETURNTOMENU, false, false),
            RESET_STEPS_WII_RETURN_TO_MENU
        );
        assert_eq!(
            reset_system_steps(true, SYS_SHUTDOWN, false, false),
            RESET_STEPS_WII_SHUTDOWN
        );
    }

    #[test]
    fn runtime_state_delegates_reset_plan() {
        let state = SystemRuntimeState::new(true);
        assert_eq!(
            state.reset_plan(SYS_POWEROFF_IDLE, false, false),
            reset_system_plan(true, SYS_POWEROFF_IDLE, false, false)
        );
    }

    #[test]
    fn power_callback_throttle_uses_due_check_and_cooldown_window() {
        assert!(!power_callback_due(99, 100));
        assert!(power_callback_due(100, 100));
        assert_eq!(next_power_enable_time(500, 50), 550);
    }

    #[test]
    fn init_sequence_matches_gc_and_wii_ordering() {
        assert_eq!(system_init_sequence(false, false, false), INIT_STEPS_GC);
        assert_eq!(system_init_sequence(false, true, false), INIT_STEPS_GC_IPL);
        assert_eq!(system_init_sequence(true, false, false), INIT_STEPS_WII);
        assert_eq!(system_init_sequence(true, true, false), INIT_STEPS_WII_IPL);
        assert_eq!(
            system_init_sequence(true, false, true),
            INIT_STEPS_ALREADY_INITIALIZED
        );
    }

    #[test]
    fn premain_sequence_matches_gc_and_wii_ordering() {
        assert_eq!(system_premain_sequence(false), PREMAIN_STEPS_GC);
        assert_eq!(system_premain_sequence(true), PREMAIN_STEPS_WII);
    }

    #[test]
    fn system_setting_getters_read_sram_backends() {
        let counter_bias = get_counter_bias();
        let display_offset_h = get_display_offset_h();
        let eu_rgb60 = get_eu_rgb60();
        let language = get_language();
        let progressive_scan = get_progressive_scan();
        let sound_mode = get_sound_mode();

        set_counter_bias(counter_bias);
        set_display_offset_h(display_offset_h);
        set_eu_rgb60(eu_rgb60);
        set_language(language);
        set_progressive_scan(progressive_scan);
        set_sound_mode(sound_mode);

        assert!(sys::snapshot_counter_bias_probe().get_called);
        assert!(sys::snapshot_counter_bias_probe().set_called);
        assert!(sys::snapshot_display_offset_probe().get_called);
        assert!(sys::snapshot_display_offset_probe().set_called);
        assert!(sys::snapshot_eu_rgb60_probe().get_called);
        assert!(sys::snapshot_eu_rgb60_probe().set_called);
        assert!(sys::snapshot_language_probe().get_called);
        assert!(sys::snapshot_language_probe().set_called);
        assert!(sys::snapshot_progressive_scan_probe().get_called);
        assert!(sys::snapshot_progressive_scan_probe().set_called);
    }
}
