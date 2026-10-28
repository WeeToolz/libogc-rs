#![no_std]

mod clock;
#[cfg(ogc_dev)]
mod dev;
#[cfg(ogc_dev)]
mod dev_protocol;
mod rtc;
mod startup_plan;
pub use clock::{ClockError, ClockSnapshot, RtcStage};
#[cfg(ogc_dev)]
pub use dev::capture as capture_dev_report;
#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
use core::arch::asm;

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.__ogc_ppc_hot_reset,"ax",@progbits
    .balign 32
    .globl __ogc_ppc_hot_reset
    .type __ogc_ppc_hot_reset,@function
__ogc_ppc_hot_reset:
    b .Lhotreset_cache_ready
.Lhotreset_enable_broadcast:
    mfspr 8, 1008
    ori 8, 8, 8
    mtspr 1008, 8
    isync
    sync
    nop
    b .Lhotreset_delay
.Lhotreset_cache_ready:
    b .Lhotreset_entered_cache
.Lhotreset_delay:
    mftb 5
.Lhotreset_delay_loop:
    mftb 6
    subf 7, 5, 6
    cmplwi 7, 0x1124
    blt .Lhotreset_delay_loop
    nop
    b .Lhotreset_write_reset
.Lhotreset_entered_cache:
    b .Lhotreset_enable_broadcast
.Lhotreset_write_reset:
    lis 8, 0xcc00
    ori 8, 8, 0x3000
    li 4, 3
    stw 4, 0x24(8)
    stw 3, 0x24(8)
    nop
    b .Lhotreset_halt
.Lhotreset_halt:
    nop
    b .Lhotreset_halt
    .size __ogc_ppc_hot_reset, .-__ogc_ppc_hot_reset
"#
);
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
use core::ffi::CStr;
use core::ffi::{c_char, c_void};
#[cfg(any(not(gamecube), test))]
use core::mem::offset_of;
use core::mem::{align_of, size_of};
use core::ptr::{
    copy, null_mut, read_unaligned, read_volatile, write_bytes, write_unaligned, write_volatile,
};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_alloc::ArenaAllocator;
#[cfg(target_arch = "powerpc")]
use ogc_cache as cache;
#[cfg(target_arch = "powerpc")]
use ogc_conf as conf;
#[cfg(target_arch = "powerpc")]
use ogc_dsp as dsp;
#[cfg(target_arch = "powerpc")]
use ogc_exi::{
    self as exi, EXI_CHANNEL_0, EXI_DEVICE_1, EXI_READ, EXI_SPEED8MHZ, EXI_SPEED32MHZ, EXI_WRITE,
};
#[cfg(all(target_arch = "powerpc", not(gamecube)))]
use ogc_ios_sys as ios;
#[cfg(target_arch = "powerpc")]
use ogc_ipc as ipc;
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
use ogc_ipc::{IPC_OPEN_NONE, IoctlBuffers, open_file};
#[cfg(target_arch = "powerpc")]
use ogc_irq as irq;
#[cfg(target_arch = "powerpc")]
use ogc_lwp_sys as lwp;
#[cfg(target_arch = "powerpc")]
use ogc_si as si;
#[cfg(all(target_arch = "powerpc", not(gamecube)))]
use ogc_wiilaunch as wiilaunch;
#[cfg(target_arch = "powerpc")]
use wpad_sys as wpad;

#[cfg(not(target_arch = "powerpc"))]
use core::sync::atomic::AtomicU64;

pub use ogc_sys::Bool;
pub use ogc_video_sys::GxrModeObj;

pub const SYS_BASE_CACHED: u32 = 0x8000_0000;
pub const SYS_BASE_UNCACHED: u32 = 0xc000_0000;
pub const SYS_WD_NULL: u32 = 0xffff_ffff;
pub const SYS_PRE_MAIN_ESTM: i32 = -0x2200;

pub const SYS_RESTART: u32 = 0;
pub const SYS_HOTRESET: u32 = 1;
pub const SYS_SHUTDOWN: u32 = 2;
pub const SYS_RETURNTOMENU: u32 = 3;
pub const SYS_POWEROFF: u32 = 4;
pub const SYS_POWEROFF_STANDBY: u32 = 5;
pub const SYS_POWEROFF_IDLE: u32 = 6;

pub const SYS_PROTECTCHAN0: u32 = 0;
pub const SYS_PROTECTCHAN1: u32 = 1;
pub const SYS_PROTECTCHAN2: u32 = 2;
pub const SYS_PROTECTCHAN3: u32 = 3;
pub const SYS_PROTECTCHANMAX: u32 = 4;

pub const SYS_PROTECTNONE: u32 = 0x0000_0000;
pub const SYS_PROTECTREAD: u32 = 0x0000_0001;
pub const SYS_PROTECTWRITE: u32 = 0x0000_0002;
pub const SYS_PROTECTRDWR: u32 = SYS_PROTECTREAD | SYS_PROTECTWRITE;

pub const SYS_LANG_ENGLISH: u32 = 0;
pub const SYS_LANG_GERMAN: u32 = 1;
pub const SYS_LANG_FRENCH: u32 = 2;
pub const SYS_LANG_SPANISH: u32 = 3;
pub const SYS_LANG_ITALIAN: u32 = 4;
pub const SYS_LANG_DUTCH: u32 = 5;

pub const SYS_VIDEO_NTSC: u32 = 0;
pub const SYS_VIDEO_PAL: u32 = 1;
pub const SYS_VIDEO_MPAL: u32 = 2;

pub const SYS_SOUND_MONO: u32 = 0;
pub const SYS_SOUND_STEREO: u32 = 1;

pub const SYS_FONTSIZE_ANSI: u32 = 288 + 131_072;
pub const SYS_FONTSIZE_SJIS: u32 = 3_840 + 1_179_648;

pub type SysWd = u32;
pub type AlarmCallback = Option<unsafe extern "C" fn(alarm: SysWd, cb_arg: *mut c_void)>;
pub type ResetCallback = Option<unsafe extern "C" fn(irq: u32, ctx: *mut c_void)>;
pub type PowerCallback = Option<unsafe extern "C" fn()>;
pub type ResetFunction = Option<unsafe extern "C" fn(final_: i32) -> i32>;
pub type DietPrintFn = Option<unsafe extern "C" fn(buf: *const c_char, size: usize)>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SystemArgv {
    pub argv_magic: i32,
    pub command_line: *mut c_char,
    pub length: i32,
    pub argc: i32,
    pub argv: *mut *mut c_char,
    pub end_argv: *mut *mut c_char,
}

static SYS_STDIO_REPORT_USE_STDOUT: AtomicBool = AtomicBool::new(false);
static SYS_COUNTER_BIAS: AtomicU32 = AtomicU32::new(0);
static SYS_COUNTER_BIAS_GET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_COUNTER_BIAS_SET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_DISPLAY_OFFSET_H: AtomicU32 = AtomicU32::new(0);
static SYS_DISPLAY_OFFSET_H_GET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_DISPLAY_OFFSET_H_SET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_EU_RGB60: AtomicU32 = AtomicU32::new(0);
static SYS_EU_RGB60_GET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_EU_RGB60_SET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_LANGUAGE: AtomicU32 = AtomicU32::new(0);
static SYS_LANGUAGE_GET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_LANGUAGE_SET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_PROGRESSIVE_SCAN: AtomicU32 = AtomicU32::new(0);
static SYS_PROGRESSIVE_SCAN_GET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_PROGRESSIVE_SCAN_SET_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_SOUND_MODE: AtomicU32 = AtomicU32::new(0);
static SYS_VIDEO_MODE: AtomicU32 = AtomicU32::new(0);
static SYS_WIRELESS_ID0: AtomicU32 = AtomicU32::new(0);
static SYS_WIRELESS_ID1: AtomicU32 = AtomicU32::new(0);
static SYS_WIRELESS_ID2: AtomicU32 = AtomicU32::new(0);
static SYS_WIRELESS_ID3: AtomicU32 = AtomicU32::new(0);
static SYS_GBS_MODE: AtomicU32 = AtomicU32::new(0);
static SYS_RESET_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static SYS_POWER_CALLBACK: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
static SYS_STM_EVENT_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static SYS_POWER_ENABLE_TIME: AtomicU32 = AtomicU32::new(0);
static SYS_POWER_DISPATCH_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_POWER_DISPATCH_RESULT: AtomicBool = AtomicBool::new(false);
static SYS_ARENA1_LO: AtomicUsize = AtomicUsize::new(0);
static SYS_ARENA1_HI: AtomicUsize = AtomicUsize::new(0);
static SYS_ARENA2_LO: AtomicUsize = AtomicUsize::new(0);
static SYS_ARENA2_HI: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "powerpc")]
static SYS_LWP_WORKSPACE: ArenaAllocator = ArenaAllocator::new();
const LWP_WORKSPACE_SIZE: usize = 512 * 1024;

#[cfg(target_arch = "powerpc")]
/// The region reserved for scheduler objects and dynamic worker stacks.
pub fn lwp_workspace_region() -> Option<core::ops::Range<usize>> {
    SYS_LWP_WORKSPACE.region()
}
static SYS_RESET_SYSTEM_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RESET_SYSTEM_RESET: AtomicU32 = AtomicU32::new(0);
static SYS_RESET_SYSTEM_CODE: AtomicU32 = AtomicU32::new(0);
static SYS_RESET_SYSTEM_FORCE_MENU: AtomicBool = AtomicBool::new(false);
static SYS_RESET_SYSTEM_TERMINAL_HINT: AtomicU32 = AtomicU32::new(0);
static SYS_RESET_REGISTER_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RESET_UNREGISTER_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RESET_REGISTER_PTR: AtomicUsize = AtomicUsize::new(0);
static SYS_RESET_UNREGISTER_PTR: AtomicUsize = AtomicUsize::new(0);
static SYS_RESET_QUEUE_HEAD: AtomicUsize = AtomicUsize::new(0);
static SYS_RESET_QUEUE_LOCK: AtomicBool = AtomicBool::new(false);
static SYS_RESET_DEBUG_HOOK: AtomicUsize = AtomicUsize::new(0);
static SYS_RUNTIME_INIT_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_INIT_STARTED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_INIT_COMPLETED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_IS_WII: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_INIT_STAGE: AtomicU32 = AtomicU32::new(0);
static SYS_RUNTIME_INIT_COMPLETED_MASK: AtomicU32 = AtomicU32::new(0);
static SYS_RUNTIME_FP_DISABLED_STAGE: AtomicU32 = AtomicU32::new(0);
static SYS_RUNTIME_INIT_FAILED_STAGE: AtomicU32 = AtomicU32::new(0);
static SYS_RUNTIME_PRE_MAIN_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_PRE_MAIN_COMPLETED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_PRE_MAIN_FAILED_STAGE: AtomicU32 = AtomicU32::new(0);
static SYS_RUNTIME_PRE_MAIN_RESULT: AtomicI32 = AtomicI32::new(0);
static SYS_PRE_MAIN_PROGRESS_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static SYS_RUNTIME_MAIN_LOOP_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_MAIN_LOOP_RESULT: AtomicBool = AtomicBool::new(false);
static SYS_RUNTIME_MAIN_LOOP_DONE: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static SYS_RUNTIME_POWER_STATE: AtomicU32 = AtomicU32::new(0);
static SYS_STARTUP_LOW_MEM_INIT_CALLED: AtomicBool = AtomicBool::new(false);
#[cfg(not(target_arch = "powerpc"))]
static SYS_TIME_VALUE: AtomicU64 = AtomicU64::new(0);
static SYS_TIME_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_FRAMEBUFFER_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_FRAMEBUFFER_RMODE: AtomicUsize = AtomicUsize::new(0);
static SYS_FRAMEBUFFER_BYTES: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
const FRAMEBUFFER_ALIGNMENT: usize = 32;
static SYS_RESET_BUTTON_DOWN_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_HOLLYWOOD_REVISION_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_PROTECT_RANGE_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_PROTECT_RANGE_CHAN: AtomicU32 = AtomicU32::new(0);
static SYS_PROTECT_RANGE_ADDR: AtomicUsize = AtomicUsize::new(0);
static SYS_PROTECT_RANGE_BYTES: AtomicU32 = AtomicU32::new(0);
static SYS_PROTECT_RANGE_CNTRL: AtomicU32 = AtomicU32::new(0);
static SYS_PMC_STARTED: AtomicBool = AtomicBool::new(false);
static SYS_PMC_DUMPED: AtomicBool = AtomicBool::new(false);
static SYS_PMC_STOPPED: AtomicBool = AtomicBool::new(false);
static SYS_PMC_RESET: AtomicBool = AtomicBool::new(false);
static SYS_PMC_MCR0VAL: AtomicU32 = AtomicU32::new(0);
static SYS_PMC_MCR1VAL: AtomicU32 = AtomicU32::new(0);
static SYS_PMC1: AtomicU32 = AtomicU32::new(0);
static SYS_PMC2: AtomicU32 = AtomicU32::new(0);
static SYS_PMC3: AtomicU32 = AtomicU32::new(0);
static SYS_PMC4: AtomicU32 = AtomicU32::new(0);
static SYS_FONT_ENCODING_QUERIED: AtomicBool = AtomicBool::new(false);
static SYS_FONT_ENCODING: AtomicU32 = AtomicU32::new(u32::MAX);
const VI_REG_BASE: usize = 0xCC00_2000;
const VI_FONT_ENCODING_FLAGS_REGISTER: usize = 55;
const VI_DTV_FLAG: u16 = 0x0002;
static SYS_FONT_INIT_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_FONT_INIT_PTR: AtomicUsize = AtomicUsize::new(0);
static SYS_FONT_TEXTURE_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_FONT_TEXTURE_C: AtomicU32 = AtomicU32::new(0);
static SYS_FONT_TEXEL_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_FONT_TEXEL_C: AtomicU32 = AtomicU32::new(0);
static SYS_FONT_DECODED_LEN: AtomicUsize = AtomicUsize::new(0);
static SYS_FONT_LOADED: AtomicBool = AtomicBool::new(false);
static SYS_FONT_DATA_PTR: AtomicUsize = AtomicUsize::new(0);
static SYS_FONT_IMAGE_PTR: AtomicUsize = AtomicUsize::new(0);

pub type SystemResetDebugHook = Option<unsafe fn(stage: u32, priority: u32, result: i32)>;

pub const SYS_RESET_TRACE_REGISTER: u32 = 1;
pub const SYS_RESET_TRACE_UNREGISTER: u32 = 2;
pub const SYS_RESET_TRACE_CALLBACK_BEGIN: u32 = 3;
pub const SYS_RESET_TRACE_CALLBACK_END: u32 = 4;
pub const SYS_RESET_TRACE_SRAM: u32 = 5;
pub const SYS_RESET_TRACE_TERMINAL: u32 = 6;
/// DSP shutdown stage and control-register value before reset callbacks.
pub const SYS_RESET_TRACE_DSP: u32 = 7;
/// Return-to-System-Menu launch started or returned with an error code.
pub const SYS_RESET_TRACE_RETURN_TO_MENU_BEGIN: u32 = 8;
pub const SYS_RESET_TRACE_RETURN_TO_MENU_RESULT: u32 = 9;
/// IOS subsystem shutdown started or returned with an error code.
pub const SYS_RESET_TRACE_IOS_SHUTDOWN_BEGIN: u32 = 10;
pub const SYS_RESET_TRACE_IOS_SHUTDOWN_RESULT: u32 = 11;
/// DSP shutdown mailbox word before its busy-bit wait.
pub const SYS_RESET_TRACE_DSP_MAILBOX: u32 = 12;
pub const SYS_RESET_TRACE_EXIT_BEGIN: u32 = 13;
pub const SYS_RESET_TRACE_EXIT_FINISH: u32 = 14;
/// priority is the selected reset type; result is 1 for the HBC stub route.
pub const SYS_RESET_TRACE_EXIT_ROUTE: u32 = 15;
pub const SYS_RESET_TRACE_INTERRUPTS_BEGIN: u32 = 16;
pub const SYS_RESET_TRACE_INTERRUPTS_END: u32 = 17;
pub const SYS_RESET_TRACE_FINAL_CALLBACKS_BEGIN: u32 = 18;
pub const SYS_RESET_TRACE_FINAL_CALLBACKS_END: u32 = 19;
pub const SYS_RESET_TRACE_CACHE_BEGIN: u32 = 20;
pub const SYS_RESET_TRACE_CACHE_END: u32 = 21;
pub const SYS_RESET_TRACE_LOWMEM_BEGIN: u32 = 22;
pub const SYS_RESET_TRACE_LOWMEM_END: u32 = 23;
pub const SYS_RESET_TRACE_RESET_RETURN: u32 = 24;
/// Last event before the interrupt mask and branch to 0x80001800.
pub const SYS_RESET_TRACE_RELOAD_STUB: u32 = 25;
pub const SYS_RESET_TRACE_RESET_BEGIN: u32 = 26;
pub const SYS_RESET_TRACE_CALLBACKS_BEGIN: u32 = 27;
pub const SYS_RESET_TRACE_CALLBACKS_END: u32 = 28;

struct ResetQueueGuard;

impl ResetQueueGuard {
    fn acquire() -> Self {
        while SYS_RESET_QUEUE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ResetQueueGuard {
    fn drop(&mut self) {
        SYS_RESET_QUEUE_LOCK.store(false, Ordering::Release);
    }
}

fn reset_debug(stage: u32, priority: u32, result: i32) {
    #[cfg(ogc_dev)]
    dev::record_reset(stage, priority, result);
    let hook = SYS_RESET_DEBUG_HOOK.load(Ordering::Acquire);
    if hook != 0 {
        let hook = unsafe { core::mem::transmute::<usize, unsafe fn(u32, u32, i32)>(hook) };
        unsafe { hook(stage, priority, result) };
    }
}

pub fn set_reset_debug_hook(hook: SystemResetDebugHook) -> SystemResetDebugHook {
    let previous = SYS_RESET_DEBUG_HOOK.swap(
        hook.map_or(0, |callback| callback as usize),
        Ordering::AcqRel,
    );
    if previous == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe fn(u32, u32, i32)>(previous) })
    }
}
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const STM_IMMEDIATE_PATH: &CStr = c"/dev/stm/immediate";
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const STM_EVENTHOOK_PATH: &CStr = c"/dev/stm/eventhook";
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const IOCTL_STM_HOTRESET: u32 = 0x2001;
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const IOCTL_STM_SHUTDOWN: u32 = 0x2003;
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const IOCTL_STM_IDLE: u32 = 0x2004;
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const IOCTL_STM_LEDMODE: u32 = 0x6002;

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn open_stm_immediate_with<T>(open: impl FnOnce(&CStr, u32) -> Result<T, i32>) -> Result<T, i32> {
    open(STM_IMMEDIATE_PATH, IPC_OPEN_NONE)
}
#[cfg(target_arch = "powerpc")]
const WPAD_RESET_PRIORITY: u32 = 127;
#[cfg(target_arch = "powerpc")]
const RESET_LOWMEM_REGIONS: [(usize, usize); 6] = [
    (0x8000_0040, 140),
    (0x8000_00d4, 20),
    (0x8000_00f4, 4),
    (0x8000_3000, 192),
    (0x8000_30c8, 12),
    (0x8000_30e2, 1),
];
#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
const IRQ_PI_RSW: u32 = 7;
#[cfg(target_arch = "powerpc")]
const POWER_CALLBACK_COOLDOWN_MS: u32 = 50;
#[cfg(target_arch = "powerpc")]
const TB_TICKS_PER_MILLISECOND: u32 = 60_750;
#[cfg(target_arch = "powerpc")]
const BOOT_TIME_ADDRESS: usize = 0x8000_30d8;
#[cfg(target_arch = "powerpc")]
const GAMECUBE_TICKS_PER_SECOND: u64 = 40_500_000;

#[cfg(target_arch = "powerpc")]
fn sys_millisecs_to_ticks(ms: u32) -> u32 {
    ms.saturating_mul(TB_TICKS_PER_MILLISECOND)
}

#[cfg(target_arch = "powerpc")]
struct ExiRtc {
    command: u32,
}
#[cfg(target_arch = "powerpc")]
impl rtc::RtcTransport for ExiRtc {
    fn lock(&mut self) -> bool {
        unsafe { exi::lock(EXI_CHANNEL_0, EXI_DEVICE_1, None) != 0 }
    }
    fn select(&mut self) -> bool {
        exi::select(EXI_CHANNEL_0, EXI_DEVICE_1, EXI_SPEED8MHZ) != 0
    }
    fn command(&mut self) -> bool {
        unsafe {
            exi::imm(
                EXI_CHANNEL_0,
                core::ptr::addr_of_mut!(self.command).cast(),
                4,
                EXI_WRITE as i32,
                None,
            ) != 0
        }
    }
    fn sync(&mut self) -> bool {
        exi::sync(EXI_CHANNEL_0) != 0
    }
    fn read(&mut self, value: &mut u32) -> bool {
        unsafe {
            exi::imm(
                EXI_CHANNEL_0,
                (value as *mut u32).cast(),
                4,
                EXI_READ as i32,
                None,
            ) != 0
        }
    }
    fn deselect(&mut self) -> bool {
        exi::deselect(EXI_CHANNEL_0) != 0
    }
    fn unlock(&mut self) -> bool {
        exi::unlock(EXI_CHANNEL_0) != 0
    }
}
#[cfg(target_arch = "powerpc")]
fn read_rtc() -> Result<u32, ClockError> {
    rtc::read(&mut ExiRtc {
        command: 0x2000_0000,
    })
}

// A bounded interrupt mask protects only in-memory state. EXI operations run
// outside this mask and publish the completed read afterward.
struct ClockCell(core::cell::UnsafeCell<clock::ClockState>);
// SAFETY: All access goes through with_clock on the single PowerPC CPU.
unsafe impl Sync for ClockCell {}
static CLOCK: ClockCell = ClockCell(core::cell::UnsafeCell::new(clock::ClockState::new(
    ogc_lwp_driver::TICKS_PER_SECOND,
)));
static CLOCK_SYNC_BUSY: AtomicBool = AtomicBool::new(false);

#[cfg(target_arch = "powerpc")]
fn with_clock<T>(operation: impl FnOnce(&mut clock::ClockState) -> T) -> T {
    let irq = ogc_lwp_driver::critical_enter();
    let result = operation(unsafe { &mut *CLOCK.0.get() });
    ogc_lwp_driver::critical_exit(irq);
    result
}

#[cfg(target_arch = "powerpc")]
fn clock_context() -> Result<(), ClockError> {
    if !SYS_RUNTIME_INIT_COMPLETED.load(Ordering::Acquire) {
        return Err(ClockError::NotInitialized);
    }
    if unsafe { sys_get_hollywood_revision() } == 0 {
        return Err(ClockError::UnsupportedHardware);
    }
    Ok(())
}

/// Read elapsed monotonic time since checked system initialization. A backward
/// hardware timebase change permanently invalidates this application clock.
pub fn sys_monotonic_time() -> Result<core::time::Duration, ClockError> {
    clock_context()?;
    with_clock(|state| state.monotonic(read_timebase()))
}

/// Read UTC wall time. A real RTC sample, counter bias, and an explicit UTC
/// offset are required. No boot-memory offset is treated as a UTC timestamp.
pub fn sys_wall_time() -> Result<core::time::Duration, ClockError> {
    clock_context()?;
    with_clock(|state| state.wall(read_timebase()))
}

/// Supply seconds east of UTC for the Wii local calendar. This changes only
/// application memory. It does not change SYSCONF, SRAM, RTC, or the timebase.
pub fn sys_set_clock_utc_offset(seconds: i32) -> Result<(), ClockError> {
    clock_context()?;
    with_clock(|state| state.set_utc_offset(seconds))
}

pub fn sys_clock_snapshot() -> ClockSnapshot {
    with_clock(|state| state.snapshot())
}

/// Refresh from the real RTC and already initialized console configuration.
/// Preserve operation and cleanup failures, and do not rewrite the timebase.
pub fn sys_synchronize_clock() -> Result<(), ClockError> {
    clock_context()?;
    if let Err(error) = ogc_lwp_driver::current_thread_checked() {
        return Err(match error {
            ogc_lwp_driver::ThreadError::ExceptionContext => ClockError::ExceptionContext,
            _ => ClockError::InterruptsDisabled,
        });
    }
    if CLOCK_SYNC_BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(ClockError::Busy);
    }
    let sample = rtc::consistent(read_rtc).and_then(|seconds| {
        let mut bias = 0;
        let result = conf::get_counter_bias(&mut bias);
        if result < 0 {
            Err(ClockError::CounterBias(result))
        } else {
            Ok((seconds, bias))
        }
    });
    let result = with_clock(|state| state.synchronize(read_timebase(), sample));
    CLOCK_SYNC_BUSY.store(false, Ordering::Release);
    result
}

#[cfg(target_arch = "powerpc")]
fn set_timebase(value: u64) {
    let upper = (value >> 32) as u32;
    let lower = value as u32;
    unsafe {
        asm!(
            "li {zero}, 0",
            "mttbl {zero}",
            "mttbu {upper}",
            "mttbl {lower}",
            "isync",
            zero = out(reg) _,
            upper = in(reg) upper,
            lower = in(reg) lower,
            options(nostack)
        );
    }
}

#[cfg(target_arch = "powerpc")]
fn set_system_time(value: u64) {
    let saved_msr: u32;
    unsafe {
        asm!("mfmsr {msr}", msr = out(reg) saved_msr, options(nostack));
        asm!(
            "mtmsr {msr}",
            "isync",
            msr = in(reg) saved_msr & !0x8000,
            options(nostack)
        );

        let boot_time = core::ptr::read_volatile(BOOT_TIME_ADDRESS as *const u64);
        let adjusted_boot_time = read_timebase().wrapping_sub(value).wrapping_add(boot_time);
        core::ptr::write_volatile(BOOT_TIME_ADDRESS as *mut u64, adjusted_boot_time);
        set_timebase(value);
        exi::probe_reset();

        asm!(
            "mtmsr {msr}",
            "isync",
            msr = in(reg) saved_msr,
            options(nostack)
        );
    }
}

#[cfg(target_arch = "powerpc")]
fn set_boot_time() {
    let is_wii = unsafe { sys_get_hollywood_revision() } != 0;
    if is_wii {
        // The result remains observable through sys_clock_snapshot and the
        // fallible clock reads. Failed wall time does not invalidate Instant.
        match sys_synchronize_clock() {
            Ok(()) => {}
            Err(error) => {
                with_clock(|state| state.record_wall_failure(error));
            }
        }
        return;
    }
    // Keep the existing GameCube compatibility path. Wii std never selects it.
    let Ok(seconds) = rtc::consistent(read_rtc) else {
        return;
    };
    let bias = unsafe { sys_get_counter_bias() };
    let adjusted_seconds = u64::from(seconds.wrapping_add(bias));
    set_system_time(adjusted_seconds * GAMECUBE_TICKS_PER_SECOND);
}

#[cfg(target_arch = "powerpc")]
fn system_time_value(timebase: u64, boot_time: u64) -> u64 {
    timebase.wrapping_add(boot_time)
}

#[cfg(target_arch = "powerpc")]
fn read_system_time() -> u64 {
    let saved_msr: u32;
    unsafe {
        asm!("mfmsr {msr}", msr = out(reg) saved_msr, options(nostack));
        asm!(
            "mtmsr {msr}",
            "isync",
            msr = in(reg) saved_msr & !0x8000,
            options(nostack)
        );

        let timebase = read_timebase();
        let boot_time = core::ptr::read_volatile(BOOT_TIME_ADDRESS as *const u64);
        let value = system_time_value(timebase, boot_time);

        asm!(
            "mtmsr {msr}",
            "isync",
            msr = in(reg) saved_msr,
            options(nostack)
        );
        value
    }
}

#[cfg(target_arch = "powerpc")]
fn power_callback_due(now: u32, enable_time: u32) -> bool {
    now.wrapping_sub(enable_time) < 0x8000_0000
}

#[cfg(target_arch = "powerpc")]
fn read_timebase() -> u64 {
    loop {
        let upper1: u32;
        let lower: u32;
        let upper2: u32;
        unsafe {
            asm!(
                "mftbu {upper1}",
                "mftb {lower}",
                "mftbu {upper2}",
                upper1 = out(reg) upper1,
                lower = out(reg) lower,
                upper2 = out(reg) upper2,
                options(nostack, preserves_flags)
            );
        }
        if upper1 == upper2 {
            return ((upper1 as u64) << 32) | lower as u64;
        }
    }
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
unsafe fn sys_stm_event_handler(event: u32) {
    unsafe {
        match event {
            0x0002_0000 => {
                if sys_reset_button_down() != 0 {
                    let _ = SYS_RUNTIME_POWER_STATE.compare_exchange(
                        0,
                        1,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    let ptr = SYS_RESET_CALLBACK.load(Ordering::Acquire);
                    if ptr != 0 {
                        let cb = {
                            core::mem::transmute::<usize, unsafe extern "C" fn(u32, *mut c_void)>(
                                ptr,
                            )
                        };
                        {
                            cb(IRQ_PI_RSW, core::ptr::null_mut());
                        }
                    }
                }
            }
            0x0000_0800 => {
                let _ = SYS_RUNTIME_POWER_STATE.compare_exchange(
                    0,
                    2,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                sys_do_power_cb();
            }
            _ => {}
        }
    }
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_register_event_handler_local(handler: Option<unsafe fn(u32)>) -> Option<unsafe fn(u32)> {
    let old = SYS_STM_EVENT_CALLBACK.swap(handler.map_or(0, |cb| cb as usize), Ordering::AcqRel);
    if old == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe fn(u32)>(old) })
    }
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_init_local() -> i32 {
    let imm = match open_stm_immediate_with(open_file) {
        Ok(fd) => fd,
        Err(err) => return err,
    };
    let hook = match open_file(STM_EVENTHOOK_PATH, IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(err) => {
            return match imm.close() {
                Ok(()) => err,
                Err(close_err) => close_err,
            };
        }
    };
    let mut result = 1;
    if let Err(err) = imm.close() {
        result = err;
    }
    if let Err(err) = hook.close()
        && result > 0
    {
        result = err;
    }
    result
}
static SYS_ALARM_LAST_ID: AtomicU32 = AtomicU32::new(0);
static SYS_ALARM_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static SYS_ALARM_PERIODIC_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static SYS_ALARM_CREATE_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_ALARM_REMOVE_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_ALARM_CANCEL_CALLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static SYS_SRAM_BUSY: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static SYS_SRAM_WRITE_OFFSET: AtomicUsize = AtomicUsize::new(64);
#[cfg(target_arch = "powerpc")]
static SYS_SRAM_INTERRUPT_STATE: AtomicU32 = AtomicU32::new(0);
static SYS_SRAM_LOCKED: AtomicBool = AtomicBool::new(false);
static SYS_SRAM_EX_LOCKED: AtomicBool = AtomicBool::new(false);
static SYS_SRAM_SYNC_CALLED: AtomicBool = AtomicBool::new(false);
static SYS_SRAM_SYNC_RESULT: AtomicBool = AtomicBool::new(false);
static mut SYS_SRAM: SysSram = SysSram {
    checksum: 0,
    checksum_inv: 0,
    ead0: 0,
    ead1: 0,
    counter_bias: 0,
    display_offset_h: 0,
    ntd: 0,
    lang: 0,
    flags: 0,
};
static mut SYS_SRAM_EX: SysSramEx = SysSramEx {
    flash_id: [[0; 12]; 2],
    wireless_kbd_id: 0,
    wireless_pad_id: [0; 4],
    dvderr_code: 0,
    padding0: 0,
    flash_id_chksum: [0; 2],
    gbs: 0,
    padding1: 0,
};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct SysSram {
    pub checksum: u16,
    pub checksum_inv: u16,
    pub ead0: u32,
    pub ead1: u32,
    pub counter_bias: u32,
    pub display_offset_h: i8,
    pub ntd: u8,
    pub lang: u8,
    pub flags: u8,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct SysSramEx {
    pub flash_id: [[u8; 12]; 2],
    pub wireless_kbd_id: u32,
    pub wireless_pad_id: [u16; 4],
    pub dvderr_code: u8,
    pub padding0: u8,
    pub flash_id_chksum: [u8; 2],
    pub gbs: u16,
    pub padding1: u16,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct SysFontHeader {
    pub font_type: u16,
    pub first_char: u16,
    pub last_char: u16,
    pub inval_char: u16,
    pub asc: u16,
    pub desc: u16,
    pub width: u16,
    pub leading: u16,
    pub cell_width: u16,
    pub cell_height: u16,
    pub sheet_size: u32,
    pub sheet_format: u16,
    pub sheet_column: u16,
    pub sheet_row: u16,
    pub sheet_width: u16,
    pub sheet_height: u16,
    pub width_table: u16,
    pub sheet_image: u32,
    pub sheet_fullsize: u32,
    pub c0: u8,
    pub c1: u8,
    pub c2: u8,
    pub c3: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LwpNode {
    pub next: *mut LwpNode,
    pub prev: *mut LwpNode,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SysResetInfo {
    pub node: LwpNode,
    pub func: ResetFunction,
    pub prio: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i32,
}

pub fn sys_stdio_report(use_stdout: Bool) {
    SYS_STDIO_REPORT_USE_STDOUT.store(use_stdout != 0, Ordering::Release);
}

#[cfg(not(target_arch = "powerpc"))]
pub static mut DIET_PRINT_FN: DietPrintFn = None;

#[cfg(not(target_arch = "powerpc"))]
pub static mut SYSTEM_ARGV: *mut SystemArgv = core::ptr::null_mut();

#[cfg(not(target_arch = "powerpc"))]
pub static mut ARGV_ARENA1_LO: *mut u8 = 0xdead_beefusize as *mut u8;

#[cfg(target_arch = "powerpc")]
unsafe extern "C" {
    static __bss_end: u8;
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" {
    static __system_argv: u8;
    static mut __argvArena1Lo: u8;
}

#[cfg(target_arch = "powerpc")]
#[inline]
/// Returns the runtime-owned process argument descriptor.
///
/// # Safety
/// The caller must synchronize access to the runtime argument globals. The returned pointer may
/// be null; when non-null, it may only be dereferenced while the runtime keeps the descriptor and
/// all storage referenced by it valid.
pub unsafe fn current_system_argv() -> *mut SystemArgv {
    unsafe { read_unaligned(core::ptr::addr_of!(__system_argv).cast::<*mut SystemArgv>()) }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline]
/// Returns the compatibility process argument descriptor.
///
/// # Safety
/// The caller must synchronize access to `SYSTEM_ARGV`. The returned pointer may be null and must
/// only be dereferenced while its caller-provided allocation remains valid.
pub unsafe fn current_system_argv() -> *mut SystemArgv {
    SYSTEM_ARGV
}

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn set_argv_arena1_lo(value: *mut u8) {
    unsafe {
        write_unaligned(
            core::ptr::addr_of_mut!(__argvArena1Lo).cast::<*mut u8>(),
            value,
        );
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline]
unsafe fn set_argv_arena1_lo(value: *mut u8) {
    ARGV_ARENA1_LO = value;
}

#[cfg(target_arch = "powerpc")]
unsafe fn write_report_uart(buffer: *const u8, length: usize) {
    if buffer.is_null() || length == 0 {
        return;
    }
    let devkit_register: usize = if unsafe { sys_get_hollywood_revision() } != 0 {
        0xCD00_6024
    } else {
        0xCC00_6024
    };
    let devkit_id = unsafe { read_volatile(devkit_register as *const u32) } & 0xff;
    if (devkit_register == 0xCC00_6024 && devkit_id == 0xff)
        || (devkit_register == 0xCD00_6024 && devkit_id != 1)
    {
        return;
    }
    if unsafe { exi::lock(EXI_CHANNEL_0, EXI_DEVICE_1, None) } == 0 {
        return;
    }
    if exi::select(EXI_CHANNEL_0, EXI_DEVICE_1, EXI_SPEED8MHZ) == 0 {
        let _ = exi::unlock(EXI_CHANNEL_0);
        return;
    }

    let mut command = 0xA001_0000u32;
    let _ = unsafe {
        exi::imm(
            EXI_CHANNEL_0,
            core::ptr::addr_of_mut!(command).cast::<c_void>(),
            4,
            EXI_WRITE as i32,
            None,
        )
    };
    let _ = exi::sync(EXI_CHANNEL_0);

    let mut output = [0u8; 256];
    let mut output_len = 0usize;
    let mut pending_carriage_return = false;
    for index in 0..length {
        let mut byte = unsafe { *buffer.add(index) };
        if pending_carriage_return {
            output[output_len] = b'\r';
            output_len += 1;
            pending_carriage_return = false;
            if output_len == output.len() {
                unsafe { write_report_chunk(&mut output, output_len) };
                output_len = 0;
            }
            if byte == b'\n' {
                continue;
            }
        }
        if byte == b'\r' {
            pending_carriage_return = true;
            continue;
        }
        if byte == b'\n' {
            byte = b'\r';
        }
        output[output_len] = byte;
        output_len += 1;
        if output_len == output.len() {
            unsafe { write_report_chunk(&mut output, output_len) };
            output_len = 0;
        }
    }
    if pending_carriage_return {
        output[output_len] = b'\r';
        output_len += 1;
    }
    if output_len != 0 {
        unsafe { write_report_chunk(&mut output, output_len) };
    }
    let _ = exi::deselect(EXI_CHANNEL_0);
    let _ = exi::unlock(EXI_CHANNEL_0);
}

#[cfg(target_arch = "powerpc")]
unsafe fn write_report_chunk(buffer: &mut [u8], length: usize) {
    let _ = unsafe {
        exi::imm_ex(
            EXI_CHANNEL_0,
            buffer.as_mut_ptr().cast::<c_void>(),
            length as u32,
            EXI_WRITE as i32,
        )
    };
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn default_diet_print(buf: *const c_char, size: usize) {
    unsafe { write_report_uart(buf.cast::<u8>(), size) };
}

#[cfg(target_arch = "powerpc")]
pub static mut DIET_PRINT_FN: DietPrintFn = Some(default_diet_print);

pub fn sys_stdio_report_uses_stdout() -> bool {
    SYS_STDIO_REPORT_USE_STDOUT.load(Ordering::Acquire)
}

#[inline]
const fn wii_target_available() -> bool {
    cfg!(target_arch = "powerpc")
}

#[cfg(target_arch = "powerpc")]
const PI_REG_BASE: usize = 0xCC00_3000;

unsafe fn build_argv(argstruct: *mut SystemArgv) {
    unsafe {
        let argv = &mut *argstruct;
        if argv.command_line.is_null() || argv.length <= 0 {
            argv.argc = 0;
            argv.argv = null_mut();
            argv.end_argv = null_mut();
            return;
        }

        let len = argv.length as usize;
        let src = core::slice::from_raw_parts(argv.command_line.cast::<u8>(), len);
        let mut count = 0usize;
        let mut i = 0usize;
        while i < len {
            while i < len && src[i] == 0 {
                i += 1;
            }
            if i >= len {
                break;
            }
            count += 1;
            while i < len && src[i] != 0 {
                i += 1;
            }
        }

        let ptr_size = core::mem::size_of::<*mut c_char>();
        let base = argv.command_line as usize;
        let argv_start =
            (base + len + (align_of::<*mut c_char>() - 1)) & !(align_of::<*mut c_char>() - 1);
        let argv_ptr = argv_start as *mut *mut c_char;
        let argv_slice = core::slice::from_raw_parts_mut(argv_ptr, count + 1);

        let mut argc = 0usize;
        let mut pos = 0usize;
        while pos < len {
            while pos < len && src[pos] == 0 {
                pos += 1;
            }
            if pos >= len {
                break;
            }
            argv_slice[argc] = argv.command_line.cast::<u8>().add(pos).cast::<c_char>();
            argc += 1;
            while pos < len && src[pos] != 0 {
                pos += 1;
            }
        }
        argv_slice[argc] = null_mut();

        argv.argc = argc as i32;
        argv.argv = argv_ptr;
        argv.end_argv = argv_ptr.add(argc + 1);
        let _ = ptr_size;
    }
}

#[cfg(target_arch = "powerpc")]
fn seed_runtime_arenas(is_wii: bool) {
    let bss_end = (core::ptr::addr_of!(__bss_end) as usize)
        .checked_next_multiple_of(32)
        .expect("BSS end exceeds the address space");
    let argv_end = unsafe { read_unaligned(core::ptr::addr_of!(__argvArena1Lo).cast::<usize>()) };
    let arena1_lo = if (bss_end..0x8180_0000).contains(&argv_end) {
        argv_end
            .checked_next_multiple_of(32)
            .expect("validated arena address")
    } else {
        bss_end
    };
    SYS_ARENA1_LO.store(arena1_lo, Ordering::Release);
    SYS_ARENA1_HI.store(0x8180_0000, Ordering::Release);
    if is_wii {
        SYS_ARENA2_LO.store(0x9000_0000, Ordering::Release);
        SYS_ARENA2_HI.store(0x9400_0000, Ordering::Release);
    }
}

#[cfg(target_arch = "powerpc")]
fn initialize_low_memory(is_wii: bool) {
    seed_runtime_arenas(is_wii);
    if !is_wii {
        const MEM1_START: usize = 0x8000_0000;
        const MEM1_SIZE: u32 = 0x0180_0000;
        const ARENA1_LO: usize = 0x8000_3000;
        const PI_MEM_SIZE_REG: usize = 0xCC00_302C;
        unsafe {
            core::ptr::write_bytes(MEM1_START as *mut u8, 0, 0x100);
            write_volatile((MEM1_START + 0x20) as *mut u32, 0x0D15_EA5E);
            write_volatile((MEM1_START + 0x24) as *mut u32, 1);
            write_volatile((MEM1_START + 0x28) as *mut u32, MEM1_SIZE);
            let memory_size = 1 + (read_volatile(PI_MEM_SIZE_REG as *const u32) >> 28);
            write_volatile((MEM1_START + 0x2C) as *mut u32, memory_size);
            write_volatile(
                (MEM1_START + 0x30) as *mut u32,
                SYS_ARENA1_LO.load(Ordering::Acquire) as u32,
            );
            write_volatile(
                (MEM1_START + 0x34) as *mut u32,
                SYS_ARENA1_HI.load(Ordering::Acquire) as u32,
            );
            write_volatile((MEM1_START + 0xEC) as *mut u32, 0x8180_0000);
            write_volatile((MEM1_START + 0xF0) as *mut u32, MEM1_SIZE);
            write_volatile((MEM1_START + 0xF8) as *mut u32, 162_000_000);
            write_volatile((MEM1_START + 0xFC) as *mut u32, 486_000_000);
            write_volatile((ARENA1_LO + 0xE0) as *mut u16, 6);
            write_volatile((ARENA1_LO + 0xE4) as *mut u32, 0xC000_8000);
        }
        unsafe {
            cache::dc_flush_range(MEM1_START as *const c_void, 0x100);
            cache::dc_flush_range(ARENA1_LO as *const c_void, 0x100);
        }
    }
    unsafe { asm!("sync", options(nostack)) };
}

#[cfg(target_arch = "powerpc")]
const SRAM_BUFFER_BYTES: usize =
    core::mem::size_of::<SysSram>() + core::mem::size_of::<SysSramEx>();

#[inline]
fn pending_sram_write_offset(pending: usize, requested: usize) -> usize {
    pending.min(requested)
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct AlignedSramBuffer([u8; SRAM_BUFFER_BYTES]);

#[cfg(target_arch = "powerpc")]
unsafe fn sram_copy_out(buffer: *mut u8) {
    unsafe {
        copy(
            core::ptr::addr_of!(SYS_SRAM).cast::<u8>(),
            buffer,
            core::mem::size_of::<SysSram>(),
        );
        copy(
            core::ptr::addr_of!(SYS_SRAM_EX).cast::<u8>(),
            buffer.add(core::mem::size_of::<SysSram>()),
            core::mem::size_of::<SysSramEx>(),
        );
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_copy_in(buffer: *const u8) {
    unsafe {
        copy(
            buffer,
            core::ptr::addr_of_mut!(SYS_SRAM).cast::<u8>(),
            core::mem::size_of::<SysSram>(),
        );
        copy(
            buffer.add(core::mem::size_of::<SysSram>()),
            core::ptr::addr_of_mut!(SYS_SRAM_EX).cast::<u8>(),
            core::mem::size_of::<SysSramEx>(),
        );
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_rebuild_checksum(buffer: *mut u8) {
    unsafe {
        let words = core::array::from_fn(|i| {
            core::ptr::read_unaligned(
                buffer
                    .add((6 + i) * core::mem::size_of::<u16>())
                    .cast::<u16>(),
            )
        });
        let (checksum, checksum_inv) = sram_checksum_words(words);
        core::ptr::write_unaligned(buffer.cast::<u16>(), checksum);
        core::ptr::write_unaligned(buffer.add(2).cast::<u16>(), checksum_inv);
    }
}

/// Calculate the two SRAM checksums from the four data words in the SRAM layout.
pub fn sram_checksum_words(words: [u16; 4]) -> (u16, u16) {
    let checksum = words.iter().copied().fold(0_u16, u16::wrapping_add);
    let checksum_inv = words
        .iter()
        .copied()
        .fold(0_u16, |sum, word| sum.wrapping_add(!word));
    (checksum, checksum_inv)
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_sync_shadow() {
    unsafe {
        SYS_COUNTER_BIAS.store(SYS_SRAM.counter_bias, Ordering::Release);
        SYS_DISPLAY_OFFSET_H.store(SYS_SRAM.display_offset_h as u32, Ordering::Release);
        SYS_EU_RGB60.store(((SYS_SRAM.ntd >> 6) & 1) as u32, Ordering::Release);
        SYS_LANGUAGE.store(SYS_SRAM.lang as u32, Ordering::Release);
        SYS_PROGRESSIVE_SCAN.store(((SYS_SRAM.flags >> 7) & 1) as u32, Ordering::Release);
        SYS_SOUND_MODE.store(((SYS_SRAM.flags >> 2) & 1) as u32, Ordering::Release);
        SYS_VIDEO_MODE.store((SYS_SRAM.flags & 0x03) as u32, Ordering::Release);
        SYS_WIRELESS_ID0.store(
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[0])) as u32,
            Ordering::Release,
        );
        SYS_WIRELESS_ID1.store(
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[1])) as u32,
            Ordering::Release,
        );
        SYS_WIRELESS_ID2.store(
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[2])) as u32,
            Ordering::Release,
        );
        SYS_WIRELESS_ID3.store(
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[3])) as u32,
            Ordering::Release,
        );
        SYS_GBS_MODE.store(
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.gbs)) as u32,
            Ordering::Release,
        );
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_read_powerpc() -> bool {
    unsafe {
        let mut buffer = AlignedSramBuffer([0u8; SRAM_BUFFER_BYTES]);
        let buffer_ptr = buffer.0.as_mut_ptr();

        // The C implementation invalidates this aligned buffer before asking EXI
        // to DMA the SRAM contents into it.
        cache::dc_invalidate_range(buffer_ptr.cast::<c_void>(), SRAM_BUFFER_BYTES as u32);

        if exi::lock(EXI_CHANNEL_0, EXI_DEVICE_1, None) == 0 {
            return false;
        }
        if exi::select(EXI_CHANNEL_0, EXI_DEVICE_1, EXI_SPEED8MHZ) == 0 {
            let _ = exi::unlock(EXI_CHANNEL_0);
            return false;
        }

        let mut command = 0x2000_0100u32;
        let mut ok = true;
        if exi::imm(
            EXI_CHANNEL_0,
            core::ptr::addr_of_mut!(command).cast::<c_void>(),
            4,
            EXI_WRITE as i32,
            None,
        ) == 0
        {
            ok = false;
        }
        if ok && exi::sync(EXI_CHANNEL_0) == 0 {
            ok = false;
        }
        if ok && {
            exi::dma(
                EXI_CHANNEL_0,
                buffer_ptr.cast::<c_void>(),
                SRAM_BUFFER_BYTES as u32,
                EXI_READ as i32,
                None,
            )
        } == 0
        {
            ok = false;
        }
        if ok && exi::sync(EXI_CHANNEL_0) == 0 {
            ok = false;
        }
        if exi::deselect(EXI_CHANNEL_0) == 0 {
            ok = false;
        }
        if exi::unlock(EXI_CHANNEL_0) == 0 {
            ok = false;
        }

        if ok {
            sram_copy_in(buffer_ptr);
            sram_sync_shadow();
        }
        SYS_SRAM_SYNC_RESULT.store(ok, Ordering::Release);
        ok
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_write_powerpc(requested_offset: usize, sanitize: bool) -> bool {
    unsafe {
        let mut buffer = [0u8; SRAM_BUFFER_BYTES];
        sram_copy_out(buffer.as_mut_ptr());
        if sanitize && (buffer[19] & 0x03) > 0x02 {
            buffer[19] &= !0x03;
        }
        if sanitize {
            sram_rebuild_checksum(buffer.as_mut_ptr());
            sram_copy_in(buffer.as_ptr());
        }
        let pending = SYS_SRAM_WRITE_OFFSET.fetch_min(requested_offset, Ordering::AcqRel);
        let offset = pending_sram_write_offset(pending, requested_offset);

        if exi::lock(EXI_CHANNEL_0, EXI_DEVICE_1, None) == 0 {
            return false;
        }
        if exi::select(EXI_CHANNEL_0, EXI_DEVICE_1, EXI_SPEED8MHZ) == 0 {
            let _ = exi::unlock(EXI_CHANNEL_0);
            return false;
        }

        let mut command = 0xa000_0100u32 + ((offset as u32) << 6);
        let mut ok = true;
        if exi::imm(
            EXI_CHANNEL_0,
            core::ptr::addr_of_mut!(command).cast::<c_void>(),
            4,
            EXI_WRITE as i32,
            None,
        ) == 0
        {
            ok = false;
        }
        if ok && exi::sync(EXI_CHANNEL_0) == 0 {
            ok = false;
        }
        if ok
            && exi::imm_ex(
                EXI_CHANNEL_0,
                buffer.as_mut_ptr().add(offset).cast::<c_void>(),
                (SRAM_BUFFER_BYTES - offset) as u32,
                EXI_WRITE as i32,
            ) == 0
        {
            ok = false;
        }
        if exi::deselect(EXI_CHANNEL_0) == 0 {
            ok = false;
        }
        if exi::unlock(EXI_CHANNEL_0) == 0 {
            ok = false;
        }

        if ok {
            SYS_SRAM_WRITE_OFFSET.store(SRAM_BUFFER_BYTES, Ordering::Release);
            sram_sync_shadow();
        }
        SYS_SRAM_SYNC_RESULT.store(ok, Ordering::Release);
        ok
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_sanitize_and_write_powerpc() -> bool {
    unsafe { sram_write_powerpc(0, true) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_write_extended_powerpc() -> bool {
    unsafe { sram_write_powerpc(size_of::<SysSram>(), false) }
}

pub fn _check_argv() {
    unsafe {
        let Some(argv) = current_system_argv().as_mut() else {
            return;
        };

        if argv.argv_magic != 0x5f61_7267 {
            argv.argc = 0;
            argv.argv = null_mut();
            return;
        }

        // The runtime entry has already sheltered and parsed HBC arguments.
        // Repeating the move here would copy them over the arena allocation.
        if argv.argc > 0 && !argv.argv.is_null() && !argv.end_argv.is_null() {
            return;
        }

        let arena_lo = sys_get_arena1_lo().cast::<u8>();
        if arena_lo.is_null() {
            return;
        }

        let dest = ((arena_lo as usize + 3) & !3) as *mut u8;
        if argv.length > 0 {
            copy(argv.command_line.cast::<u8>(), dest, argv.length as usize);
        }
        argv.command_line = dest.cast::<c_char>();
        build_argv(argv);
        set_argv_arena1_lo(argv.end_argv.cast::<u8>());
    }
}

/// # Safety
///
/// Matches the runtime entry ABI. The unreachable non-PowerPC fallback does not access any pointers.
pub unsafe fn sys_init() {
    unsafe {
        SYS_RUNTIME_INIT_CALLED.store(true, Ordering::Release);
        SYS_RUNTIME_FP_DISABLED_STAGE.store(0, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        ogc_sys::set_exit_callback(Some(sys_exit_callback));
        #[cfg(target_arch = "powerpc")]
        record_fp_disabled_stage(0);
        if SYS_RUNTIME_INIT_STARTED.swap(true, Ordering::AcqRel) {
            return;
        }

        // The C libogc startup stores runtime state in C globals. This Rust
        // implementation uses fixed scheduler and IPC state instead. The shared
        // plan keeps the observable order testable while the operation closure
        // performs the platform calls.
        SYS_RUNTIME_INIT_COMPLETED.store(false, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        {
            let is_wii = { sys_get_hollywood_revision() } != 0;
            SYS_RUNTIME_IS_WII.store(is_wii, Ordering::Release);
            startup_plan::run(
                // These PowerPC targets produce DOL applications, not an IPL
                // image. C libogc skips memory protection for IPL builds.
                startup_plan::steps(is_wii, false),
                |step| {
                    let succeeded = match step {
                        startup_plan::StartupStep::LowMemory => {
                            SYS_STARTUP_LOW_MEM_INIT_CALLED.store(true, Ordering::Release);
                            initialize_low_memory(is_wii);
                            true
                        }
                        startup_plan::StartupStep::IpcBuffers => {
                            // IPC request storage and queues are fixed, aligned Rust statics.
                            true
                        }
                        startup_plan::StartupStep::LwpWorkspace => {
                            let workspace_ready = initialize_lwp_workspace();
                            if workspace_ready {
                                lwp::set_workspace_allocator(
                                    Some(lwp_workspace_alloc_callback),
                                    Some(lwp_workspace_free_callback),
                                );
                            }
                            lwp::initialize();
                            workspace_ready
                        }
                        startup_plan::StartupStep::ResetQueue => {
                            let _guard = ResetQueueGuard::acquire();
                            SYS_RESET_QUEUE_HEAD.store(0, Ordering::Release);
                            true
                        }
                        startup_plan::StartupStep::Exi => {
                            exi::init();
                            true
                        }
                        startup_plan::StartupStep::Sram => {
                            SYS_SRAM_BUSY.store(false, Ordering::Release);
                            SYS_SRAM_LOCKED.store(false, Ordering::Release);
                            SYS_SRAM_EX_LOCKED.store(false, Ordering::Release);
                            SYS_SRAM_WRITE_OFFSET.store(64, Ordering::Release);
                            let ready = { sram_read_powerpc() };
                            SYS_SRAM_SYNC_RESULT.store(ready, Ordering::Release);
                            ready
                        }
                        startup_plan::StartupStep::Si => {
                            // SAFETY: These accessors validate channels and use the
                            // system SRAM state prepared by the preceding startup step.
                            {
                                si::register_wireless_id_accessors(
                                    si_wireless_id_getter,
                                    si_wireless_id_setter,
                                );
                            }
                            si::init();
                            true
                        }
                        startup_plan::StartupStep::Dsp => {
                            dsp::init();
                            true
                        }
                        startup_plan::StartupStep::MemoryProtection => {
                            initialize_memory_protection();
                            true
                        }
                        startup_plan::StartupStep::Callbacks => {
                            SYS_RESET_CALLBACK.store(
                                default_reset_callback as *const () as usize,
                                Ordering::Release,
                            );
                            SYS_POWER_CALLBACK.store(
                                default_power_callback as *const () as usize,
                                Ordering::Release,
                            );
                            SYS_RUNTIME_POWER_STATE.store(0, Ordering::Release);
                            true
                        }
                        startup_plan::StartupStep::IpcClient => {
                            ipc::reinitialize();
                            true
                        }
                        startup_plan::StartupStep::ResetIrq => {
                            initialize_reset_irq();
                            true
                        }
                    };
                    record_fp_disabled_stage(step.stage());
                    succeeded
                },
                |stage| SYS_RUNTIME_INIT_STAGE.store(stage, Ordering::Release),
                |bit| {
                    SYS_RUNTIME_INIT_COMPLETED_MASK.fetch_or(bit, Ordering::AcqRel);
                },
                |stage| SYS_RUNTIME_INIT_FAILED_STAGE.store(stage, Ordering::Release),
            );

            let gbs = sys_get_gbs_mode();
            sys_set_gbs_mode(gbs);
            record_fp_disabled_stage(12);
        }

        #[cfg(not(target_arch = "powerpc"))]
        {
            SYS_RUNTIME_INIT_STAGE.store(1, Ordering::Release);
            SYS_STARTUP_LOW_MEM_INIT_CALLED.store(true, Ordering::Release);
            SYS_RUNTIME_INIT_COMPLETED_MASK.fetch_or(1 << 0, Ordering::AcqRel);
        }

        if SYS_RUNTIME_IS_WII.load(Ordering::Acquire) {
            let result = with_clock(|state| state.initialize(read_timebase()));
            if let Err(_error) = result {
                // Retained by ClockState; checked reads return the real error.
                SYS_RUNTIME_INIT_FAILED_STAGE.store(13, Ordering::Release);
            }
        }
        SYS_RUNTIME_INIT_STAGE.store(0, Ordering::Release);
        SYS_RUNTIME_INIT_COMPLETED.store(true, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        ogc_sys::enable_runtime_interrupts();
    }
}

#[cfg(target_arch = "powerpc")]
fn record_fp_disabled_stage(stage: u32) {
    let msr: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) msr, options(nostack, preserves_flags));
    }
    if msr & (1 << 13) == 0 {
        let recorded_stage = if stage == 0 { 13 } else { stage };
        if let Err(existing_stage) = SYS_RUNTIME_FP_DISABLED_STAGE.compare_exchange(
            0,
            recorded_stage,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            debug_assert_ne!(existing_stage, 0);
        }
    }
}

pub fn sys_runtime_fp_disabled_stage() -> u32 {
    SYS_RUNTIME_FP_DISABLED_STAGE.load(Ordering::Acquire)
}

#[cfg(target_arch = "powerpc")]
fn initialize_lwp_workspace() -> bool {
    // Std workers use dynamically allocated stacks. Reserve enough room for
    // two concurrent std default stacks plus scheduler objects, rather than
    // 64 KiB, which could not hold even one requested stack.
    let current_lo = SYS_ARENA1_LO.load(Ordering::Acquire);
    let Some(start) = current_lo.checked_add(31).map(|value| value & !31) else {
        return false;
    };
    let Some(end) = current_lo.checked_add(LWP_WORKSPACE_SIZE) else {
        return false;
    };
    let Some(usable_size) = end.checked_sub(start) else {
        return false;
    };
    unsafe {
        core::ptr::write_bytes(start as *mut u8, 0, usable_size);
        if SYS_LWP_WORKSPACE
            .initialize_region(start as *mut u8, usable_size)
            .is_err()
        {
            return false;
        }
    }
    SYS_ARENA1_LO.store(end, Ordering::Release);
    true
}

#[cfg(all(test, target_arch = "powerpc"))]
mod lwp_workspace_tests {
    use super::LWP_WORKSPACE_SIZE;
    use ogc_alloc::ArenaAllocator;

    #[repr(align(32))]
    struct TestMemory([u8; LWP_WORKSPACE_SIZE]);

    #[test]
    fn lwp_workspace_holds_two_large_std_worker_stacks() {
        // std's Wii default is 128 KiB. Include alignment and stack-frame
        // allowance from the LWP provider for each concurrent allocation.
        const STACK_ALLOCATION: usize = 131_200;

        let allocator = ArenaAllocator::new();
        let mut memory = TestMemory([0; LWP_WORKSPACE_SIZE]);
        unsafe { allocator.initialize_region(memory.0.as_mut_ptr(), memory.0.len()) }.unwrap();

        let first = allocator.allocate(STACK_ALLOCATION, 32);
        let second = allocator.allocate(STACK_ALLOCATION, 32);
        assert!(!first.is_null());
        assert!(!second.is_null());
        assert_eq!(first as usize % 32, 0);
        assert_eq!(second as usize % 32, 0);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn allocate_lwp_workspace(size: usize) -> *mut u8 {
    let interrupt_state = unsafe { sram_disable_interrupts() };
    let allocation = SYS_LWP_WORKSPACE.allocate(size, 32);
    unsafe { sram_restore_interrupts(interrupt_state) };
    allocation
}

#[cfg(target_arch = "powerpc")]
unsafe fn free_lwp_workspace(allocation: *mut u8, size: usize) {
    let interrupt_state = unsafe { sram_disable_interrupts() };
    unsafe { SYS_LWP_WORKSPACE.deallocate(allocation, size, 32) };
    unsafe { sram_restore_interrupts(interrupt_state) };
}

#[cfg(target_arch = "powerpc")]
unsafe fn lwp_workspace_alloc_callback(size: usize) -> *mut u8 {
    unsafe { allocate_lwp_workspace(size) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn lwp_workspace_free_callback(allocation: *mut u8, size: usize) {
    unsafe { free_lwp_workspace(allocation, size) };
}

extern "C" fn sys_exit_callback(status: i32) -> ! {
    sys_exit_to_loader(status)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SysExitRoute {
    ReloadStub,
    Reset(u32),
}

fn sys_exit_route(main_loop_done: bool, stub_present: bool, power_state: u32) -> SysExitRoute {
    if !main_loop_done && stub_present {
        SysExitRoute::ReloadStub
    } else if power_state == 2 {
        SysExitRoute::Reset(SYS_POWEROFF)
    } else {
        SysExitRoute::Reset(SYS_HOTRESET)
    }
}

/// Exit through libogc's process-exit route.
///
/// Return to the HBC loader when the runtime has not completed its main loop
/// and the loader stub is present. Otherwise, use the reset or power-off path
/// selected by libogc. The C implementation ignores `status`.
pub fn sys_exit_to_loader(_status: i32) -> ! {
    reset_debug(SYS_RESET_TRACE_EXIT_BEGIN, 0, _status);
    #[cfg(ogc_dev)]
    dev::finish(_status);
    reset_debug(SYS_RESET_TRACE_EXIT_FINISH, 0, _status);
    let route = sys_exit_route(
        SYS_RUNTIME_MAIN_LOOP_DONE.load(Ordering::Acquire),
        sys_hbc_reload_stub_present(),
        SYS_RUNTIME_POWER_STATE.load(Ordering::Acquire),
    );
    match route {
        SysExitRoute::ReloadStub => {
            reset_debug(SYS_RESET_TRACE_EXIT_ROUTE, SYS_SHUTDOWN, 1);
            unsafe { sys_reset_system(SYS_SHUTDOWN as i32, 0, 0) };
            reset_debug(SYS_RESET_TRACE_RELOAD_STUB, SYS_SHUTDOWN, 0);
            // Match libogc's __irq_shutdown before transferring control to the
            // loader stub. The DOL begins at 0x80004000, so its BSS clear and
            // arena allocations do not cover the stub at 0x80001800.
            #[cfg(target_arch = "powerpc")]
            unsafe {
                let current_msr: u32;
                asm!("mfmsr {msr}", msr = out(reg) current_msr, options(nostack));
                let exit_msr = reset_shutdown_msr(current_msr);
                asm!(
                    "mtmsr {msr}",
                    "isync",
                    "lis 12, 0x8000",
                    "ori 12, 12, 0x1800",
                    "mtctr 12",
                    "bctr",
                    msr = in(reg) exit_msr,
                    options(noreturn)
                );
            }
        }
        SysExitRoute::Reset(reset) => {
            reset_debug(SYS_RESET_TRACE_EXIT_ROUTE, reset, 0);
            unsafe { sys_reset_system(reset as i32, 0, 0) };
        }
    }
    loop {
        core::hint::spin_loop();
    }
}

/// Return true when the HBC loader left its reload stub in low memory.
#[cfg(target_arch = "powerpc")]
pub fn sys_hbc_reload_stub_present() -> bool {
    ogc_sys::hbc_reload_stub_present()
}

#[cfg(not(target_arch = "powerpc"))]
pub fn sys_hbc_reload_stub_present() -> bool {
    false
}

/*
 * Keep the exit-route policy independently testable. It mirrors C
 * __syscall_exit: the loader stub takes priority until SYS_MainLoop has
 * completed; afterward the recorded power event selects reset or power-off.
 */
#[cfg(test)]
mod exit_route_tests {
    use super::{SYS_HOTRESET, SYS_POWEROFF, SysExitRoute, sys_exit_route};

    #[test]
    fn stub_is_used_until_main_loop_completes() {
        assert_eq!(sys_exit_route(false, true, 0), SysExitRoute::ReloadStub);
        assert_eq!(
            sys_exit_route(true, true, 0),
            SysExitRoute::Reset(SYS_HOTRESET)
        );
    }

    #[test]
    fn fallback_matches_reset_and_power_button_state() {
        assert_eq!(
            sys_exit_route(false, false, 0),
            SysExitRoute::Reset(SYS_HOTRESET)
        );
        assert_eq!(
            sys_exit_route(true, true, 2),
            SysExitRoute::Reset(SYS_POWEROFF)
        );
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn default_reset_callback(_irq: u32, _context: *mut c_void) {}

#[cfg(target_arch = "powerpc")]
unsafe fn default_power_callback() {}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn memory_exception_handler(_irq: u32, _context: *mut c_void) {
    const MEM_REG_BASE: usize = 0xCC00_4000;
    unsafe { write_volatile((MEM_REG_BASE + 16 * 4) as *mut u32, 0) };
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn memory_reset_function(_final_: i32) -> i32 {
    const MEM_REG_BASE: usize = 0xCC00_4000;
    if _final_ != 0 {
        unsafe { write_volatile((MEM_REG_BASE + 8 * 4) as *mut u16, 255) };
        irq::unmask(
            (1 << irq::IRQ_MEM0)
                | (1 << irq::IRQ_MEM1)
                | (1 << irq::IRQ_MEM2)
                | (1 << irq::IRQ_MEM3),
        );
    }
    1
}

#[cfg(target_arch = "powerpc")]
static mut MEMORY_RESET_INFO: SysResetInfo = SysResetInfo {
    node: LwpNode {
        next: null_mut(),
        prev: null_mut(),
    },
    func: Some(memory_reset_function),
    prio: 127,
};

#[cfg(target_arch = "powerpc")]
fn initialize_memory_protection() {
    use ogc_irq::{IRQ_MEM0, IRQ_MEM1, IRQ_MEM2, IRQ_MEM3, IRQ_MEMADDRESS};

    const MEM_REG_BASE: usize = 0xCC00_4000;
    let memory_irq_mask = (1 << IRQ_MEM0)
        | (1 << IRQ_MEM1)
        | (1 << IRQ_MEM2)
        | (1 << IRQ_MEM3)
        | (1 << IRQ_MEMADDRESS);
    irq::mask(memory_irq_mask);
    unsafe {
        write_volatile((MEM_REG_BASE + 16 * 4) as *mut u32, 0);
        write_volatile((MEM_REG_BASE + 8 * 4) as *mut u32, 255);
        for id in [IRQ_MEM0, IRQ_MEM1, IRQ_MEM2, IRQ_MEM3, IRQ_MEMADDRESS] {
            let _ = irq::request(id, Some(memory_exception_handler), null_mut());
        }
        sys_register_reset_func(core::ptr::addr_of_mut!(MEMORY_RESET_INFO));
    }
    irq::unmask(1 << IRQ_MEMADDRESS);
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn reset_switch_handler(id: u32, context: *mut c_void) {
    if unsafe { sys_reset_button_down() } != 0 {
        irq::mask(1 << id);
        let callback = SYS_RESET_CALLBACK.load(Ordering::Acquire);
        if callback != 0 {
            let callback = unsafe {
                core::mem::transmute::<usize, unsafe extern "C" fn(u32, *mut c_void)>(callback)
            };
            unsafe { callback(id, context) };
        }
        irq::unmask(1 << id);
    }
    const PI_RESET_BUTTON_REG: usize = 0xCC00_3000;
    unsafe { write_volatile(PI_RESET_BUTTON_REG as *mut u32, 2) };
}

#[cfg(target_arch = "powerpc")]
fn initialize_reset_irq() {
    irq::mask(1 << irq::IRQ_PI_RSW);
    unsafe {
        let _ = irq::request(irq::IRQ_PI_RSW, Some(reset_switch_handler), null_mut());
    }
}

#[cfg(test)]
unsafe fn sys_init_with_sram_read(read_sram: unsafe fn() -> bool) -> bool {
    SYS_RUNTIME_INIT_CALLED.store(true, Ordering::Release);
    SYS_STARTUP_LOW_MEM_INIT_CALLED.store(true, Ordering::Release);
    if cfg!(target_arch = "powerpc") {
        seed_runtime_arenas(true);
        SYS_SRAM_BUSY.store(false, Ordering::Release);
        SYS_SRAM_LOCKED.store(false, Ordering::Release);
        SYS_SRAM_EX_LOCKED.store(false, Ordering::Release);
        let sram_ready = unsafe { read_sram() };
        SYS_SRAM_SYNC_RESULT.store(sram_ready, Ordering::Release);
        if !sram_ready {
            return false;
        }
        let gbs = sys_get_gbs_mode();
        sys_set_gbs_mode(gbs);
    }
    true
}

/// Installs an early-boot diagnostic callback invoked before each pre-main
/// subsystem operation. Passing `None` removes the callback.
pub fn set_pre_main_progress_callback(callback: Option<unsafe fn(u32)>) {
    SYS_PRE_MAIN_PROGRESS_CALLBACK.store(callback.map_or(0, |cb| cb as usize), Ordering::Release);
}

#[cfg(any(not(gamecube), test))]
fn report_pre_main_progress(stage: u32) {
    let callback = SYS_PRE_MAIN_PROGRESS_CALLBACK.load(Ordering::Acquire);
    if callback != 0 {
        let callback: unsafe fn(u32) = unsafe { core::mem::transmute(callback) };
        unsafe { callback(stage) };
    }
}

#[cfg(any(not(gamecube), test))]
type StmHandler = Option<unsafe fn(u32)>;

#[cfg(any(not(gamecube), test))]
struct PreMainOperations {
    load_startup_ios: fn() -> i32,
    initialize_ios: fn() -> i32,
    register_stm_handler: fn(StmHandler) -> StmHandler,
    initialize_stm: fn() -> i32,
    initialize_conf: fn() -> i32,
    initialize_wiilaunch: fn() -> i32,
}

#[cfg(any(not(gamecube), test))]
fn finish_pre_main(result: i32, failed_stage: u32, completed: bool) -> i32 {
    SYS_RUNTIME_PRE_MAIN_RESULT.store(result, Ordering::Release);
    SYS_RUNTIME_PRE_MAIN_FAILED_STAGE.store(failed_stage, Ordering::Release);
    SYS_RUNTIME_PRE_MAIN_COMPLETED.store(completed, Ordering::Release);
    result
}

#[cfg(any(not(gamecube), test))]
fn sys_pre_main_with_operations(operations: &PreMainOperations) -> i32 {
    SYS_RUNTIME_PRE_MAIN_CALLED.store(true, Ordering::Release);
    SYS_RUNTIME_PRE_MAIN_COMPLETED.store(false, Ordering::Release);
    SYS_RUNTIME_PRE_MAIN_FAILED_STAGE.store(0, Ordering::Release);
    SYS_RUNTIME_PRE_MAIN_RESULT.store(0, Ordering::Release);

    // C libogc closes the inherited IOS descriptors before loading startup
    // services. HBC can leave these descriptors open, so close them even
    // though an unused descriptor is expected to report an error.
    report_pre_main_progress(0);
    for fd in 0..32 {
        let _ = ogc_ipc::FileDescriptor::new(fd).close();
    }

    report_pre_main_progress(1);
    let result = (operations.load_startup_ios)();
    let mut first_failure = (result < 0).then_some((1, result));

    report_pre_main_progress(2);
    let result = (operations.initialize_ios)();
    if result < 0 && first_failure.is_none() {
        first_failure = Some((2, result));
    }

    report_pre_main_progress(3);
    let _previous_handler = (operations.register_stm_handler)(Some(sys_stm_event_handler));

    report_pre_main_progress(4);
    let result = (operations.initialize_stm)();
    if result <= 0 && first_failure.is_none() {
        first_failure = Some((
            4,
            if result < 0 {
                result
            } else {
                SYS_PRE_MAIN_ESTM
            },
        ));
    }

    report_pre_main_progress(5);
    let result = (operations.initialize_conf)();
    if result < 0 && first_failure.is_none() {
        first_failure = Some((5, result));
    }

    report_pre_main_progress(6);
    let result = (operations.initialize_wiilaunch)();
    if result < 0 && first_failure.is_none() {
        first_failure = Some((6, result));
    }

    SYS_RUNTIME_POWER_STATE.store(0, Ordering::Release);
    SYS_POWER_ENABLE_TIME.store(0, Ordering::Release);
    report_pre_main_progress(7);
    let (failed_stage, result) = first_failure.unwrap_or((0, 0));
    finish_pre_main(result, failed_stage, failed_stage == 0)
}

/// # Safety
///
/// Matches the runtime entry ABI. The unreachable non-PowerPC fallback does not access any pointers.
pub unsafe fn sys_pre_main() {
    if cfg!(target_arch = "powerpc") {
        #[cfg(not(gamecube))]
        if unsafe { sys_get_hollywood_revision() } != 0 {
            let operations = PreMainOperations {
                load_startup_ios: ios::_ios_load_startup_ios,
                initialize_ios: ios::_ios_initialize_subsystems,
                register_stm_handler: stm_register_event_handler_local,
                initialize_stm: stm_init_local,
                initialize_conf: conf::init,
                initialize_wiilaunch: wiilaunch::initialize,
            };
            let _ = sys_pre_main_with_operations(&operations);
        }
        set_boot_time();
    }
}

/// # Safety
///
/// Matches the runtime entry ABI. The unreachable non-PowerPC fallback does not access any pointers.
pub unsafe fn sys_main_loop() -> Bool {
    SYS_RUNTIME_MAIN_LOOP_CALLED.store(true, Ordering::Release);

    let done = SYS_RUNTIME_MAIN_LOOP_DONE.load(Ordering::Acquire);
    let is_wii = SYS_RUNTIME_IS_WII.load(Ordering::Acquire);
    let power_state = SYS_RUNTIME_POWER_STATE.load(Ordering::Acquire);
    let reset_button_down = !done && !is_wii && unsafe { sys_reset_button_down() } != 0;
    if !main_loop_should_continue(done, is_wii, power_state, reset_button_down) {
        SYS_RUNTIME_MAIN_LOOP_DONE.store(true, Ordering::Release);
        SYS_RUNTIME_MAIN_LOOP_RESULT.store(false, Ordering::Release);
        return 0;
    }
    SYS_RUNTIME_MAIN_LOOP_RESULT.store(true, Ordering::Release);
    1
}

fn main_loop_should_continue(
    done: bool,
    is_wii: bool,
    power_state: u32,
    reset_button_down: bool,
) -> bool {
    !done
        && if is_wii {
            power_state == 0
        } else {
            !reset_button_down
        }
}

/// # Safety
///
/// `rmode` must describe a valid video mode. The returned framebuffer is
/// uninitialized, as with C's `memalign`; clear it before reading or displaying
/// its pixels. This port reserves aligned arena memory rather than a C heap
/// allocation, so the returned buffer must not be passed to C `free`.
pub unsafe fn sys_allocate_framebuffer(_rmode: *mut GxrModeObj) -> *mut c_void {
    unsafe {
        SYS_FRAMEBUFFER_CALLED.store(true, Ordering::Release);
        SYS_FRAMEBUFFER_RMODE.store(_rmode as usize, Ordering::Release);
        let bytes = if _rmode.is_null() {
            0
        } else {
            let width = ((*_rmode).fb_width.wrapping_add(15)) & !15;
            let height = (*_rmode).xfb_height as u32 + if (*_rmode).aa != 0 { 4 } else { 0 };
            width as u32 * height * 2
        };
        SYS_FRAMEBUFFER_BYTES.store(bytes, Ordering::Release);
        if bytes == 0 {
            return core::ptr::null_mut();
        }

        // C's SYS_AllocateFramebuffer only allocates. In particular, it does
        // not dirty the cached alias before the caller clears through K1.
        sys_alloc_arena1_mem_lo(bytes, FRAMEBUFFER_ALIGNMENT as u32)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct FramebufferSnapshot {
    pub called: bool,
    pub rmode: usize,
    pub bytes: u32,
}

pub fn snapshot_framebuffer() -> FramebufferSnapshot {
    FramebufferSnapshot {
        called: SYS_FRAMEBUFFER_CALLED.load(Ordering::Acquire),
        rmode: SYS_FRAMEBUFFER_RMODE.load(Ordering::Acquire),
        bytes: SYS_FRAMEBUFFER_BYTES.load(Ordering::Acquire),
    }
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_ioctl(ioctl: u32, value: u32) -> i32 {
    let fd = match open_stm_immediate_with(open_file) {
        Ok(fd) => fd,
        Err(err) => return err,
    };

    let mut input = [0u32; 8];
    let mut output = [0u32; 8];
    input[0] = value;
    let in_bytes =
        unsafe { core::slice::from_raw_parts_mut(input.as_mut_ptr().cast::<u8>(), 0x20) };
    let out_bytes =
        unsafe { core::slice::from_raw_parts_mut(output.as_mut_ptr().cast::<u8>(), 0x20) };
    let rc = fd.ioctl(
        ioctl as i32,
        IoctlBuffers::from_slices(Some(in_bytes), Some(out_bytes)),
    );
    let _ = fd.close();
    rc
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_reboot_system() -> i32 {
    stm_ioctl(IOCTL_STM_HOTRESET, 0)
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_shutdown_to_standby() -> i32 {
    stm_ioctl(IOCTL_STM_SHUTDOWN, 0)
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_shutdown_to_idle() -> i32 {
    let value = match unsafe { sys_get_hollywood_revision() } {
        0..=2 => 0xFCA0_8280,
        _ => 0xFCE0_82C0,
    };
    stm_ioctl(IOCTL_STM_IDLE, value)
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn stm_set_led_mode(mode: u32) -> i32 {
    stm_ioctl(IOCTL_STM_LEDMODE, mode)
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn clear_wpad_active_devices(pads: &mut conf::ConfPads) {
    // ConfPads is packed, so clear the active-device bytes without creating
    // an unaligned reference to its active field.
    unsafe {
        core::ptr::write_bytes(
            (pads as *mut conf::ConfPads)
                .cast::<u8>()
                .add(offset_of!(conf::ConfPads, active)),
            0,
            size_of::<[conf::ConfPadDevice; conf::CONF_PAD_MAX_ACTIVE]>(),
        );
    }
}

#[cfg(all(target_arch = "powerpc", any(not(gamecube), test)))]
fn clear_wpad_active_list() -> i32 {
    let mut pads = conf::ConfPads::default();
    if conf::get_pad_devices(&mut pads) < 0 {
        return 1;
    }
    clear_wpad_active_devices(&mut pads);
    if conf::set_pad_devices(&pads) < 0 {
        return 1;
    }
    if conf::set_pad_guest_devices(Some(&conf::ConfPadGuests::default())) < 0 {
        return 1;
    }
    conf::save_changes()
}

#[cfg(target_arch = "powerpc")]
unsafe fn shutdown_interrupts() {
    let current_msr: u32;
    unsafe {
        asm!("mfmsr {msr}", msr = out(reg) current_msr, options(nostack));
        let shutdown_msr = reset_shutdown_msr(current_msr);
        asm!(
            "mtmsr {msr}",
            "isync",
            msr = in(reg) shutdown_msr,
            options(nostack)
        );
    }
}

#[cfg(target_arch = "powerpc")]
fn reset_shutdown_msr(current_msr: u32) -> u32 {
    const MSR_EE: u32 = 1 << 15;
    const MSR_FP: u32 = 1 << 13;
    const MSR_RI: u32 = 1 << 1;
    (current_msr & !MSR_EE) | MSR_FP | MSR_RI
}

#[cfg(target_arch = "powerpc")]
unsafe fn clear_reset_low_memory() {
    for (address, len) in RESET_LOWMEM_REGIONS {
        unsafe { write_bytes(address as *mut u8, 0, len) };
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn pad_disable_recalibration(disable: u32) -> u32 {
    unsafe {
        const LOWMEM_RECALIBRATION_DISABLE: *mut u8 = 0x8000_30e3 as *mut u8;
        let mut value = read_volatile(LOWMEM_RECALIBRATION_DISABLE);
        let prev = (value & 0x40) != 0;
        value &= !0x40;
        if disable != 0 {
            value |= 0x40;
        }
        write_volatile(LOWMEM_RECALIBRATION_DISABLE, value);
        prev as u32
    }
}

#[cfg(target_arch = "powerpc")]
#[cfg(not(gamecube))]
fn wii_reset_system(reset: u32, force_menu: bool) {
    dsp::shutdown_with_progress(Some(|stage, control, mailbox| {
        reset_debug(SYS_RESET_TRACE_DSP, stage, i32::from(control));
        if stage == 2 {
            reset_debug(SYS_RESET_TRACE_DSP_MAILBOX, stage, i32::from(mailbox));
        }
    }));
    let reset = if reset == SYS_HOTRESET {
        SYS_RETURNTOMENU
    } else {
        let _ = force_menu;
        reset
    };
    let mut result = if reset == SYS_SHUTDOWN {
        unsafe { pad_disable_recalibration(1) }
    } else {
        0
    };

    if matches!(
        reset,
        SYS_POWEROFF | SYS_POWEROFF_STANDBY | SYS_POWEROFF_IDLE
    ) {
        let _ = clear_wpad_active_list();
    }
    reset_debug(SYS_RESET_TRACE_CALLBACKS_BEGIN, reset, 0);
    while sys_run_reset_functions(0) == 0 {
        core::hint::spin_loop();
    }
    reset_debug(SYS_RESET_TRACE_CALLBACKS_END, reset, 1);
    reset_debug(SYS_RESET_TRACE_TERMINAL, reset, 0);

    match reset {
        SYS_RESTART => {
            let _ = stm_reboot_system();
        }
        SYS_SHUTDOWN => {}
        SYS_POWEROFF => {
            if conf::get_shutdown_mode() == conf::CONF_SHUTDOWN_IDLE as i32 {
                let led = conf::get_idle_led_mode();
                result = led as u32;
                if (0..=2).contains(&led) {
                    let _ = stm_set_led_mode(led as u32);
                }
                let _ = stm_shutdown_to_idle();
            } else {
                let _ = stm_shutdown_to_standby();
            }
        }
        SYS_POWEROFF_STANDBY => {
            let _ = stm_shutdown_to_standby();
        }
        SYS_POWEROFF_IDLE => {
            let led = conf::get_idle_led_mode();
            result = led as u32;
            if (0..=2).contains(&led) {
                let _ = stm_set_led_mode(led as u32);
            }
            let _ = stm_shutdown_to_idle();
        }
        SYS_RETURNTOMENU => {
            reset_debug(SYS_RESET_TRACE_RETURN_TO_MENU_BEGIN, reset, 0);
            let launch_result = wiilaunch::return_to_menu();
            reset_debug(SYS_RESET_TRACE_RETURN_TO_MENU_RESULT, reset, launch_result);
        }
        _ => {}
    }

    reset_debug(SYS_RESET_TRACE_IOS_SHUTDOWN_BEGIN, reset, 0);
    let shutdown_result = ios::_ios_shutdown_subsystems();
    reset_debug(SYS_RESET_TRACE_IOS_SHUTDOWN_RESULT, reset, shutdown_result);
    reset_debug(SYS_RESET_TRACE_INTERRUPTS_BEGIN, reset, 0);
    unsafe { shutdown_interrupts() };
    reset_debug(SYS_RESET_TRACE_INTERRUPTS_END, reset, 0);
    reset_debug(SYS_RESET_TRACE_FINAL_CALLBACKS_BEGIN, reset, 0);
    let final_result = sys_run_reset_functions(1);
    reset_debug(
        SYS_RESET_TRACE_FINAL_CALLBACKS_END,
        reset,
        final_result as i32,
    );
    reset_debug(SYS_RESET_TRACE_CACHE_BEGIN, reset, 0);
    cache::lc_disable();
    reset_debug(SYS_RESET_TRACE_CACHE_END, reset, 0);
    reset_debug(SYS_RESET_TRACE_LOWMEM_BEGIN, reset, 0);
    unsafe { clear_reset_low_memory() };
    reset_debug(SYS_RESET_TRACE_LOWMEM_END, reset, 0);
    unsafe {
        let _ = pad_disable_recalibration(result);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" {
    fn __ogc_ppc_hot_reset(code: u32) -> !;
}

#[cfg(target_arch = "powerpc")]
unsafe fn gamecube_hot_reset(code: u32) -> ! {
    unsafe {
        write_volatile(0xCC00_2002 as *mut u16, 0);
        let _hid0: u32;
        asm!(
            "mfspr {hid0}, 1008",
            "ori {hid0}, {hid0}, 0x0800",
            "mtspr 1008, {hid0}",
            "isync",
            hid0 = out(reg) _hid0,
            options(nostack)
        );
        __ogc_ppc_hot_reset(code)
    }
}

#[cfg(target_arch = "powerpc")]
fn gamecube_reset_system(reset: u32, reset_code: u32, force_menu: bool) {
    dsp::shutdown();
    let recalibration = if reset == SYS_SHUTDOWN {
        unsafe { pad_disable_recalibration(1) }
    } else {
        0
    };

    while sys_run_reset_functions(0) == 0 {
        core::hint::spin_loop();
    }

    if reset == SYS_HOTRESET && force_menu {
        unsafe {
            let sram = _sys_lock_sram();
            if !sram.is_null() {
                (*sram).flags |= 0x40;
                let _ = _sys_unlock_sram(1);
            }
            while _sys_sync_sram() == 0 {
                core::hint::spin_loop();
            }
        }
    }

    unsafe { shutdown_interrupts() };
    let _ = sys_run_reset_functions(1);
    cache::lc_disable();

    if reset == SYS_HOTRESET {
        unsafe { gamecube_hot_reset(reset_code.wrapping_shl(3)) };
    } else if reset == SYS_RESTART {
        unsafe {
            write_volatile(0x817F_FFFC as *mut u32, 0);
            write_volatile(0x817F_FFF8 as *mut u32, 0);
            write_volatile(0x8000_30E2 as *mut u8, 1);
            gamecube_hot_reset(reset_code.wrapping_shl(3));
        }
    }

    unsafe { clear_reset_low_memory() };
    unsafe {
        let _ = pad_disable_recalibration(recalibration);
    }
}

/// # Safety
///
/// Matches the reset ABI. The unreachable non-PowerPC fallback records no hardware side effects.
pub unsafe fn sys_reset_system(reset: i32, reset_code: u32, force_menu: i32) {
    reset_debug(SYS_RESET_TRACE_RESET_BEGIN, reset as u32, force_menu);
    SYS_RESET_SYSTEM_CALLED.store(true, Ordering::Release);
    SYS_RESET_SYSTEM_RESET.store(reset as u32, Ordering::Release);
    SYS_RESET_SYSTEM_CODE.store(reset_code, Ordering::Release);
    SYS_RESET_SYSTEM_FORCE_MENU.store(force_menu != 0, Ordering::Release);
    let terminal_hint = match reset as u32 {
        SYS_RESTART => 1,
        SYS_HOTRESET | SYS_RETURNTOMENU => 2,
        SYS_SHUTDOWN => 3,
        SYS_POWEROFF | SYS_POWEROFF_STANDBY | SYS_POWEROFF_IDLE => 4,
        _ => 0,
    };
    SYS_RESET_SYSTEM_TERMINAL_HINT.store(terminal_hint, Ordering::Release);

    if cfg!(target_arch = "powerpc") {
        #[cfg(not(gamecube))]
        if unsafe { sys_get_hollywood_revision() } != 0 {
            wii_reset_system(reset as u32, force_menu != 0);
        } else if !cfg!(gamecube) {
            gamecube_reset_system(reset as u32, reset_code, force_menu != 0);
        }
        #[cfg(gamecube)]
        gamecube_reset_system(reset as u32, reset_code, force_menu != 0);
    }
    reset_debug(SYS_RESET_TRACE_RESET_RETURN, reset as u32, 0);
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ResetSystemSnapshot {
    pub called: bool,
    pub reset: u32,
    pub reset_code: u32,
    pub force_menu: bool,
    pub terminal_hint: u32,
}

pub fn snapshot_reset_system() -> ResetSystemSnapshot {
    ResetSystemSnapshot {
        called: SYS_RESET_SYSTEM_CALLED.load(Ordering::Acquire),
        reset: SYS_RESET_SYSTEM_RESET.load(Ordering::Acquire),
        reset_code: SYS_RESET_SYSTEM_CODE.load(Ordering::Acquire),
        force_menu: SYS_RESET_SYSTEM_FORCE_MENU.load(Ordering::Acquire),
        terminal_hint: SYS_RESET_SYSTEM_TERMINAL_HINT.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the font encoding ABI. The unreachable non-PowerPC fallback reports the default encoding.
pub unsafe fn sys_get_font_encoding() -> u32 {
    SYS_FONT_ENCODING_QUERIED.store(true, Ordering::Release);

    let cached = SYS_FONT_ENCODING.load(Ordering::Acquire);
    if let Some(encoding) = cached_font_encoding(cached) {
        return encoding;
    }

    let tv_mode = ogc_video_sys::get_current_tv_mode();
    // SAFETY: VI register 55 is a readable 16-bit hardware register on Wii/GC.
    let vi_flags = unsafe {
        core::ptr::read_volatile((VI_REG_BASE as *const u16).add(VI_FONT_ENCODING_FLAGS_REGISTER))
    };
    let has_480p_flag = vi_flags & VI_DTV_FLAG != 0;
    let encoding = u32::from(tv_mode == 0 && has_480p_flag);

    match SYS_FONT_ENCODING.compare_exchange(cached, encoding, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) => encoding,
        Err(current) => cached_font_encoding(current).unwrap_or(encoding),
    }
}

fn cached_font_encoding(cached: u32) -> Option<u32> {
    if cached <= 1 { Some(cached) } else { None }
}

#[cfg(test)]
mod font_encoding_tests {
    use super::cached_font_encoding;

    #[test]
    fn accepts_only_the_two_c_abi_font_encoding_values() {
        assert_eq!(cached_font_encoding(0), Some(0));
        assert_eq!(cached_font_encoding(1), Some(1));
        assert_eq!(cached_font_encoding(2), None);
        assert_eq!(cached_font_encoding(u32::MAX), None);
    }
}

#[cfg(target_arch = "powerpc")]
const FONT_ROM_ANSI_OFFSET: usize = 2_084_608;
#[cfg(target_arch = "powerpc")]
const FONT_ROM_ANSI_LEN: usize = 12_288;
#[cfg(target_arch = "powerpc")]
const FONT_ROM_SJIS_OFFSET: usize = 1_769_216;
#[cfg(target_arch = "powerpc")]
const FONT_ROM_SJIS_LEN: usize = 315_392;
#[cfg(target_arch = "powerpc")]
const FONT_READ_MAX: usize = FONT_ROM_SJIS_LEN;

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct FontReadBuffer([u8; FONT_READ_MAX]);

#[cfg(target_arch = "powerpc")]
static mut SYS_FONT_READ_BUFFER: FontReadBuffer = FontReadBuffer([0; FONT_READ_MAX]);

#[cfg(target_arch = "powerpc")]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct FontLayout {
    rom_offset: usize,
    rom_len: usize,
    font_len: usize,
}

#[cfg(target_arch = "powerpc")]
fn font_layout_for_encoding(encoding: u32) -> FontLayout {
    if encoding == 1 {
        FontLayout {
            rom_offset: FONT_ROM_SJIS_OFFSET,
            rom_len: FONT_ROM_SJIS_LEN,
            font_len: SYS_FONTSIZE_SJIS as usize,
        }
    } else {
        FontLayout {
            rom_offset: FONT_ROM_ANSI_OFFSET,
            rom_len: FONT_ROM_ANSI_LEN,
            font_len: SYS_FONTSIZE_ANSI as usize,
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn clear_font_state() {
    SYS_FONT_LOADED.store(false, Ordering::Release);
    SYS_FONT_DATA_PTR.store(0, Ordering::Release);
    SYS_FONT_IMAGE_PTR.store(0, Ordering::Release);
    SYS_FONT_DECODED_LEN.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
unsafe fn read_rom(dest: *mut u8, len: usize, offset: usize) -> bool {
    unsafe {
        let mut remaining = len;
        let mut cursor = 0usize;

        while remaining > 0 {
            let chunk = remaining.min(1024 - (offset.wrapping_add(cursor) % 1024));
            let chunk_dest = dest.add(cursor);
            cache::dc_invalidate_range(chunk_dest.cast::<c_void>(), chunk as u32);

            if exi::lock(EXI_CHANNEL_0, EXI_DEVICE_1, None) == 0 {
                return false;
            }
            if exi::select(EXI_CHANNEL_0, EXI_DEVICE_1, EXI_SPEED32MHZ) == 0 {
                let _ = exi::unlock(EXI_CHANNEL_0);
                return false;
            }

            let mut loff = ((offset + cursor) as u32) << 6;
            let mut ok = exi::imm(
                EXI_CHANNEL_0,
                core::ptr::addr_of_mut!(loff).cast::<c_void>(),
                4,
                EXI_WRITE as i32,
                None,
            ) != 0;
            if ok && exi::sync(EXI_CHANNEL_0) == 0 {
                ok = false;
            }
            if ok
                && exi::dma(
                    EXI_CHANNEL_0,
                    chunk_dest.cast::<c_void>(),
                    chunk as u32,
                    EXI_READ as i32,
                    None,
                ) == 0
            {
                ok = false;
            }
            if ok && exi::sync(EXI_CHANNEL_0) == 0 {
                ok = false;
            }
            if exi::deselect(EXI_CHANNEL_0) == 0 {
                ok = false;
            }
            if exi::unlock(EXI_CHANNEL_0) == 0 {
                ok = false;
            }
            if !ok {
                return false;
            }

            cursor += chunk;
            remaining -= chunk;
        }

        true
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn decode_yay0(src: &[u8], dest: *mut u8, dest_len: usize) -> Option<usize> {
    unsafe {
        if src.len() < 16 || dest.is_null() || dest_len == 0 {
            return None;
        }
        if &src[..4] != b"Yay0" {
            return None;
        }

        let decoded_len = u32::from_be_bytes([src[4], src[5], src[6], src[7]]) as usize;
        let links_offset = u32::from_be_bytes([src[8], src[9], src[10], src[11]]) as usize;
        let chunks_offset = u32::from_be_bytes([src[12], src[13], src[14], src[15]]) as usize;
        if decoded_len == 0
            || decoded_len > dest_len
            || links_offset >= src.len()
            || chunks_offset >= src.len()
        {
            return None;
        }

        let mut mask_offset = 16usize;
        let mut link_offset = links_offset;
        let mut chunk_offset = chunks_offset;
        let mut written = 0usize;
        let dest_ptr = dest;

        let mut cmask = 0u32;
        let mut bits_remaining = 0u32;

        while written < decoded_len {
            if bits_remaining == 0 {
                if mask_offset.checked_add(4).is_none_or(|end| end > src.len()) {
                    return None;
                }
                cmask = u32::from_be_bytes([
                    src[mask_offset],
                    src[mask_offset + 1],
                    src[mask_offset + 2],
                    src[mask_offset + 3],
                ]);
                mask_offset += 4;
                bits_remaining = 32;
            }

            if (cmask & 0x8000_0000) != 0 {
                if chunk_offset >= src.len() {
                    return None;
                }
                *dest_ptr.add(written) = src[chunk_offset];
                chunk_offset += 1;
                written += 1;
            } else {
                if link_offset.checked_add(2).is_none_or(|end| end > src.len()) {
                    return None;
                }
                let link = u16::from_be_bytes([src[link_offset], src[link_offset + 1]]) as usize;
                link_offset += 2;
                if written <= (link & 0x0fff) {
                    return None;
                }
                let mut source_index = written - (link & 0x0fff) - 1;
                let mut count = link >> 12;
                if count == 0 {
                    if chunk_offset >= src.len() {
                        return None;
                    }
                    count = usize::from(src[chunk_offset]) + 18;
                    chunk_offset += 1;
                } else {
                    count += 2;
                }
                if written
                    .checked_add(count)
                    .is_none_or(|end| end > decoded_len)
                {
                    return None;
                }
                let mut copied = 0usize;
                while copied < count {
                    *dest_ptr.add(written) = *dest_ptr.add(source_index);
                    written += 1;
                    source_index += 1;
                    copied += 1;
                }
            }

            cmask <<= 1;
            bits_remaining -= 1;
        }

        Some(decoded_len)
    }
}

#[cfg(target_arch = "powerpc")]
fn font_header_is_valid(header: &SysFontHeader, decoded_len: usize) -> bool {
    let first = header.first_char as usize;
    let last = header.last_char as usize;
    let inval = header.inval_char as usize;
    let glyph_count = match last
        .checked_sub(first)
        .and_then(|delta| delta.checked_add(1))
    {
        Some(count) if count > 0 => count,
        _ => return false,
    };

    if inval < first || inval > last {
        return false;
    }

    let width_table = header.width_table as usize;
    if width_table >= decoded_len
        || width_table
            .checked_add(glyph_count)
            .is_none_or(|end| end > decoded_len)
    {
        return false;
    }

    let sheet_image = header.sheet_image as usize;
    let compressed_sheet_len = match (header.sheet_fullsize as usize).checked_div(2) {
        Some(len) if len > 0 => len,
        _ => return false,
    };
    if sheet_image >= decoded_len
        || sheet_image
            .checked_add(compressed_sheet_len)
            .is_none_or(|end| end > decoded_len)
    {
        return false;
    }

    let sheet_size = header.sheet_size as usize;
    let sheet_fullsize = header.sheet_fullsize as usize;
    let sheet_column = header.sheet_column as usize;
    let sheet_row = header.sheet_row as usize;
    let sheet_width = header.sheet_width as usize;
    let sheet_height = header.sheet_height as usize;
    let cell_width = header.cell_width as usize;
    let cell_height = header.cell_height as usize;

    if sheet_size == 0 || sheet_fullsize == 0 || !sheet_fullsize.is_multiple_of(2) {
        return false;
    }
    if !sheet_fullsize.is_multiple_of(sheet_size) {
        return false;
    }
    if sheet_column == 0 || sheet_row == 0 || cell_width == 0 || cell_height == 0 {
        return false;
    }
    if sheet_width == 0
        || sheet_height == 0
        || !sheet_width.is_multiple_of(8)
        || !sheet_height.is_multiple_of(8)
    {
        return false;
    }
    if cell_width > sheet_width || cell_height > sheet_height {
        return false;
    }
    if let Some(min_sheet_size) = sheet_width.checked_mul(sheet_height).map(|value| value / 2) {
        if sheet_size < min_sheet_size {
            return false;
        }
    } else {
        return false;
    }

    let sheet_count = sheet_fullsize / sheet_size;
    let chars_per_sheet = sheet_column * sheet_row;
    if chars_per_sheet == 0 || sheet_count == 0 {
        return false;
    }
    if glyph_count > sheet_count.saturating_mul(chars_per_sheet) {
        return false;
    }

    let Some(sheet_image_aligned) = sheet_image.checked_next_multiple_of(32) else {
        return false;
    };
    match sheet_image_aligned.checked_add(sheet_fullsize) {
        Some(end) => end <= decoded_len,
        None => false,
    }
}

#[cfg(target_arch = "powerpc")]
fn current_font_state() -> Option<(SysFontHeader, *mut u8, *mut u8, usize)> {
    if !SYS_FONT_LOADED.load(Ordering::Acquire) {
        return None;
    }

    let font_data = SYS_FONT_DATA_PTR.load(Ordering::Acquire) as *mut u8;
    let font_image = SYS_FONT_IMAGE_PTR.load(Ordering::Acquire) as *mut u8;
    let decoded_len = SYS_FONT_DECODED_LEN.load(Ordering::Acquire);
    if font_data.is_null() || font_image.is_null() || decoded_len == 0 {
        return None;
    }

    let header = unsafe { read_unaligned(font_data.cast::<SysFontHeader>()) };
    if !font_header_is_valid(&header, decoded_len) {
        return None;
    }

    Some((header, font_data, font_image, decoded_len))
}

#[cfg(target_arch = "powerpc")]
fn font_glyph_index(header: &SysFontHeader, c: i32) -> Option<usize> {
    let first = header.first_char as i32;
    let last = header.last_char as i32;
    let invalid = header.inval_char as i32;
    let glyph = if c < first || c > last {
        invalid - first
    } else {
        c - first
    };
    if glyph < 0 {
        return None;
    }
    let glyph = glyph as usize;
    let glyph_count = (last - first + 1) as usize;
    (glyph < glyph_count).then_some(glyph)
}

#[cfg(target_arch = "powerpc")]
unsafe fn init_font_powerpc(font_data: *mut SysFontHeader) -> u32 {
    unsafe {
        let layout = font_layout_for_encoding(sys_get_font_encoding());
        if layout.font_len == 0 || layout.rom_len > FONT_READ_MAX {
            clear_font_state();
            return 0;
        }

        let font_bytes = font_data.cast::<u8>();
        core::ptr::write_bytes(font_bytes, 0, layout.font_len);

        let scratch = core::ptr::addr_of_mut!(SYS_FONT_READ_BUFFER.0).cast::<u8>();
        if !read_rom(scratch, layout.rom_len, layout.rom_offset) {
            clear_font_state();
            return 0;
        }

        let Some(decoded_len) = decode_yay0(
            core::slice::from_raw_parts(scratch, layout.rom_len),
            font_bytes,
            layout.font_len,
        ) else {
            clear_font_state();
            return 0;
        };

        let header = read_unaligned(font_data.cast::<SysFontHeader>());
        if !font_header_is_valid(&header, decoded_len) {
            clear_font_state();
            return 0;
        }

        let sheet_image = font_bytes.add(header.sheet_image as usize);
        let Some(image_offset) = (header.sheet_image as usize).checked_next_multiple_of(32) else {
            clear_font_state();
            return 0;
        };
        let font_image = font_bytes.add(image_offset);
        let expanded_len = header.sheet_fullsize as usize;
        let compressed_len = expanded_len / 2;
        if compressed_len == 0 {
            clear_font_state();
            return 0;
        }

        let mut idx = compressed_len;
        let palette = [header.c0, header.c1, header.c2, header.c3];
        while idx > 0 {
            idx -= 1;
            let byte = *sheet_image.add(idx);
            let out = font_image.add(idx * 2);
            let val1 = palette[((byte >> 6) & 0x03) as usize];
            let val2 = palette[((byte >> 4) & 0x03) as usize];
            *out.add(0) = (val1 & 0xf0) | (val2 & 0x0f);
            let val1 = palette[((byte >> 2) & 0x03) as usize];
            let val2 = palette[(byte & 0x03) as usize];
            *out.add(1) = (val1 & 0xf0) | (val2 & 0x0f);
        }
        cache::dc_store_range(font_image.cast::<c_void>(), expanded_len as u32);

        SYS_FONT_DATA_PTR.store(font_data as usize, Ordering::Release);
        SYS_FONT_IMAGE_PTR.store(font_image as usize, Ordering::Release);
        SYS_FONT_DECODED_LEN.store(decoded_len, Ordering::Release);
        SYS_FONT_LOADED.store(true, Ordering::Release);
        1
    }
}

/// # Safety
///
/// Matches the font initialization ABI. Invalid pointers or unreadable Wii font storage return zero.
pub unsafe fn sys_init_font(font_data: *mut SysFontHeader) -> u32 {
    SYS_FONT_INIT_CALLED.store(true, Ordering::Release);
    SYS_FONT_INIT_PTR.store(font_data as usize, Ordering::Release);
    if font_data.is_null() {
        clear_font_state();
        return 0;
    }

    #[cfg(target_arch = "powerpc")]
    unsafe {
        init_font_powerpc(font_data)
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = font_data;
        0
    }
}

/// # Safety
///
/// Matches the font texture ABI. Output pointers are only written when non-null.
pub unsafe fn sys_get_font_texture(
    c: i32,
    image: *mut *mut c_void,
    xpos: *mut i32,
    ypos: *mut i32,
    width: *mut i32,
) {
    SYS_FONT_TEXTURE_CALLED.store(true, Ordering::Release);
    SYS_FONT_TEXTURE_C.store(c as u32, Ordering::Release);

    unsafe {
        if !image.is_null() {
            *image = core::ptr::null_mut();
        }
        if !xpos.is_null() {
            *xpos = 0;
        }
        if !ypos.is_null() {
            *ypos = 0;
        }
        if !width.is_null() {
            *width = 0;
        }
    }

    let Some((header, font_data, font_image, _decoded_len)) = current_font_state() else {
        return;
    };
    let Some(glyph_index) = font_glyph_index(&header, c) else {
        return;
    };

    let sheet_size = header.sheet_size as usize;
    let chars_per_sheet = (header.sheet_column as usize) * (header.sheet_row as usize);
    let sheets = glyph_index / chars_per_sheet;
    let rem = glyph_index % chars_per_sheet;
    let sheet_count = header.sheet_fullsize as usize / sheet_size;
    if sheets >= sheet_count {
        return;
    }

    let glyph_width = unsafe { *font_data.add(header.width_table as usize).add(glyph_index) };
    unsafe {
        if !image.is_null() {
            *image = font_image.add(sheet_size * sheets).cast::<c_void>();
        }
        if !xpos.is_null() {
            *xpos = ((rem % (header.sheet_column as usize)) * header.cell_width as usize) as i32;
        }
        if !ypos.is_null() {
            *ypos = ((rem / (header.sheet_column as usize)) * header.cell_height as usize) as i32;
        }
        if !width.is_null() {
            *width = i32::from(glyph_width);
        }
    }
}

/// # Safety
///
/// Matches the font texel ABI. The destination must be large enough for the requested glyph copy.
pub unsafe fn sys_get_font_texel(
    c: i32,
    image: *mut c_void,
    pos: i32,
    stride: i32,
    width: *mut i32,
) {
    SYS_FONT_TEXEL_CALLED.store(true, Ordering::Release);
    SYS_FONT_TEXEL_C.store(c as u32, Ordering::Release);

    unsafe {
        if !width.is_null() {
            *width = 0;
        }
    }

    let Some((header, font_data, font_image, _decoded_len)) = current_font_state() else {
        return;
    };
    if image.is_null() || width.is_null() || pos < 0 || stride <= 0 {
        return;
    }
    let Some(glyph_index) = font_glyph_index(&header, c) else {
        return;
    };

    let cell_width = header.cell_width as usize;
    let cell_height = header.cell_height as usize;
    let stride = stride as usize;
    let pos = pos as usize;
    if pos.checked_add(cell_width).is_none_or(|end| end > stride) {
        return;
    }

    let sheet_size = header.sheet_size as usize;
    let chars_per_sheet = (header.sheet_column as usize) * (header.sheet_row as usize);
    let sheets = glyph_index / chars_per_sheet;
    let rem = glyph_index % chars_per_sheet;
    let sheet_count = header.sheet_fullsize as usize / sheet_size;
    if sheets >= sheet_count {
        return;
    }

    let xoff = (rem % (header.sheet_column as usize)) * cell_width;
    let yoff = (rem / (header.sheet_column as usize)) * cell_height;
    let img_start = unsafe { font_image.add(sheet_size * sheets) };
    let out = image.cast::<u8>();
    let src_row_bytes = (header.sheet_width as usize / 8) * 32;
    let dst_row_bytes = ((stride << 1) / 8) * 32;

    let mut y = 0usize;
    while y < cell_height {
        let src_row = unsafe { img_start.add(src_row_bytes * ((y + yoff) / 8)) };
        let mut x = 0usize;
        while x < cell_width {
            let src = unsafe {
                src_row
                    .add(((x + xoff) / 8) * 32)
                    .add(((y + yoff) % 8) * 4)
                    .add(((x + xoff) % 8) / 2)
            };
            let dst = unsafe {
                out.add((y / 8) * dst_row_bytes)
                    .add(((x + pos) / 8) * 32)
                    .add(((x + pos) % 8) / 2)
                    .add((y % 8) * 4)
            };
            unsafe {
                *dst = *src;
            }
            x += 2;
        }
        y += 1;
    }

    let glyph_width = unsafe { *font_data.add(header.width_table as usize).add(glyph_index) };
    unsafe {
        *width = i32::from(glyph_width);
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
    FontSnapshot {
        encoding_queried: SYS_FONT_ENCODING_QUERIED.load(Ordering::Acquire),
        init_called: SYS_FONT_INIT_CALLED.load(Ordering::Acquire),
        init_ptr: SYS_FONT_INIT_PTR.load(Ordering::Acquire),
        texture_called: SYS_FONT_TEXTURE_CALLED.load(Ordering::Acquire),
        texture_c: SYS_FONT_TEXTURE_C.load(Ordering::Acquire),
        texel_called: SYS_FONT_TEXEL_CALLED.load(Ordering::Acquire),
        texel_c: SYS_FONT_TEXEL_C.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the protect-range ABI. The unreachable non-PowerPC fallback does not access the pointers.
pub unsafe fn sys_protect_range(chan: u32, addr: *mut c_void, bytes: u32, cntrl: u32) {
    if !wii_target_available() {
        let _ = (chan, addr, bytes, cntrl);
        return;
    }
    SYS_PROTECT_RANGE_CALLED.store(true, Ordering::Release);
    SYS_PROTECT_RANGE_CHAN.store(chan, Ordering::Release);
    SYS_PROTECT_RANGE_ADDR.store(addr as usize, Ordering::Release);
    SYS_PROTECT_RANGE_BYTES.store(bytes, Ordering::Release);
    SYS_PROTECT_RANGE_CNTRL.store(cntrl, Ordering::Release);

    if chan >= SYS_PROTECTCHANMAX {
        return;
    }
    let (start, end) = protect_range_bounds(addr as u32, bytes);
    unsafe {
        cache::dc_flush_range(start as *const c_void, end.wrapping_sub(start));
        let interrupt_state = sram_disable_interrupts();
        irq::unmask(1 << (irq::IRQ_MEM0 + chan));

        const MEM_REG_BASE: usize = 0xCC00_4000;
        let channel = chan as usize;
        write_volatile(
            (MEM_REG_BASE + (channel << 3)) as *mut u16,
            (start >> 10) as u16,
        );
        write_volatile(
            (MEM_REG_BASE + (channel << 3) + 2) as *mut u16,
            (end >> 10) as u16,
        );

        let control = (MEM_REG_BASE + 16) as *mut u16;
        let previous = read_volatile(control);
        let shift = channel << 1;
        let updated = (previous & !(3 << shift)) | (((cntrl & 3) as u16) << shift);
        write_volatile(control, updated);
        if cntrl == SYS_PROTECTRDWR {
            irq::mask(1 << (irq::IRQ_MEM0 + chan));
        }
        sram_restore_interrupts(interrupt_state);
    }
}

fn protect_range_bounds(addr: u32, bytes: u32) -> (u32, u32) {
    let start = addr & !0x3ff;
    let end = addr.wrapping_add(bytes).wrapping_add(0x3ff) & !0x3ff;
    (start, end)
}

/// # Safety
///
/// Matches the performance monitor ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_start_pmc(mcr0val: u32, mcr1val: u32) {
    if !wii_target_available() {
        let _ = (mcr0val, mcr1val);
        return;
    }
    SYS_PMC_STARTED.store(true, Ordering::Release);
    SYS_PMC_DUMPED.store(false, Ordering::Release);
    SYS_PMC_STOPPED.store(false, Ordering::Release);
    SYS_PMC_RESET.store(false, Ordering::Release);
    SYS_PMC_MCR0VAL.store(mcr0val, Ordering::Release);
    SYS_PMC_MCR1VAL.store(mcr1val, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    unsafe {
        asm!(
            "mtspr 952, {mcr0}",
            "mtspr 956, {mcr1}",
            mcr0 = in(reg) mcr0val,
            mcr1 = in(reg) mcr1val,
            options(nostack)
        );
    }
}

/// # Safety
///
/// Matches the performance monitor ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_dump_pmc() {
    if !wii_target_available() {
        return;
    }
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let pmc1: u32;
        let pmc2: u32;
        let pmc3: u32;
        let pmc4: u32;
        asm!("mfspr {value}, 953", value = out(reg) pmc1, options(nostack));
        asm!("mfspr {value}, 954", value = out(reg) pmc2, options(nostack));
        asm!("mfspr {value}, 957", value = out(reg) pmc3, options(nostack));
        asm!("mfspr {value}, 958", value = out(reg) pmc4, options(nostack));
        SYS_PMC1.store(pmc1, Ordering::Release);
        SYS_PMC2.store(pmc2, Ordering::Release);
        SYS_PMC3.store(pmc3, Ordering::Release);
        SYS_PMC4.store(pmc4, Ordering::Release);
    }
    SYS_PMC_DUMPED.store(true, Ordering::Release);
}

/// # Safety
///
/// Matches the performance monitor ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_stop_pmc() {
    if !wii_target_available() {
        return;
    }
    SYS_PMC_STOPPED.store(true, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    unsafe {
        asm!(
            "li {zero}, 0",
            "mtspr 952, {zero}",
            "mtspr 956, {zero}",
            zero = out(reg) _,
            options(nostack)
        );
    }
}

/// # Safety
///
/// Matches the performance monitor ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_reset_pmc() {
    if !wii_target_available() {
        return;
    }
    SYS_PMC_RESET.store(true, Ordering::Release);
    SYS_PMC_STARTED.store(false, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    unsafe {
        asm!(
            "li {zero}, 0",
            "mtspr 953, {zero}",
            "mtspr 954, {zero}",
            "mtspr 957, {zero}",
            "mtspr 958, {zero}",
            zero = out(reg) _,
            options(nostack)
        );
    }
}

/// # Safety
///
/// Matches the alarm ABI. The unreachable non-PowerPC fallback does not dereference `thealarm`.
pub unsafe fn sys_create_alarm(_thealarm: *mut SysWd) -> i32 {
    if !wii_target_available() {
        let _ = _thealarm;
        return -1;
    }
    if _thealarm.is_null() {
        return -1;
    }
    const LWP_ALARM_STORAGE_SIZE: usize = 32;
    unsafe {
        let alarm = allocate_lwp_workspace(LWP_ALARM_STORAGE_SIZE);
        if alarm.is_null() {
            return -1;
        }
        write_bytes(alarm, 0, LWP_ALARM_STORAGE_SIZE);
        SYS_ALARM_CREATE_CALLED.store(true, Ordering::Release);
        let alarm_id = alarm as usize as SysWd;
        SYS_ALARM_LAST_ID.store(alarm_id, Ordering::Release);
        *_thealarm = alarm_id;
        0
    }
}

fn alarm_timespec_to_ticks(timespec: TimeSpec, ticks_per_second: u64) -> u64 {
    (timespec.tv_sec as u64)
        .wrapping_mul(ticks_per_second)
        .wrapping_add((timespec.tv_nsec as u64).wrapping_mul(ticks_per_second) / 1_000_000_000)
}

#[cfg(target_arch = "powerpc")]
unsafe fn alarm_ticks_per_second() -> u64 {
    if unsafe { sys_get_hollywood_revision() } != 0 {
        lwp::TICKS_PER_SECOND
    } else {
        40_500_000
    }
}

/// # Safety
///
/// Matches the alarm ABI. The unreachable non-PowerPC fallback does not dereference `tp` or `cbarg`.
pub unsafe fn sys_set_alarm(
    _thealarm: SysWd,
    _tp: *const TimeSpec,
    _cb: AlarmCallback,
    _cbarg: *mut c_void,
) -> i32 {
    if !wii_target_available() {
        let _ = (_thealarm, _tp, _cb, _cbarg);
        return -1;
    }
    if _thealarm == 0 || _tp.is_null() || _cb.is_none() {
        return -1;
    }
    let ticks_per_second = unsafe { alarm_ticks_per_second() };
    let ticks = alarm_timespec_to_ticks(unsafe { *_tp }, ticks_per_second);
    let interrupt_state = unsafe { sram_disable_interrupts() };
    let result = unsafe { lwp::timer_start_with_key(_thealarm as usize, _cb, _cbarg, ticks, 0) };
    unsafe { sram_restore_interrupts(interrupt_state) };
    SYS_ALARM_CALLBACK.store(_cb.map_or(0, |func| func as usize), Ordering::Release);
    SYS_ALARM_LAST_ID.store(_thealarm, Ordering::Release);
    result
}

/// # Safety
///
/// Matches the alarm ABI. The unreachable non-PowerPC fallback does not dereference `tp_start`, `tp_period`, or `cbarg`.
pub unsafe fn sys_set_periodic_alarm(
    _thealarm: SysWd,
    _tp_start: *const TimeSpec,
    _tp_period: *const TimeSpec,
    _cb: AlarmCallback,
    _cbarg: *mut c_void,
) -> i32 {
    if !wii_target_available() {
        let _ = (_thealarm, _tp_start, _tp_period, _cb, _cbarg);
        return -1;
    }
    if _thealarm == 0 || _tp_start.is_null() || _tp_period.is_null() || _cb.is_none() {
        return -1;
    }
    let ticks_per_second = unsafe { alarm_ticks_per_second() };
    let start = alarm_timespec_to_ticks(unsafe { *_tp_start }, ticks_per_second);
    let period = alarm_timespec_to_ticks(unsafe { *_tp_period }, ticks_per_second);
    let interrupt_state = unsafe { sram_disable_interrupts() };
    let result =
        unsafe { lwp::timer_start_with_key(_thealarm as usize, _cb, _cbarg, start, period) };
    unsafe { sram_restore_interrupts(interrupt_state) };
    SYS_ALARM_PERIODIC_CALLBACK.store(_cb.map_or(0, |func| func as usize), Ordering::Release);
    SYS_ALARM_LAST_ID.store(_thealarm, Ordering::Release);
    result
}

/// # Safety
///
/// Matches the alarm ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_remove_alarm(_thealarm: SysWd) -> i32 {
    if !wii_target_available() {
        let _ = _thealarm;
        return -1;
    }
    if _thealarm == 0 {
        return -1;
    }
    lwp::timer_cancel(_thealarm as usize);
    unsafe { free_lwp_workspace(_thealarm as usize as *mut u8, 32) };
    SYS_ALARM_REMOVE_CALLED.store(true, Ordering::Release);
    SYS_ALARM_LAST_ID.store(_thealarm, Ordering::Release);
    0
}

/// # Safety
///
/// Matches the alarm ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_cancel_alarm(_thealarm: SysWd) -> i32 {
    if !wii_target_available() {
        let _ = _thealarm;
        return -1;
    }
    if _thealarm == 0 {
        return -1;
    }
    lwp::timer_cancel(_thealarm as usize);
    SYS_ALARM_CANCEL_CALLED.store(true, Ordering::Release);
    SYS_ALARM_LAST_ID.store(_thealarm, Ordering::Release);
    0
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct AlarmSnapshot {
    pub created: bool,
    pub removed: bool,
    pub cancelled: bool,
    pub alarm: SysWd,
    pub callback: usize,
    pub periodic_callback: usize,
}

pub fn snapshot_alarms() -> AlarmSnapshot {
    AlarmSnapshot {
        created: SYS_ALARM_CREATE_CALLED.load(Ordering::Acquire),
        removed: SYS_ALARM_REMOVE_CALLED.load(Ordering::Acquire),
        cancelled: SYS_ALARM_CANCEL_CALLED.load(Ordering::Acquire),
        alarm: SYS_ALARM_LAST_ID.load(Ordering::Acquire),
        callback: SYS_ALARM_CALLBACK.load(Ordering::Acquire),
        periodic_callback: SYS_ALARM_PERIODIC_CALLBACK.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the counter-bias ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_counter_bias() -> u32 {
    unsafe {
        with_sram_lock(|sram| {
            SYS_COUNTER_BIAS_GET_CALLED.store(true, Ordering::Release);
            let bias = (*sram).counter_bias;
            SYS_COUNTER_BIAS.store(bias, Ordering::Release);
            bias
        })
    }
    .unwrap_or_else(|| SYS_COUNTER_BIAS.load(Ordering::Acquire))
}

/// # Safety
///
/// Matches the counter-bias ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_counter_bias(bias: u32) {
    unsafe { sys_set_counter_bias_with_writer(bias, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_counter_bias_with_writer(bias: u32, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            SYS_COUNTER_BIAS_SET_CALLED.store(true, Ordering::Release);
            if (*sram).counter_bias != bias {
                (*sram).counter_bias = bias;
                SYS_COUNTER_BIAS.store(bias, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct CounterBiasProbeSnapshot {
    pub get_called: bool,
    pub set_called: bool,
}

pub fn snapshot_counter_bias_probe() -> CounterBiasProbeSnapshot {
    CounterBiasProbeSnapshot {
        get_called: SYS_COUNTER_BIAS_GET_CALLED.load(Ordering::Acquire),
        set_called: SYS_COUNTER_BIAS_SET_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the display-offset ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_display_offset_h() -> i8 {
    unsafe {
        with_sram_lock(|sram| {
            SYS_DISPLAY_OFFSET_H_GET_CALLED.store(true, Ordering::Release);
            let offset = (*sram).display_offset_h;
            SYS_DISPLAY_OFFSET_H.store(offset as u32, Ordering::Release);
            offset
        })
    }
    .unwrap_or_else(|| SYS_DISPLAY_OFFSET_H.load(Ordering::Acquire) as i8)
}

/// # Safety
///
/// Matches the display-offset ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_display_offset_h(offset: i8) {
    unsafe { sys_set_display_offset_h_with_writer(offset, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_display_offset_h_with_writer(offset: i8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            SYS_DISPLAY_OFFSET_H_SET_CALLED.store(true, Ordering::Release);
            if (*sram).display_offset_h != offset {
                (*sram).display_offset_h = offset;
                SYS_DISPLAY_OFFSET_H.store(offset as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayOffsetProbeSnapshot {
    pub get_called: bool,
    pub set_called: bool,
}

pub fn snapshot_display_offset_probe() -> DisplayOffsetProbeSnapshot {
    DisplayOffsetProbeSnapshot {
        get_called: SYS_DISPLAY_OFFSET_H_GET_CALLED.load(Ordering::Acquire),
        set_called: SYS_DISPLAY_OFFSET_H_SET_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the EU RGB60 ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_eu_rgb60() -> u8 {
    unsafe {
        with_sram_lock(|sram| {
            SYS_EU_RGB60_GET_CALLED.store(true, Ordering::Release);
            let enable = ((*sram).ntd >> 6) & 1;
            SYS_EU_RGB60.store(enable as u32, Ordering::Release);
            enable
        })
    }
    .unwrap_or_else(|| SYS_EU_RGB60.load(Ordering::Acquire) as u8)
}

/// # Safety
///
/// Matches the EU RGB60 ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_eu_rgb60(enable: u8) {
    unsafe { sys_set_eu_rgb60_with_writer(enable, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_eu_rgb60_with_writer(enable: u8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            let enable = enable & 1;
            SYS_EU_RGB60_SET_CALLED.store(true, Ordering::Release);
            if (((*sram).ntd >> 6) & 1) != enable {
                (*sram).ntd = ((*sram).ntd & !0x40) | (enable << 6);
                SYS_EU_RGB60.store(enable as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct EuRgb60ProbeSnapshot {
    pub get_called: bool,
    pub set_called: bool,
}

pub fn snapshot_eu_rgb60_probe() -> EuRgb60ProbeSnapshot {
    EuRgb60ProbeSnapshot {
        get_called: SYS_EU_RGB60_GET_CALLED.load(Ordering::Acquire),
        set_called: SYS_EU_RGB60_SET_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the language ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_language() -> u8 {
    unsafe {
        with_sram_lock(|sram| {
            SYS_LANGUAGE_GET_CALLED.store(true, Ordering::Release);
            let lang = (*sram).lang;
            SYS_LANGUAGE.store(lang as u32, Ordering::Release);
            lang
        })
    }
    .unwrap_or_else(|| SYS_LANGUAGE.load(Ordering::Acquire) as u8)
}

/// # Safety
///
/// Matches the language ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_language(lang: u8) {
    unsafe { sys_set_language_with_writer(lang, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_language_with_writer(lang: u8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            SYS_LANGUAGE_SET_CALLED.store(true, Ordering::Release);
            if (*sram).lang != lang {
                (*sram).lang = lang;
                SYS_LANGUAGE.store(lang as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct LanguageProbeSnapshot {
    pub get_called: bool,
    pub set_called: bool,
}

pub fn snapshot_language_probe() -> LanguageProbeSnapshot {
    LanguageProbeSnapshot {
        get_called: SYS_LANGUAGE_GET_CALLED.load(Ordering::Acquire),
        set_called: SYS_LANGUAGE_SET_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the progressive-scan ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_progressive_scan() -> u8 {
    unsafe {
        with_sram_lock(|sram| {
            SYS_PROGRESSIVE_SCAN_GET_CALLED.store(true, Ordering::Release);
            let enable = ((*sram).flags >> 7) & 1;
            SYS_PROGRESSIVE_SCAN.store(enable as u32, Ordering::Release);
            enable
        })
    }
    .unwrap_or_else(|| SYS_PROGRESSIVE_SCAN.load(Ordering::Acquire) as u8)
}

/// # Safety
///
/// Matches the progressive-scan ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_progressive_scan(enable: u8) {
    unsafe { sys_set_progressive_scan_with_writer(enable, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_progressive_scan_with_writer(enable: u8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            let enable = enable & 1;
            SYS_PROGRESSIVE_SCAN_SET_CALLED.store(true, Ordering::Release);
            if (((*sram).flags >> 7) & 1) != enable {
                (*sram).flags = ((*sram).flags & !0x80) | (enable << 7);
                SYS_PROGRESSIVE_SCAN.store(enable as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgressiveScanProbeSnapshot {
    pub get_called: bool,
    pub set_called: bool,
}

pub fn snapshot_progressive_scan_probe() -> ProgressiveScanProbeSnapshot {
    ProgressiveScanProbeSnapshot {
        get_called: SYS_PROGRESSIVE_SCAN_GET_CALLED.load(Ordering::Acquire),
        set_called: SYS_PROGRESSIVE_SCAN_SET_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the sound-mode ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_sound_mode() -> u8 {
    unsafe {
        with_sram_lock(|sram| {
            let mode = ((*sram).flags >> 2) & 1;
            SYS_SOUND_MODE.store(mode as u32, Ordering::Release);
            mode
        })
    }
    .unwrap_or_else(|| SYS_SOUND_MODE.load(Ordering::Acquire) as u8)
}

/// # Safety
///
/// Matches the sound-mode ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_sound_mode(mode: u8) {
    unsafe { sys_set_sound_mode_with_writer(mode, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_sound_mode_with_writer(mode: u8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            let mode = mode & 1;
            if (((*sram).flags >> 2) & 1) != mode {
                (*sram).flags = ((*sram).flags & !0x04) | (mode << 2);
                SYS_SOUND_MODE.store(mode as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

/// # Safety
///
/// Matches the video-mode ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_video_mode() -> u8 {
    unsafe {
        with_sram_lock(|sram| {
            let mode = (*sram).flags & 3;
            SYS_VIDEO_MODE.store(mode as u32, Ordering::Release);
            mode
        })
    }
    .unwrap_or_else(|| SYS_VIDEO_MODE.load(Ordering::Acquire) as u8)
}

/// # Safety
///
/// Matches the video-mode ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_video_mode(mode: u8) {
    unsafe { sys_set_video_mode_with_writer(mode, sram_sanitize_and_write_powerpc) };
}

unsafe fn sys_set_video_mode_with_writer(mode: u8, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_lock(|sram| {
            let mode = mode & 3;
            if ((*sram).flags & 3) != mode {
                (*sram).flags = ((*sram).flags & !0x03) | mode;
                SYS_VIDEO_MODE.store(mode as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

/// # Safety
///
/// Matches the wireless-ID ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_wireless_id(chan: u32) -> u16 {
    let index = match chan {
        0..=3 => chan as usize,
        _ => return 0,
    };
    // BUG FIX: C reads wireless IDs under the extended SRAM lock. Without this
    // lock, SI callbacks can read the SRAM image during a concurrent update.
    let sram_ex = unsafe { _sys_lock_sram_ex() };
    if sram_ex.is_null() {
        return 0;
    }
    unsafe {
        let id0 = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).wireless_pad_id[0]));
        let id1 = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).wireless_pad_id[1]));
        let id2 = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).wireless_pad_id[2]));
        let id3 = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).wireless_pad_id[3]));
        SYS_WIRELESS_ID0.store(id0 as u32, Ordering::Release);
        SYS_WIRELESS_ID1.store(id1 as u32, Ordering::Release);
        SYS_WIRELESS_ID2.store(id2 as u32, Ordering::Release);
        SYS_WIRELESS_ID3.store(id3 as u32, Ordering::Release);
        let id = [id0, id1, id2, id3][index];
        _sys_unlock_sram_ex(0);
        id
    }
}

unsafe extern "C" fn si_wireless_id_getter(chan: u32) -> u16 {
    // SAFETY: SI validates the channel and calls this only after SYS SRAM init.
    unsafe { sys_get_wireless_id(chan) }
}

/// # Safety
///
/// Matches the wireless-ID ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_wireless_id(chan: u32, id: u16) {
    unsafe { sys_set_wireless_id_with_writer(chan, id, sram_write_extended_powerpc) };
}

unsafe extern "C" fn si_wireless_id_setter(chan: u32, id: u16) {
    // SAFETY: SI validates the channel and calls this only after SYS SRAM init.
    unsafe { sys_set_wireless_id(chan, id) }
}

unsafe fn sys_set_wireless_id_with_writer(chan: u32, id: u16, write_sram: unsafe fn() -> bool) {
    let index = match chan {
        0..=3 => chan as usize,
        _ => return,
    };
    // BUG FIX: C updates wireless IDs under the extended SRAM lock. Keep the
    // comparison, SRAM write, and persisted shadow update inside that lock.
    let sram_ex = unsafe { _sys_lock_sram_ex() };
    if sram_ex.is_null() {
        return;
    }
    unsafe {
        let slot = core::ptr::addr_of_mut!((*sram_ex).wireless_pad_id[index]);
        let snapshot = [
            &SYS_WIRELESS_ID0,
            &SYS_WIRELESS_ID1,
            &SYS_WIRELESS_ID2,
            &SYS_WIRELESS_ID3,
        ][index];
        let previous = core::ptr::read_unaligned(slot);
        if previous != id {
            core::ptr::write_unaligned(slot, id);
            snapshot.store(id as u32, Ordering::Release);
            let persisted = write_sram();
            SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
        }
        _sys_unlock_sram_ex(0);
    }
}

/// # Safety
///
/// Matches the GBS ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_get_gbs_mode() -> u16 {
    unsafe {
        with_sram_ex_lock(|sram_ex| {
            let mode = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).gbs));
            SYS_GBS_MODE.store(mode as u32, Ordering::Release);
            mode
        })
    }
    .unwrap_or_else(|| SYS_GBS_MODE.load(Ordering::Acquire) as u16)
}

/// # Safety
///
/// Matches the GBS ABI. The unreachable non-PowerPC fallback only updates local state.
pub unsafe fn sys_set_gbs_mode(mode: u16) {
    unsafe { sys_set_gbs_mode_with_writer(mode, sram_write_extended_powerpc) };
}

unsafe fn sys_set_gbs_mode_with_writer(mode: u16, write_sram: unsafe fn() -> bool) {
    let _ = unsafe {
        with_sram_ex_lock(|sram_ex| {
            let mode = {
                if ((mode >> 10) & 0x1f) >= 20
                    || ((mode >> 6) & 0x03) == 0x03
                    || (mode & 0x3f) >= 60
                {
                    0
                } else {
                    mode
                }
            };
            let previous = core::ptr::read_unaligned(core::ptr::addr_of!((*sram_ex).gbs));
            if previous != mode {
                core::ptr::write_unaligned(core::ptr::addr_of_mut!((*sram_ex).gbs), mode);
                SYS_GBS_MODE.store(mode as u32, Ordering::Release);
                let persisted = write_sram();
                SYS_SRAM_SYNC_RESULT.store(persisted, Ordering::Release);
            }
        })
    };
}

/// # Safety
///
/// Matches the reset-callback ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_register_reset_func(info: *mut SysResetInfo) {
    SYS_RESET_REGISTER_CALLED.store(true, Ordering::Release);
    SYS_RESET_REGISTER_PTR.store(info as usize, Ordering::Release);
    if info.is_null() {
        return;
    }
    let _guard = ResetQueueGuard::acquire();
    unsafe {
        let mut registered = SYS_RESET_QUEUE_HEAD.load(Ordering::Acquire) as *mut SysResetInfo;
        while !registered.is_null() {
            if registered == info {
                return;
            }
            registered = (*registered).node.next.cast::<SysResetInfo>();
        }
        (*info).node.next = null_mut();
        (*info).node.prev = null_mut();
        let mut previous: *mut SysResetInfo = null_mut();
        let mut current = SYS_RESET_QUEUE_HEAD.load(Ordering::Acquire) as *mut SysResetInfo;
        while !current.is_null() && (*current).prio <= (*info).prio {
            previous = current;
            current = (*current).node.next.cast::<SysResetInfo>();
        }

        (*info).node.prev = previous.cast::<LwpNode>();
        (*info).node.next = current.cast::<LwpNode>();
        if previous.is_null() {
            SYS_RESET_QUEUE_HEAD.store(info as usize, Ordering::Release);
        } else {
            (*previous).node.next = info.cast::<LwpNode>();
        }
        if !current.is_null() {
            (*current).node.prev = info.cast::<LwpNode>();
        }
        reset_debug(SYS_RESET_TRACE_REGISTER, (*info).prio, 1);
    }
}

/// # Safety
///
/// Matches the reset-callback ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_unregister_reset_func(info: *mut SysResetInfo) {
    SYS_RESET_UNREGISTER_CALLED.store(true, Ordering::Release);
    SYS_RESET_UNREGISTER_PTR.store(info as usize, Ordering::Release);
    if info.is_null() {
        return;
    }
    let _guard = ResetQueueGuard::acquire();
    unsafe {
        let mut current = SYS_RESET_QUEUE_HEAD.load(Ordering::Acquire) as *mut SysResetInfo;
        while !current.is_null() && current != info {
            current = (*current).node.next.cast::<SysResetInfo>();
        }
        if current.is_null() {
            return;
        }
        let previous = (*current).node.prev.cast::<SysResetInfo>();
        let next = (*current).node.next.cast::<SysResetInfo>();
        if previous.is_null() {
            SYS_RESET_QUEUE_HEAD.store(next as usize, Ordering::Release);
        } else {
            (*previous).node.next = next.cast::<LwpNode>();
        }
        if !next.is_null() {
            (*next).node.prev = previous.cast::<LwpNode>();
        }
        (*current).node.next = null_mut();
        (*current).node.prev = null_mut();
        reset_debug(SYS_RESET_TRACE_UNREGISTER, (*current).prio, 1);
    }
}

/// Runs one pass of the registered production reset-function queue.
///
/// This is also the diagnostic entry point used by the reset probe. A zero
/// return means that at least one callback or SRAM write is not ready.
pub fn sys_run_reset_functions(final_: i32) -> u32 {
    let mut ready = true;
    const RESET_SNAPSHOT_CAPACITY: usize = 64;
    let mut callbacks = [(0u32, None); RESET_SNAPSHOT_CAPACITY];
    let mut count = 0usize;
    {
        let _guard = ResetQueueGuard::acquire();
        let mut current = SYS_RESET_QUEUE_HEAD.load(Ordering::Acquire) as *mut SysResetInfo;
        while !current.is_null() && count < callbacks.len() {
            callbacks[count] = unsafe { ((*current).prio, (*current).func) };
            count += 1;
            current = unsafe { (*current).node.next.cast::<SysResetInfo>() };
        }
        if !current.is_null() {
            ready = false;
        }
    }
    #[cfg(target_arch = "powerpc")]
    let mut wpad_reset_callback_ran = false;
    for &(priority, function) in &callbacks[..count] {
        #[cfg(target_arch = "powerpc")]
        if final_ == 0 && !wpad_reset_callback_ran && priority > WPAD_RESET_PRIORITY {
            if wpad::get_status() != wpad::WPAD_STATE_DISABLED {
                let _shutdown_result = wpad::shutdown();
            }
            wpad_reset_callback_ran = true;
        }
        reset_debug(SYS_RESET_TRACE_CALLBACK_BEGIN, priority, final_);
        let result = function.map_or(1, |callback| unsafe { callback(final_) });
        reset_debug(SYS_RESET_TRACE_CALLBACK_END, priority, result);
        if result == 0 {
            ready = false;
        }
    }
    #[cfg(target_arch = "powerpc")]
    if final_ == 0 && !wpad_reset_callback_ran && wpad::get_status() != wpad::WPAD_STATE_DISABLED {
        let _shutdown_result = wpad::shutdown();
    }
    let sram_ready = unsafe { _sys_sync_sram() } != 0;
    reset_debug(SYS_RESET_TRACE_SRAM, 0, sram_ready as i32);
    (ready && sram_ready) as u32
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
    ResetRegistrationSnapshot {
        register_called: SYS_RESET_REGISTER_CALLED.load(Ordering::Acquire),
        unregister_called: SYS_RESET_UNREGISTER_CALLED.load(Ordering::Acquire),
        register_ptr: SYS_RESET_REGISTER_PTR.load(Ordering::Acquire),
        unregister_ptr: SYS_RESET_UNREGISTER_PTR.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the reset-callback ABI. The unreachable non-PowerPC fallback preserves the callback for later return.
pub unsafe fn sys_set_reset_callback(cb: ResetCallback) -> ResetCallback {
    let old = SYS_RESET_CALLBACK.swap(cb.map_or(0, |func| func as usize), Ordering::AcqRel);
    if old == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(u32, *mut c_void)>(old) })
    }
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback does not dereference pointers.
pub unsafe fn sys_get_arena1_lo() -> *mut c_void {
    SYS_ARENA1_LO.load(Ordering::Acquire) as *mut c_void
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback does not dereference pointers.
pub unsafe fn sys_set_arena1_lo(new_lo: *mut c_void) {
    SYS_ARENA1_LO.store(new_lo as usize, Ordering::Release);
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback does not dereference pointers.
pub unsafe fn sys_get_arena1_hi() -> *mut c_void {
    SYS_ARENA1_HI.load(Ordering::Acquire) as *mut c_void
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback does not dereference pointers.
pub unsafe fn sys_set_arena1_hi(new_hi: *mut c_void) {
    SYS_ARENA1_HI.store(new_hi as usize, Ordering::Release);
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback returns a neutral size.
pub unsafe fn sys_get_arena1_size() -> u32 {
    let lo = SYS_ARENA1_LO.load(Ordering::Acquire);
    let hi = SYS_ARENA1_HI.load(Ordering::Acquire);
    arena_size(lo, hi)
}

fn arena_size(lo: usize, hi: usize) -> u32 {
    (hi as u32).wrapping_sub(lo as u32)
}

fn checked_arena_low_allocation(
    lo: usize,
    hi: usize,
    size: usize,
    align: usize,
) -> Option<(usize, usize)> {
    if size == 0 || align == 0 || !align.is_power_of_two() {
        return None;
    }
    let mask = align - 1;
    let ptr = lo.checked_add(mask)? & !mask;
    let next_lo = ptr.checked_add(size)?.checked_add(mask)? & !mask;
    (next_lo <= hi).then_some((ptr, next_lo))
}

/// # Safety
///
/// Matches the MEM1 arena ABI. The unreachable non-PowerPC fallback does not allocate.
pub unsafe fn sys_alloc_arena1_mem_lo(_size: u32, _align: u32) -> *mut c_void {
    let lo = SYS_ARENA1_LO.load(Ordering::Acquire);
    let hi = SYS_ARENA1_HI.load(Ordering::Acquire);
    let Some((ptr, next_lo)) =
        checked_arena_low_allocation(lo, hi, _size as usize, _align as usize)
    else {
        return core::ptr::null_mut();
    };
    SYS_ARENA1_LO.store(next_lo, Ordering::Release);
    ptr as *mut c_void
}

/// # Safety
///
/// Matches the reset-button ABI. The unreachable non-PowerPC fallback reports no button press.
pub unsafe fn sys_reset_button_down() -> u32 {
    unsafe {
        const PI_RESET_BUTTON_REG: usize = PI_REG_BASE;
        u32::from((core::ptr::read_volatile(PI_RESET_BUTTON_REG as *const u32) & 0x0001_0000) == 0)
    }
}

/// # Safety
///
/// Matches libogc `SYS_GetHollywoodRevision`.
///
/// On Wii/RVL, the Hollywood revision is published in low memory at
/// `0x80003138`. Non-PowerPC runtime builds are rejected at crate root.
pub unsafe fn sys_get_hollywood_revision() -> u32 {
    SYS_HOLLYWOOD_REVISION_CALLED.store(true, Ordering::Release);
    unsafe { ogc_sys::sys_get_hollywood_revision() }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct HardwareProbeSnapshot {
    pub reset_button_called: bool,
    pub hollywood_revision_called: bool,
}

pub fn snapshot_hardware_probe() -> HardwareProbeSnapshot {
    HardwareProbeSnapshot {
        reset_button_called: SYS_RESET_BUTTON_DOWN_CALLED.load(Ordering::Acquire),
        hollywood_revision_called: SYS_HOLLYWOOD_REVISION_CALLED.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_get_arena2_lo() -> *mut c_void {
    let interrupt_state = irq::irq_disable();
    let address = SYS_ARENA2_LO.load(Ordering::Acquire);
    irq::irq_restore(interrupt_state);
    address as *mut c_void
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_set_arena2_lo(new_lo: *mut c_void) {
    publish_arena2_boundary(
        new_lo,
        irq::irq_disable,
        |address| SYS_ARENA2_LO.store(address, Ordering::Release),
        irq::irq_restore,
    );
}

#[cfg(not(gamecube))]
fn publish_arena2_boundary(
    new_lo: *mut c_void,
    disable_interrupts: impl FnOnce() -> u32,
    set_rust_boundary: impl FnOnce(usize),
    restore_interrupts: impl FnOnce(u32),
) {
    let interrupt_state = disable_interrupts();
    set_rust_boundary(new_lo as usize);
    restore_interrupts(interrupt_state);
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_get_arena2_hi() -> *mut c_void {
    let interrupt_state = irq::irq_disable();
    let address = SYS_ARENA2_HI.load(Ordering::Acquire);
    irq::irq_restore(interrupt_state);
    address as *mut c_void
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_set_arena2_hi(new_hi: *mut c_void) {
    publish_arena2_boundary(
        new_hi,
        irq::irq_disable,
        |address| SYS_ARENA2_HI.store(address, Ordering::Release),
        irq::irq_restore,
    );
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_get_arena2_size() -> u32 {
    let interrupt_state = irq::irq_disable();
    let lo = SYS_ARENA2_LO.load(Ordering::Acquire);
    let hi = SYS_ARENA2_HI.load(Ordering::Acquire);
    let size = arena_size(lo, hi);
    irq::irq_restore(interrupt_state);
    size
}

/// # Safety
///
/// Matches the Wii-only MEM2 arena API.
#[cfg(not(gamecube))]
pub unsafe fn sys_alloc_arena2_mem_lo(size: u32, align: u32) -> *mut c_void {
    let lo = unsafe { sys_get_arena2_lo() } as usize;
    let hi = unsafe { sys_get_arena2_hi() } as usize;
    let Some((ptr, next_lo)) = checked_arena_low_allocation(lo, hi, size as usize, align as usize)
    else {
        return core::ptr::null_mut();
    };
    unsafe { sys_set_arena2_lo(next_lo as *mut c_void) };
    ptr as *mut c_void
}

/// C ABI implementation for libogc/newlib callers. It reads the same boundary
/// as the Rust system API, so `_sbrk_r` cannot allocate through a Rust heap.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_GetArena2Lo")]
pub extern "C" fn abi_sys_get_arena2_lo() -> *mut c_void {
    unsafe { sys_get_arena2_lo() }
}

/// C ABI implementation for libogc/newlib callers.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_SetArena2Lo")]
/// # Safety
/// The new arena boundary must preserve all live allocations and stay within valid MEM2 bounds. Serialize arena changes.
pub unsafe extern "C" fn abi_sys_set_arena2_lo(new_lo: *mut c_void) {
    unsafe { sys_set_arena2_lo(new_lo) };
}

/// C ABI implementation for libogc/newlib callers.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_GetArena2Hi")]
pub extern "C" fn abi_sys_get_arena2_hi() -> *mut c_void {
    unsafe { sys_get_arena2_hi() }
}

/// C ABI implementation for libogc/newlib callers.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_SetArena2Hi")]
/// # Safety
/// The new arena boundary must preserve all live allocations and stay within valid MEM2 bounds. Serialize arena changes.
pub unsafe extern "C" fn abi_sys_set_arena2_hi(new_hi: *mut c_void) {
    unsafe { sys_set_arena2_hi(new_hi) };
}

/// C ABI implementation for libogc/newlib callers.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_GetArena2Size")]
pub extern "C" fn abi_sys_get_arena2_size() -> u32 {
    unsafe { sys_get_arena2_size() }
}

/// C ABI implementation for libogc/newlib callers.
#[cfg(not(gamecube))]
#[unsafe(export_name = "SYS_AllocArena2MemLo")]
/// # Safety
/// Serialize arena allocation with all users of the arena. The returned memory must not outlive its arena reservation.
pub unsafe extern "C" fn abi_sys_alloc_arena2_mem_lo(size: u32, align: u32) -> *mut c_void {
    unsafe { sys_alloc_arena2_mem_lo(size, align) }
}

// Keep the C header spellings available to Rust callers while the exported
// implementation identifiers follow Rust naming conventions.
#[cfg(not(gamecube))]
pub use abi_sys_alloc_arena2_mem_lo as SYS_AllocArena2MemLo;
#[cfg(not(gamecube))]
pub use abi_sys_get_arena2_hi as SYS_GetArena2Hi;
#[cfg(not(gamecube))]
pub use abi_sys_get_arena2_lo as SYS_GetArena2Lo;
#[cfg(not(gamecube))]
pub use abi_sys_get_arena2_size as SYS_GetArena2Size;
#[cfg(not(gamecube))]
pub use abi_sys_set_arena2_hi as SYS_SetArena2Hi;
#[cfg(not(gamecube))]
pub use abi_sys_set_arena2_lo as SYS_SetArena2Lo;

/// # Safety
///
/// Matches the power-callback ABI. The unreachable non-PowerPC fallback returns the incoming callback.
pub unsafe fn sys_set_power_callback(cb: PowerCallback) -> PowerCallback {
    let old = SYS_POWER_CALLBACK.swap(cb.map_or(0, |func| func as usize), Ordering::AcqRel);
    if old == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn()>(old) })
    }
}

/// # Safety
///
/// Matches the power-callback ABI. The unreachable non-PowerPC fallback records no state.
pub unsafe fn sys_do_power_cb() {
    let ptr = SYS_POWER_CALLBACK.load(Ordering::Acquire);
    SYS_POWER_DISPATCH_CALLED.store(true, Ordering::Release);
    if cfg!(target_arch = "powerpc") {
        let now = read_timebase() as u32;
        let enable_time = SYS_POWER_ENABLE_TIME.load(Ordering::Acquire);
        let due = power_callback_due(now, enable_time);
        SYS_POWER_ENABLE_TIME.store(
            now.wrapping_add(sys_millisecs_to_ticks(POWER_CALLBACK_COOLDOWN_MS)),
            Ordering::Release,
        );
        if !due {
            SYS_POWER_DISPATCH_RESULT.store(false, Ordering::Release);
            return;
        }
    }
    if ptr != 0 {
        let cb = unsafe { core::mem::transmute::<usize, unsafe extern "C" fn()>(ptr) };
        unsafe {
            cb();
        }
        SYS_POWER_DISPATCH_RESULT.store(true, Ordering::Release);
    } else {
        SYS_POWER_DISPATCH_RESULT.store(false, Ordering::Release);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PowerDispatchSnapshot {
    pub called: bool,
    pub result: bool,
}

pub fn snapshot_power_dispatch() -> PowerDispatchSnapshot {
    PowerDispatchSnapshot {
        called: SYS_POWER_DISPATCH_CALLED.load(Ordering::Acquire),
        result: SYS_POWER_DISPATCH_RESULT.load(Ordering::Acquire),
    }
}

/// # Safety
///
/// Matches the time ABI. The unreachable non-PowerPC fallback returns a monotonically increasing value.
pub unsafe fn sys_time() -> u64 {
    SYS_TIME_CALLED.store(true, Ordering::Release);
    read_system_time()
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TimeProbeSnapshot {
    pub called: bool,
}

pub fn snapshot_time_probe() -> TimeProbeSnapshot {
    TimeProbeSnapshot {
        called: SYS_TIME_CALLED.load(Ordering::Acquire),
    }
}

const MSR_EXTERNAL_INTERRUPT: u32 = 1 << 15;
const MSR_FLOATING_POINT: u32 = 1 << 13;

fn sram_locked_msr(msr: u32) -> (u32, u32) {
    (
        msr & !(MSR_EXTERNAL_INTERRUPT | MSR_FLOATING_POINT),
        ogc_sys::msr_lock_state(msr),
    )
}

fn sram_unlocked_msr(msr: u32, interrupt_state: u32) -> u32 {
    ogc_sys::msr_unlock_value(msr, interrupt_state)
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_disable_interrupts() -> u32 {
    let msr: u32;
    unsafe {
        asm!("mfmsr {msr}", msr = out(reg) msr, options(nostack));
        let (disabled_msr, interrupt_state) = sram_locked_msr(msr);
        asm!("mtmsr {msr}", msr = in(reg) disabled_msr, options(nostack));
        interrupt_state
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn sram_restore_interrupts(interrupt_state: u32) {
    unsafe {
        let msr: u32;
        asm!("mfmsr {msr}", msr = out(reg) msr, options(nostack));
        let msr = sram_unlocked_msr(msr, interrupt_state);
        asm!("mtmsr {msr}", msr = in(reg) msr, options(nostack));
    }
}

/// # Safety
///
/// Matches the SRAM lock ABI. The unreachable non-PowerPC fallback does not provide shared SRAM.
pub unsafe fn _sys_lock_sram() -> *mut SysSram {
    let interrupt_state = unsafe { sram_disable_interrupts() };
    if SYS_SRAM_BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        unsafe { sram_restore_interrupts(interrupt_state) };
        return core::ptr::null_mut();
    }
    SYS_SRAM_INTERRUPT_STATE.store(interrupt_state, Ordering::Release);
    SYS_SRAM_LOCKED.store(true, Ordering::Release);
    SYS_SRAM_EX_LOCKED.store(false, Ordering::Release);
    core::ptr::addr_of_mut!(SYS_SRAM)
}

/// # Safety
///
/// Matches the extended SRAM lock ABI. The unreachable non-PowerPC fallback does not provide shared SRAM.
pub unsafe fn _sys_lock_sram_ex() -> *mut SysSramEx {
    let interrupt_state = unsafe { sram_disable_interrupts() };
    if SYS_SRAM_BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        unsafe { sram_restore_interrupts(interrupt_state) };
        return core::ptr::null_mut();
    }
    SYS_SRAM_INTERRUPT_STATE.store(interrupt_state, Ordering::Release);
    SYS_SRAM_EX_LOCKED.store(true, Ordering::Release);
    SYS_SRAM_LOCKED.store(false, Ordering::Release);
    core::ptr::addr_of_mut!(SYS_SRAM_EX)
}

/// # Safety
///
/// Matches the SRAM unlock ABI. The unreachable non-PowerPC fallback performs no hardware writeback.
pub unsafe fn _sys_unlock_sram(_write: u32) -> u32 {
    unsafe {
        let was_locked = SYS_SRAM_LOCKED.load(Ordering::Acquire);
        let mut sync_ok = SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire);
        if was_locked && _write != 0 {
            sync_ok = sram_sanitize_and_write_powerpc();
            SYS_SRAM_SYNC_RESULT.store(sync_ok, Ordering::Release);
        }
        SYS_SRAM_SYNC_CALLED.store(true, Ordering::Release);
        if was_locked {
            SYS_SRAM_LOCKED.store(false, Ordering::Release);
            SYS_SRAM_BUSY.store(false, Ordering::Release);
            SYS_SRAM_EX_LOCKED.store(false, Ordering::Release);
            sram_restore_interrupts(SYS_SRAM_INTERRUPT_STATE.load(Ordering::Acquire));
        }
        if was_locked { sync_ok as u32 } else { 0 }
    }
}

/// # Safety
///
/// Matches the extended SRAM unlock ABI. The unreachable non-PowerPC fallback performs no hardware writeback.
pub unsafe fn _sys_unlock_sram_ex(_write: u32) -> u32 {
    unsafe {
        let was_locked = SYS_SRAM_EX_LOCKED.load(Ordering::Acquire);
        let mut sync_ok = SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire);
        if was_locked && _write != 0 {
            sync_ok = sram_write_extended_powerpc();
            SYS_SRAM_SYNC_RESULT.store(sync_ok, Ordering::Release);
        }
        SYS_SRAM_SYNC_CALLED.store(true, Ordering::Release);
        if was_locked {
            SYS_SRAM_EX_LOCKED.store(false, Ordering::Release);
            SYS_SRAM_BUSY.store(false, Ordering::Release);
            SYS_SRAM_LOCKED.store(false, Ordering::Release);
            sram_restore_interrupts(SYS_SRAM_INTERRUPT_STATE.load(Ordering::Acquire));
        }
        if was_locked { sync_ok as u32 } else { 0 }
    }
}

// BUG FIX: Keep settings access serialized with the C SRAM lock protocol.
unsafe fn with_sram_lock<R>(access: impl FnOnce(*mut SysSram) -> R) -> Option<R> {
    let sram = unsafe { _sys_lock_sram() };
    if sram.is_null() {
        return None;
    }
    let result = access(sram);
    unsafe { _sys_unlock_sram(0) };
    Some(result)
}

unsafe fn with_sram_ex_lock<R>(access: impl FnOnce(*mut SysSramEx) -> R) -> Option<R> {
    let sram = unsafe { _sys_lock_sram_ex() };
    if sram.is_null() {
        return None;
    }
    let result = access(sram);
    unsafe { _sys_unlock_sram_ex(0) };
    Some(result)
}

/// # Safety
///
/// Matches the SRAM sync ABI. The unreachable non-PowerPC fallback performs no hardware sync.
pub unsafe fn _sys_sync_sram() -> u32 {
    SYS_SRAM_SYNC_CALLED.store(true, Ordering::Release);
    SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire) as u32
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
    ProtectRangeSnapshot {
        called: SYS_PROTECT_RANGE_CALLED.load(Ordering::Acquire),
        chan: SYS_PROTECT_RANGE_CHAN.load(Ordering::Acquire),
        addr: SYS_PROTECT_RANGE_ADDR.load(Ordering::Acquire) as *mut c_void,
        bytes: SYS_PROTECT_RANGE_BYTES.load(Ordering::Acquire),
        cntrl: SYS_PROTECT_RANGE_CNTRL.load(Ordering::Acquire),
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
    pub pmc1: u32,
    pub pmc2: u32,
    pub pmc3: u32,
    pub pmc4: u32,
}

pub fn snapshot_pmc() -> PmcSnapshot {
    PmcSnapshot {
        started: SYS_PMC_STARTED.load(Ordering::Acquire),
        dumped: SYS_PMC_DUMPED.load(Ordering::Acquire),
        stopped: SYS_PMC_STOPPED.load(Ordering::Acquire),
        reset: SYS_PMC_RESET.load(Ordering::Acquire),
        mcr0val: SYS_PMC_MCR0VAL.load(Ordering::Acquire),
        mcr1val: SYS_PMC_MCR1VAL.load(Ordering::Acquire),
        pmc1: SYS_PMC1.load(Ordering::Acquire),
        pmc2: SYS_PMC2.load(Ordering::Acquire),
        pmc3: SYS_PMC3.load(Ordering::Acquire),
        pmc4: SYS_PMC4.load(Ordering::Acquire),
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
    RuntimeCallSnapshot {
        init_called: SYS_RUNTIME_INIT_CALLED.load(Ordering::Acquire),
        pre_main_called: SYS_RUNTIME_PRE_MAIN_CALLED.load(Ordering::Acquire),
        main_loop_called: SYS_RUNTIME_MAIN_LOOP_CALLED.load(Ordering::Acquire),
        main_loop_result: SYS_RUNTIME_MAIN_LOOP_RESULT.load(Ordering::Acquire),
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
    PreMainSnapshot {
        called: SYS_RUNTIME_PRE_MAIN_CALLED.load(Ordering::Acquire),
        completed: SYS_RUNTIME_PRE_MAIN_COMPLETED.load(Ordering::Acquire),
        failed_stage: SYS_RUNTIME_PRE_MAIN_FAILED_STAGE.load(Ordering::Acquire),
        result: SYS_RUNTIME_PRE_MAIN_RESULT.load(Ordering::Acquire),
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
    StartupSnapshot {
        init_called: SYS_RUNTIME_INIT_CALLED.load(Ordering::Acquire),
        low_mem_init_called: SYS_STARTUP_LOW_MEM_INIT_CALLED.load(Ordering::Acquire),
        init_started: SYS_RUNTIME_INIT_STARTED.load(Ordering::Acquire),
        init_completed: SYS_RUNTIME_INIT_COMPLETED.load(Ordering::Acquire),
        init_stage: SYS_RUNTIME_INIT_STAGE.load(Ordering::Acquire),
        init_completed_mask: SYS_RUNTIME_INIT_COMPLETED_MASK.load(Ordering::Acquire),
        init_failed_stage: SYS_RUNTIME_INIT_FAILED_STAGE.load(Ordering::Acquire),
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};

    const FONT_TEST_BYTES: usize = SYS_FONTSIZE_SJIS as usize;

    #[test]
    fn system_power_device_opens_with_c_libogc_mode() {
        let mut observed = None;
        let result = open_stm_immediate_with(|path, mode| {
            observed = Some((path.to_bytes(), mode));
            Ok(17)
        });
        assert_eq!(result, Ok(17));
        assert_eq!(observed, Some((b"/dev/stm/immediate".as_slice(), 0u32)));
    }

    #[test]
    fn system_power_device_open_error_is_preserved() {
        let result = open_stm_immediate_with::<()>(|_, mode| {
            assert_eq!(mode, 0);
            Err(-6)
        });
        assert_eq!(result, Err(-6));
    }

    #[test]
    fn sram_lock_restores_only_the_saved_external_interrupt_bit() {
        let (locked_msr, interrupt_state) = sram_locked_msr(0xA000_0001);
        assert_eq!(locked_msr, 0x0000_0001);
        assert_eq!(interrupt_state, MSR_EXTERNAL_INTERRUPT | MSR_FLOATING_POINT);
        assert_eq!(sram_unlocked_msr(locked_msr, interrupt_state), 0xA000_0001);
    }

    #[test]
    fn main_loop_uses_platform_terminal_state_and_stays_stopped() {
        assert!(!main_loop_should_continue(true, true, 0, false));
        assert!(!main_loop_should_continue(false, true, 1, false));
        assert!(main_loop_should_continue(false, true, 0, true));
        assert!(!main_loop_should_continue(false, false, 0, true));
        assert!(main_loop_should_continue(false, false, 1, false));
    }

    #[test]
    fn protect_range_bounds_match_c_alignment_and_wrapping() {
        assert_eq!(protect_range_bounds(0x1234, 1), (0x1000, 0x1400));
        assert_eq!(protect_range_bounds(0x1000, 0), (0x1000, 0x1000));
        assert_eq!(
            protect_range_bounds(0xffff_ff00, 0x200),
            (0xffff_fc00, 0x400)
        );
    }

    #[test]
    fn extended_sram_write_preserves_an_earlier_pending_base_offset() {
        let extended_offset = size_of::<SysSram>();
        assert_eq!(
            pending_sram_write_offset(64, extended_offset),
            extended_offset
        );
        assert_eq!(pending_sram_write_offset(0, extended_offset), 0);
        assert_eq!(SRAM_BUFFER_BYTES - extended_offset, size_of::<SysSramEx>());
    }

    #[test]
    fn loader_handoff_preserves_translation_and_sets_c_shutdown_bits() {
        // Independent MSR values: C __irq_shutdown clears EE and sets FP/RI.
        // IR/DR are preserved, not used as substitutes for FP/RI.
        assert_eq!(reset_shutdown_msr(0x0000_8000), 0x0000_2002);
        assert_eq!(reset_shutdown_msr(0x0000_8030), 0x0000_2032);
        assert_eq!(reset_shutdown_msr(0x0000_2032), 0x0000_2032);
        assert_eq!(reset_shutdown_msr(0xffff_ffff), 0xffff_7fff);
    }

    #[test]
    fn reset_cleanup_matches_c_interrupt_and_low_memory_values() {
        assert_eq!(reset_shutdown_msr(0x8000_0040), 0x8000_2042);
        assert_eq!(
            RESET_LOWMEM_REGIONS,
            [
                (0x8000_0040, 140),
                (0x8000_00d4, 20),
                (0x8000_00f4, 4),
                (0x8000_3000, 192),
                (0x8000_30c8, 12),
                (0x8000_30e2, 1),
            ]
        );
    }

    #[test]
    fn clearing_wpad_active_devices_preserves_registered_device_data() {
        let mut pads = conf::ConfPads::default();
        let bytes = &mut pads as *mut conf::ConfPads as *mut u8;
        unsafe {
            core::ptr::write_bytes(bytes, 0x5a, size_of::<conf::ConfPads>());
        }

        clear_wpad_active_devices(&mut pads);

        let pads_bytes = unsafe {
            core::slice::from_raw_parts(
                (&pads as *const conf::ConfPads).cast::<u8>(),
                size_of::<conf::ConfPads>(),
            )
        };
        let active_offset = offset_of!(conf::ConfPads, active);
        let active_len = size_of::<[conf::ConfPadDevice; conf::CONF_PAD_MAX_ACTIVE]>();
        assert!(pads_bytes[..active_offset].iter().all(|byte| *byte == 0x5a));
        assert!(
            pads_bytes[active_offset..active_offset + active_len]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert!(
            pads_bytes[active_offset + active_len..]
                .iter()
                .all(|byte| *byte == 0x5a)
        );
    }

    #[repr(align(32))]
    struct FontBuffer([u8; FONT_TEST_BYTES]);

    static mut FONT_BUFFER: FontBuffer = FontBuffer([0; FONT_TEST_BYTES]);
    static FIRED: AtomicBool = AtomicBool::new(false);
    static RESET_TEST_ORDER: AtomicU32 = AtomicU32::new(0);
    static PRE_MAIN_TEST_CALLS: AtomicU32 = AtomicU32::new(0);
    static PRE_MAIN_TEST_ORDER: AtomicU32 = AtomicU32::new(0);
    static PRE_MAIN_TEST_RESULTS: [AtomicI32; 8] = [const { AtomicI32::new(0) }; 8];

    unsafe fn power_cb() {
        FIRED.store(true, Ordering::Release);
    }

    unsafe fn reset_cb(_irq: u32, _ctx: *mut c_void) {}
    unsafe fn diet_cb(_buf: *const c_char, _size: usize) {}
    unsafe fn sram_read_succeeds() -> bool {
        true
    }
    unsafe fn sram_read_fails() -> bool {
        false
    }
    unsafe fn sram_write_succeeds() -> bool {
        true
    }
    unsafe fn sram_write_fails() -> bool {
        false
    }
    unsafe fn sram_write_observes_lock() -> bool {
        SYS_SRAM_BUSY.load(Ordering::Acquire)
    }
    fn pre_main_test_result(stage: usize) -> i32 {
        PRE_MAIN_TEST_CALLS.fetch_or(1 << stage, Ordering::AcqRel);
        PRE_MAIN_TEST_ORDER
            .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                Some(value.saturating_mul(10).saturating_add(stage as u32))
            })
            .expect("pre-main test order update cannot fail");
        PRE_MAIN_TEST_RESULTS[stage].load(Ordering::Acquire)
    }
    fn pre_main_load_ios() -> i32 {
        pre_main_test_result(1)
    }
    fn pre_main_initialize_ios() -> i32 {
        pre_main_test_result(2)
    }
    fn pre_main_register_handler(handler: Option<unsafe fn(u32)>) -> Option<unsafe fn(u32)> {
        PRE_MAIN_TEST_CALLS.fetch_or(1 << 3, Ordering::AcqRel);
        PRE_MAIN_TEST_ORDER
            .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                Some(value.saturating_mul(10).saturating_add(3))
            })
            .expect("pre-main test order update cannot fail");
        stm_register_event_handler_local(handler)
    }
    fn pre_main_initialize_stm() -> i32 {
        pre_main_test_result(4)
    }
    fn pre_main_initialize_conf() -> i32 {
        pre_main_test_result(5)
    }
    fn pre_main_initialize_wiilaunch() -> i32 {
        pre_main_test_result(6)
    }
    fn pre_main_test_operations() -> PreMainOperations {
        PreMainOperations {
            load_startup_ios: pre_main_load_ios,
            initialize_ios: pre_main_initialize_ios,
            register_stm_handler: pre_main_register_handler,
            initialize_stm: pre_main_initialize_stm,
            initialize_conf: pre_main_initialize_conf,
            initialize_wiilaunch: pre_main_initialize_wiilaunch,
        }
    }
    fn reset_pre_main_test_state() {
        PRE_MAIN_TEST_CALLS.store(0, Ordering::Release);
        PRE_MAIN_TEST_ORDER.store(0, Ordering::Release);
        for result in &PRE_MAIN_TEST_RESULTS {
            result.store(0, Ordering::Release);
        }
        PRE_MAIN_TEST_RESULTS[4].store(1, Ordering::Release);
        let _previous_handler = stm_register_event_handler_local(None);
    }
    unsafe fn reset_order_one(_final: i32) -> i32 {
        let _ = RESET_TEST_ORDER.try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            Some(value.saturating_mul(10).saturating_add(1))
        });
        1
    }
    unsafe fn reset_order_two(_final: i32) -> i32 {
        let _ = RESET_TEST_ORDER.try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            Some(value.saturating_mul(10).saturating_add(2))
        });
        1
    }

    #[test]
    fn power_callback_round_trips_and_dispatches() {
        let old = sys_set_power_callback(Some(power_cb));
        assert!(old.is_none());
        sys_do_power_cb();
        assert!(FIRED.load(Ordering::Acquire));
        assert_eq!(
            snapshot_power_dispatch(),
            PowerDispatchSnapshot {
                called: true,
                result: true,
            }
        );
        let old = sys_set_power_callback(None);
        assert!(old.is_some());
    }

    #[test]
    fn reset_callback_round_trips() {
        let old = sys_set_reset_callback(Some(reset_cb));
        assert!(old.is_none());
        let old = sys_set_reset_callback(None);
        assert!(old.is_some());
    }

    #[test]
    fn reset_registration_snapshot_tracks_host_calls() {
        let mut info: SysResetInfo = unsafe { core::mem::zeroed() };
        unsafe {
            sys_register_reset_func(&mut info);
            sys_unregister_reset_func(&mut info);
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
    fn registered_reset_queue_runs_in_priority_order() {
        let mut later = SysResetInfo {
            node: LwpNode {
                next: null_mut(),
                prev: null_mut(),
            },
            func: Some(reset_order_one),
            prio: 20,
        };
        let mut earlier = SysResetInfo {
            node: LwpNode {
                next: null_mut(),
                prev: null_mut(),
            },
            func: Some(reset_order_two),
            prio: 10,
        };
        RESET_TEST_ORDER.store(0, Ordering::Release);
        SYS_SRAM_SYNC_RESULT.store(true, Ordering::Release);
        unsafe {
            sys_register_reset_func(&mut later);
            sys_register_reset_func(&mut earlier);
        }
        assert_eq!(sys_run_reset_functions(0), 1);
        assert_eq!(RESET_TEST_ORDER.load(Ordering::Acquire), 21);
        unsafe {
            sys_unregister_reset_func(&mut earlier);
            sys_unregister_reset_func(&mut later);
        }
    }

    #[test]
    fn reset_system_state_is_recorded() {
        sys_reset_system(2, 0x1234_5678, 1);
        assert!(SYS_RESET_SYSTEM_CALLED.load(Ordering::Acquire));
        assert_eq!(SYS_RESET_SYSTEM_RESET.load(Ordering::Acquire), 2);
        assert_eq!(SYS_RESET_SYSTEM_CODE.load(Ordering::Acquire), 0x1234_5678);
        assert!(SYS_RESET_SYSTEM_FORCE_MENU.load(Ordering::Acquire));
        assert_eq!(SYS_RESET_SYSTEM_TERMINAL_HINT.load(Ordering::Acquire), 3);
    }

    #[test]
    fn watchdog_restart_uses_the_system_restart_route() {
        unsafe { sys_reset_system(SYS_RESTART as i32, 0, 0) };
        assert_eq!(
            snapshot_reset_system(),
            ResetSystemSnapshot {
                called: true,
                reset: SYS_RESTART,
                reset_code: 0,
                force_menu: false,
                terminal_hint: 1,
            }
        );
    }

    #[test]
    fn runtime_entry_snapshot_tracks_host_calls() {
        unsafe {
            sys_init();
            sys_pre_main();
            let result = sys_main_loop();
            assert_eq!(result, 0);
        }
        assert_eq!(
            snapshot_runtime_calls(),
            RuntimeCallSnapshot {
                init_called: true,
                pre_main_called: true,
                main_loop_called: true,
                main_loop_result: false,
            }
        );
        let startup = snapshot_startup();
        assert!(startup.init_called);
        assert!(startup.low_mem_init_called);
    }

    #[test]
    fn pre_main_completes_all_operations_in_order() {
        reset_pre_main_test_state();

        assert_eq!(sys_pre_main_with_operations(&pre_main_test_operations()), 0);
        assert_eq!(PRE_MAIN_TEST_CALLS.load(Ordering::Acquire), 0x7e);
        assert_eq!(PRE_MAIN_TEST_ORDER.load(Ordering::Acquire), 123_456);
        assert_eq!(
            snapshot_pre_main(),
            PreMainSnapshot {
                called: true,
                completed: true,
                failed_stage: 0,
                result: 0,
            }
        );

        let _previous_handler = stm_register_event_handler_local(None);
    }

    #[test]
    fn pre_main_continues_all_operations_after_initialization_errors() {
        for (stage, failure) in [(1usize, -11), (2, -12), (4, 0), (5, -15), (6, -16)] {
            reset_pre_main_test_state();
            PRE_MAIN_TEST_RESULTS[stage].store(failure, Ordering::Release);

            let expected = if stage == 4 {
                SYS_PRE_MAIN_ESTM
            } else {
                failure
            };
            assert_eq!(
                sys_pre_main_with_operations(&pre_main_test_operations()),
                expected
            );
            assert_eq!(PRE_MAIN_TEST_CALLS.load(Ordering::Acquire), 0x7e);
            assert_eq!(PRE_MAIN_TEST_ORDER.load(Ordering::Acquire), 123_456);
            assert_eq!(
                snapshot_pre_main(),
                PreMainSnapshot {
                    called: true,
                    completed: false,
                    failed_stage: stage as u32,
                    result: expected,
                }
            );
            assert_ne!(SYS_STM_EVENT_CALLBACK.load(Ordering::Acquire), 0);
        }
    }

    #[test]
    fn sys_init_records_successful_sram_read() {
        SYS_SRAM_SYNC_RESULT.store(false, Ordering::Release);

        assert!(unsafe { sys_init_with_sram_read(sram_read_succeeds) });
        assert_eq!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire), true);
    }

    #[test]
    fn sys_init_stops_and_records_failed_sram_read() {
        SYS_SRAM_SYNC_RESULT.store(true, Ordering::Release);

        assert!(!unsafe { sys_init_with_sram_read(sram_read_fails) });
        assert_eq!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire), false);
    }

    #[test]
    fn counter_bias_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.counter_bias };
        unsafe {
            SYS_SRAM.counter_bias = 10;
            SYS_COUNTER_BIAS.store(10, Ordering::Release);
            sys_set_counter_bias_with_writer(20, sram_write_succeeds);
            let bias = SYS_SRAM.counter_bias;
            assert_eq!(bias, 20);
        }
        assert_eq!(SYS_COUNTER_BIAS.load(Ordering::Acquire), 20);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.counter_bias = 10;
            SYS_COUNTER_BIAS.store(10, Ordering::Release);
            sys_set_counter_bias_with_writer(20, sram_write_fails);
            let bias = SYS_SRAM.counter_bias;
            assert_eq!(bias, 20);
        }
        assert_eq!(SYS_COUNTER_BIAS.load(Ordering::Acquire), 20);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.counter_bias = previous;
        }
        SYS_COUNTER_BIAS.store(previous, Ordering::Release);
    }

    #[test]
    fn counter_bias_write_runs_while_sram_is_locked() {
        let previous = unsafe { SYS_SRAM.counter_bias };
        unsafe {
            SYS_SRAM.counter_bias = 10;
            SYS_COUNTER_BIAS.store(10, Ordering::Release);
            sys_set_counter_bias_with_writer(20, sram_write_observes_lock);
            assert_eq!(SYS_SRAM.counter_bias, 20);
            SYS_SRAM.counter_bias = previous;
        }
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));
        assert!(!SYS_SRAM_BUSY.load(Ordering::Acquire));
        SYS_COUNTER_BIAS.store(previous, Ordering::Release);
    }

    #[test]
    fn display_offset_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.display_offset_h };
        unsafe {
            SYS_SRAM.display_offset_h = 10;
            SYS_DISPLAY_OFFSET_H.store(10, Ordering::Release);
            sys_set_display_offset_h_with_writer(20, sram_write_succeeds);
            let offset = SYS_SRAM.display_offset_h;
            assert_eq!(offset, 20);
        }
        assert_eq!(SYS_DISPLAY_OFFSET_H.load(Ordering::Acquire), 20);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.display_offset_h = 10;
            SYS_DISPLAY_OFFSET_H.store(10, Ordering::Release);
            sys_set_display_offset_h_with_writer(20, sram_write_fails);
            let offset = SYS_SRAM.display_offset_h;
            assert_eq!(offset, 20);
        }
        assert_eq!(SYS_DISPLAY_OFFSET_H.load(Ordering::Acquire), 20);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.display_offset_h = previous;
        }
        SYS_DISPLAY_OFFSET_H.store(previous as u32, Ordering::Release);
    }

    #[test]
    fn eu_rgb60_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.ntd };
        unsafe {
            SYS_SRAM.ntd = 0x15;
            SYS_EU_RGB60.store(0, Ordering::Release);
            sys_set_eu_rgb60_with_writer(1, sram_write_succeeds);
            let ntd = SYS_SRAM.ntd;
            assert_eq!(ntd, 0x55);
        }
        assert_eq!(SYS_EU_RGB60.load(Ordering::Acquire), 1);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.ntd = 0x15;
            SYS_EU_RGB60.store(0, Ordering::Release);
            sys_set_eu_rgb60_with_writer(1, sram_write_fails);
            let ntd = SYS_SRAM.ntd;
            assert_eq!(ntd, 0x55);
        }
        assert_eq!(SYS_EU_RGB60.load(Ordering::Acquire), 1);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.ntd = previous;
        }
        SYS_EU_RGB60.store(((previous >> 6) & 1) as u32, Ordering::Release);
    }

    #[test]
    fn language_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.lang };
        unsafe {
            SYS_SRAM.lang = 1;
            SYS_LANGUAGE.store(1, Ordering::Release);
            sys_set_language_with_writer(2, sram_write_succeeds);
            let lang = SYS_SRAM.lang;
            assert_eq!(lang, 2);
        }
        assert_eq!(SYS_LANGUAGE.load(Ordering::Acquire), 2);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.lang = 1;
            SYS_LANGUAGE.store(1, Ordering::Release);
            sys_set_language_with_writer(2, sram_write_fails);
            let lang = SYS_SRAM.lang;
            assert_eq!(lang, 2);
        }
        assert_eq!(SYS_LANGUAGE.load(Ordering::Acquire), 2);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.lang = previous;
        }
        SYS_LANGUAGE.store(previous as u32, Ordering::Release);
    }

    #[test]
    fn progressive_scan_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.flags };
        unsafe {
            SYS_SRAM.flags = 0x15;
            SYS_PROGRESSIVE_SCAN.store(0, Ordering::Release);
            sys_set_progressive_scan_with_writer(1, sram_write_succeeds);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x95);
        }
        assert_eq!(SYS_PROGRESSIVE_SCAN.load(Ordering::Acquire), 1);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = 0x15;
            SYS_PROGRESSIVE_SCAN.store(0, Ordering::Release);
            sys_set_progressive_scan_with_writer(1, sram_write_fails);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x95);
        }
        assert_eq!(SYS_PROGRESSIVE_SCAN.load(Ordering::Acquire), 1);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = previous;
        }
        SYS_PROGRESSIVE_SCAN.store(((previous >> 7) & 1) as u32, Ordering::Release);
    }

    #[test]
    fn sound_mode_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.flags };
        unsafe {
            SYS_SRAM.flags = 0x11;
            SYS_SOUND_MODE.store(0, Ordering::Release);
            sys_set_sound_mode_with_writer(1, sram_write_succeeds);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x15);
        }
        assert_eq!(SYS_SOUND_MODE.load(Ordering::Acquire), 1);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = 0x11;
            SYS_SOUND_MODE.store(0, Ordering::Release);
            sys_set_sound_mode_with_writer(1, sram_write_fails);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x15);
        }
        assert_eq!(SYS_SOUND_MODE.load(Ordering::Acquire), 1);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = previous;
        }
        SYS_SOUND_MODE.store(((previous >> 2) & 1) as u32, Ordering::Release);
    }

    #[test]
    fn video_mode_write_failure_keeps_shadow_value() {
        let previous = unsafe { SYS_SRAM.flags };
        unsafe {
            SYS_SRAM.flags = 0x14;
            SYS_VIDEO_MODE.store(0, Ordering::Release);
            sys_set_video_mode_with_writer(1, sram_write_succeeds);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x15);
        }
        assert_eq!(SYS_VIDEO_MODE.load(Ordering::Acquire), 1);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = 0x14;
            SYS_VIDEO_MODE.store(0, Ordering::Release);
            sys_set_video_mode_with_writer(1, sram_write_fails);
            let flags = SYS_SRAM.flags;
            assert_eq!(flags, 0x15);
        }
        assert_eq!(SYS_VIDEO_MODE.load(Ordering::Acquire), 1);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            SYS_SRAM.flags = previous;
        }
        SYS_VIDEO_MODE.store((previous & 3) as u32, Ordering::Release);
    }

    #[test]
    fn gbs_mode_write_failure_keeps_shadow_value() {
        let previous = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.gbs)) };
        unsafe {
            core::ptr::write_unaligned(core::ptr::addr_of_mut!(SYS_SRAM_EX.gbs), 0x0001);
            SYS_GBS_MODE.store(0x0001, Ordering::Release);
            sys_set_gbs_mode_with_writer(0x0402, sram_write_succeeds);
            let mode = core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.gbs));
            assert_eq!(mode, 0x0402);
        }
        assert_eq!(SYS_GBS_MODE.load(Ordering::Acquire), 0x0402);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            core::ptr::write_unaligned(core::ptr::addr_of_mut!(SYS_SRAM_EX.gbs), 0x0001);
            SYS_GBS_MODE.store(0x0001, Ordering::Release);
            sys_set_gbs_mode_with_writer(0x0402, sram_write_fails);
            let mode = core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.gbs));
            assert_eq!(mode, 0x0402);
        }
        assert_eq!(SYS_GBS_MODE.load(Ordering::Acquire), 0x0402);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            core::ptr::write_unaligned(core::ptr::addr_of_mut!(SYS_SRAM_EX.gbs), previous);
        }
        SYS_GBS_MODE.store(previous as u32, Ordering::Release);
    }

    #[test]
    fn wireless_id_write_failure_keeps_shadow_value() {
        let previous = unsafe {
            core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[2]))
        };
        unsafe {
            core::ptr::write_unaligned(
                core::ptr::addr_of_mut!(SYS_SRAM_EX.wireless_pad_id[2]),
                0x1234,
            );
            SYS_WIRELESS_ID2.store(0x1234, Ordering::Release);
            sys_set_wireless_id_with_writer(2, 0x5678, sram_write_succeeds);
            let id = core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[2]));
            assert_eq!(id, 0x5678);
        }
        assert!(!SYS_SRAM_BUSY.load(Ordering::Acquire));
        assert!(!SYS_SRAM_EX_LOCKED.load(Ordering::Acquire));
        assert_eq!(SYS_WIRELESS_ID2.load(Ordering::Acquire), 0x5678);
        assert!(SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            core::ptr::write_unaligned(
                core::ptr::addr_of_mut!(SYS_SRAM_EX.wireless_pad_id[2]),
                0x1234,
            );
            SYS_WIRELESS_ID2.store(0x1234, Ordering::Release);
            sys_set_wireless_id_with_writer(2, 0x5678, sram_write_fails);
            let id = core::ptr::read_unaligned(core::ptr::addr_of!(SYS_SRAM_EX.wireless_pad_id[2]));
            assert_eq!(id, 0x5678);
        }
        assert!(!SYS_SRAM_BUSY.load(Ordering::Acquire));
        assert!(!SYS_SRAM_EX_LOCKED.load(Ordering::Acquire));
        assert_eq!(SYS_WIRELESS_ID2.load(Ordering::Acquire), 0x5678);
        assert!(!SYS_SRAM_SYNC_RESULT.load(Ordering::Acquire));

        unsafe {
            core::ptr::write_unaligned(
                core::ptr::addr_of_mut!(SYS_SRAM_EX.wireless_pad_id[2]),
                previous,
            );
        }
        SYS_WIRELESS_ID2.store(previous as u32, Ordering::Release);
    }

    #[test]
    fn framebuffer_snapshot_tracks_host_calls() {
        let mut rmode = GxrModeObj {
            vi_tv_mode: 0,
            fb_width: 17,
            efb_height: 0,
            xfb_height: 3,
            vi_x_origin: 0,
            vi_y_origin: 0,
            vi_width: 0,
            vi_height: 0,
            xfb_mode: 0,
            field_rendering: 0,
            aa: 1,
            sample_pattern: [[0; 2]; 12],
            vfilter: [0; 7],
        };
        let value = sys_allocate_framebuffer(core::ptr::addr_of_mut!(rmode));
        assert!(value.is_null());
        assert_eq!(
            snapshot_framebuffer(),
            FramebufferSnapshot {
                called: true,
                rmode: core::ptr::addr_of_mut!(rmode) as usize,
                bytes: 448,
            }
        );
    }

    #[test]
    fn font_queries_fail_closed_without_initialization() {
        let _guard = test_guard();
        assert_eq!(sys_init_font(core::ptr::null_mut()), 0);
        let mut image: *mut c_void = core::ptr::null_mut();
        let mut xpos = -1;
        let mut ypos = -1;
        let mut width = -1;
        sys_get_font_texture(7, &mut image, &mut xpos, &mut ypos, &mut width);
        assert!(image.is_null());
        assert_eq!((xpos, ypos, width), (0, 0, 0));

        let mut texel = [0xffu8; 128];
        let mut texel_width = -1;
        sys_get_font_texel(7, texel.as_mut_ptr().cast(), 0, 32, &mut texel_width);
        assert_eq!(texel_width, 0);
        assert!(texel.iter().all(|&byte| byte == 0xff));
    }

    #[test]
    fn font_snapshot_tracks_host_calls() {
        let _guard = test_guard();
        unsafe {
            let encoding = sys_get_font_encoding();
            assert!(encoding <= 1);
            let font_ptr = core::ptr::addr_of_mut!(FONT_BUFFER.0).cast::<SysFontHeader>();
            let init_rc = sys_init_font(font_ptr);
            assert!(init_rc <= 1);
            let mut image: *mut c_void = core::ptr::null_mut();
            let mut xpos = 0;
            let mut ypos = 0;
            let mut width = 0;
            sys_get_font_texture(7, &mut image, &mut xpos, &mut ypos, &mut width);
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
            sys_get_font_texel(9, texel.as_mut_ptr().cast(), 0, 64, &mut texel_width);
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
                init_ptr: core::ptr::addr_of_mut!(FONT_BUFFER.0) as usize,
                texture_called: true,
                texture_c: 7,
                texel_called: true,
                texel_c: 9,
            }
        );
    }

    #[test]
    fn alarm_timespec_conversion_matches_wii_and_gamecube_clocks() {
        let timespec = TimeSpec {
            tv_sec: 1,
            tv_nsec: 250_000_000,
        };
        assert_eq!(alarm_timespec_to_ticks(timespec, 60_750_000), 75_937_500);
        assert_eq!(alarm_timespec_to_ticks(timespec, 40_500_000), 50_625_000);
    }

    #[test]
    fn diet_print_global_is_host_owned() {
        unsafe {
            DIET_PRINT_FN = Some(diet_cb);
        }
        let value = unsafe { core::ptr::read(core::ptr::addr_of!(DIET_PRINT_FN)) };
        assert!(value.is_some());
        unsafe {
            DIET_PRINT_FN = None;
        }
    }

    #[test]
    fn check_argv_relocates_and_builds_pointers() {
        let mut argv_buf = [0u8; 128];
        let command_line = b"app\0first\0second\0\0";
        argv_buf[..command_line.len()].copy_from_slice(command_line);

        let mut argv = SystemArgv {
            argv_magic: 0x5f61_7267,
            command_line: argv_buf.as_mut_ptr().cast(),
            length: command_line.len() as i32,
            argc: 0,
            argv: core::ptr::null_mut(),
            end_argv: core::ptr::null_mut(),
        };

        let old_argv = unsafe { SYSTEM_ARGV };
        let old_arena = sys_get_arena1_lo();

        unsafe {
            SYSTEM_ARGV = core::ptr::addr_of_mut!(argv);
            sys_set_arena1_lo(argv_buf.as_mut_ptr().cast());
            _check_argv();
        }

        assert_eq!(argv.argc, 3);
        assert!(!argv.argv.is_null());
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr((*argv.argv).cast()).to_bytes() },
            b"app"
        );
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr((*argv.argv.add(1)).cast()).to_bytes() },
            b"first"
        );
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr((*argv.argv.add(2)).cast()).to_bytes() },
            b"second"
        );
        assert!(unsafe { (*argv.argv.add(3)).is_null() });
        assert!(!unsafe { ARGV_ARENA1_LO }.is_null());

        unsafe {
            SYSTEM_ARGV = old_argv;
            sys_set_arena1_lo(old_arena);
        }
    }

    #[test]
    #[cfg(not(gamecube))]
    fn arena_size_tracks_host_bounds() {
        unsafe {
            sys_set_arena1_lo(0x1000 as *mut c_void);
            sys_set_arena1_hi(0x1800 as *mut c_void);
            sys_set_arena2_lo(0x2000 as *mut c_void);
            sys_set_arena2_hi(0x2800 as *mut c_void);
        }
        assert_eq!(sys_get_arena1_size(), 0x800);
        assert_eq!(sys_get_arena2_size(), 0x800);
        assert_eq!(arena_size(0x1800, 0x1000), 0xffff_f800);
    }

    #[test]
    #[cfg(not(gamecube))]
    fn arena2_boundary_publication_is_irq_guarded_and_ordered() {
        use core::cell::Cell;

        let phase = Cell::new(0);
        let boundary = 0x9000_1000usize as *mut c_void;
        publish_arena2_boundary(
            boundary,
            || {
                assert_eq!(phase.get(), 0);
                phase.set(1);
                0x20
            },
            |published_boundary| {
                assert_eq!(phase.get(), 1);
                assert_eq!(published_boundary as usize, boundary as usize);
                phase.set(2);
            },
            |interrupt_state| {
                assert_eq!(phase.get(), 2);
                assert_eq!(interrupt_state, 0x20);
                phase.set(3);
            },
        );
        assert_eq!(phase.get(), 3);
    }

    #[test]
    #[cfg(not(gamecube))]
    fn arena2_c_abi_uses_the_rust_runtime_boundary() {
        let original_lo = SYS_GetArena2Lo();
        let original_hi = SYS_GetArena2Hi();
        unsafe {
            SYS_SetArena2Lo(0x2000 as *mut c_void);
            SYS_SetArena2Hi(0x2800 as *mut c_void);
        }

        assert_eq!(SYS_GetArena2Lo() as usize, 0x2000);
        assert_eq!(SYS_GetArena2Hi() as usize, 0x2800);
        assert_eq!(unsafe { sys_get_arena2_lo() } as usize, 0x2000);
        assert_eq!(unsafe { sys_get_arena2_hi() } as usize, 0x2800);
        assert_eq!(SYS_GetArena2Size(), 0x800);

        let allocation = unsafe { SYS_AllocArena2MemLo(0x100, 0x40) };
        assert_eq!(allocation as usize, 0x2000);
        assert_eq!(SYS_GetArena2Lo() as usize, 0x2100);
        assert_eq!(unsafe { sys_get_arena2_lo() } as usize, 0x2100);

        unsafe {
            SYS_SetArena2Lo(original_lo);
            SYS_SetArena2Hi(original_hi);
        }
    }

    #[test]
    fn protect_range_snapshot_tracks_last_call() {
        unsafe {
            sys_protect_range(
                SYS_PROTECTCHAN1,
                0x1234 as *mut c_void,
                0x40,
                SYS_PROTECTRDWR,
            );
        }
        assert_eq!(
            snapshot_protect_range(),
            ProtectRangeSnapshot {
                called: true,
                chan: SYS_PROTECTCHAN1,
                addr: 0x1234 as *mut c_void,
                bytes: 0x40,
                cntrl: SYS_PROTECTRDWR,
            }
        );
    }

    #[test]
    fn pmc_snapshot_tracks_lifecycle() {
        unsafe {
            sys_start_pmc(0xAA55_AA55, 0x55AA_55AA);
            sys_dump_pmc();
            sys_stop_pmc();
            sys_reset_pmc();
        }
        let snapshot = snapshot_pmc();
        assert!(!snapshot.started);
        assert!(snapshot.dumped);
        assert!(snapshot.stopped);
        assert!(snapshot.reset);
        assert_eq!(snapshot.mcr0val, 0xAA55_AA55);
        assert_eq!(snapshot.mcr1val, 0x55AA_55AA);
    }

    #[test]
    fn sram_locks_round_trip_and_sync_records_activity() {
        let sram = _sys_lock_sram();
        assert!(!sram.is_null());
        let unlock = _sys_unlock_sram(1);
        assert_eq!(unlock, 1);

        let sram_ex = _sys_lock_sram_ex();
        assert!(!sram_ex.is_null());
        let unlock_ex = _sys_unlock_sram_ex(1);
        assert_eq!(unlock_ex, 1);

        let sync = _sys_sync_sram();
        assert_eq!(sync, 0);
        assert!(SYS_SRAM_SYNC_CALLED.load(Ordering::Acquire));
    }

    #[test]
    fn rtc_requires_two_matching_reads() {
        let mut reads = [Some(120), Some(120)].into_iter();
        assert_eq!(
            rtc::consistent(|| reads.next().flatten().ok_or(ClockError::RtcUnstable)),
            Ok(120)
        );

        let mut reads = [Some(120), Some(121), Some(121), Some(121)].into_iter();
        assert_eq!(
            rtc::consistent(|| reads.next().flatten().ok_or(ClockError::RtcUnstable)),
            Ok(121)
        );

        let mut reads = [Some(120), None].into_iter();
        assert_eq!(
            rtc::consistent(|| reads.next().flatten().ok_or(ClockError::RtcUnstable)),
            Err(ClockError::RtcUnstable)
        );
    }

    #[test]
    fn system_time_adds_the_boot_offset_with_unsigned_arithmetic() {
        assert_eq!(system_time_value(100, 20), 120);
        assert_eq!(system_time_value(u64::MAX, 1), 0);
    }
}
