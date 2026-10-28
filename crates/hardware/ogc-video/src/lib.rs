#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::ptr::{copy_nonoverlapping, null_mut};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

#[cfg(not(target_arch = "powerpc"))]
extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
use alloc::alloc::{Layout, alloc_zeroed, dealloc};

pub use ogc_video_sys as sys;
pub use sys::{
    Bool, GxrModeObj, PlatformClock, ViPositionCallback, ViRetraceCallback, VideoRegisterSnapshot,
    VideoStandard, get_current_framebuffer, get_next_framebuffer, retrace_count,
    retrace_period_ticks, ticks_to_microseconds, timer_clock_hz, video_standard_for_mode,
};

include!("console_font.rs");

mod xfb_color;

const VI_DISPLAY_PIX_SZ: usize = 2;
const FONT_XSIZE: usize = 8;
const FONT_YSIZE: usize = 16;
const SYS_BASE_CACHED: usize = 0x8000_0000;
const SYS_BASE_UNCACHED: usize = 0xC000_0000;
#[cfg(target_arch = "powerpc")]
const FRAMEBUFFER_POOL_BYTES: usize = 2 * 1024 * 1024;

const CONSOLE_COLOR_BOLD: u32 = 1 << 0;
const CONSOLE_COLOR_FAINT: u32 = 1 << 1;
const CONSOLE_ITALIC: u32 = 1 << 2;
const CONSOLE_UNDERLINE: u32 = 1 << 3;
const CONSOLE_BLINK_SLOW: u32 = 1 << 4;
const CONSOLE_BLINK_FAST: u32 = 1 << 5;
const CONSOLE_COLOR_REVERSE: u32 = 1 << 6;
const CONSOLE_CONCEAL: u32 = 1 << 7;
const CONSOLE_CROSSED_OUT: u32 = 1 << 8;
const CONSOLE_FG_CUSTOM: u32 = 1 << 9;
const CONSOLE_BG_CUSTOM: u32 = 1 << 10;
const CONSOLE_COLOR_FG_BRIGHT: u32 = 1 << 11;
const CONSOLE_COLOR_BG_BRIGHT: u32 = 1 << 12;
const CONSOLE_COLOR_BLACK: u32 = 0;
const CONSOLE_COLOR_WHITE: u32 = 7;
const ANSI_MAX_ARGS: usize = 16;

static DO_XFB_COPY: AtomicBool = AtomicBool::new(false);
static VIDEO_BLACK: AtomicBool = AtomicBool::new(false);
static LAST_CURRENT_TV_MODE: AtomicU32 = AtomicU32::new(0);
static LAST_VIDEO_SCAN_MODE: AtomicU32 = AtomicU32::new(0);
static LAST_HAVE_COMPONENT_CABLE: AtomicU32 = AtomicU32::new(0);
static LAST_CURRENT_LINE: AtomicU32 = AtomicU32::new(0);
static LAST_NEXT_FIELD: AtomicU32 = AtomicU32::new(0);
static LAST_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static LAST_NEXT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static LAST_NEXT_RIGHT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static PRE_RETRACE_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
static POST_RETRACE_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(target_arch = "powerpc"))]
static HOST_VSYNC_COUNT: AtomicU32 = AtomicU32::new(0);
static CONSOLE_BUFFER_SIZE: AtomicUsize = AtomicUsize::new(0);
static CONSOLE_VIDEO_INITIALIZED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct FramebufferPool([u8; FRAMEBUFFER_POOL_BYTES]);

#[cfg(target_arch = "powerpc")]
static mut FRAMEBUFFER_POOL: FramebufferPool = FramebufferPool([0; FRAMEBUFFER_POOL_BYTES]);
#[cfg(target_arch = "powerpc")]
static FRAMEBUFFER_POOL_NEXT: AtomicUsize = AtomicUsize::new(0);
static mut CONSOLE_BUFFER: *mut u8 = null_mut();
#[cfg(target_arch = "powerpc")]
static CONSOLE_BUFFER_POOL_START: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "powerpc")]
static CONSOLE_BUFFER_POOL_END: AtomicUsize = AtomicUsize::new(0);
static mut DEFAULT_CONSOLE: PrintConsole = PrintConsole::new();
static mut CURRENT_CONSOLE_PTR: *mut PrintConsole = core::ptr::addr_of_mut!(DEFAULT_CONSOLE);
static mut CURRENT_COPY: PrintConsole = PrintConsole::new();
static mut ESCAPE_SEQ: EscapeSeq = EscapeSeq::new();

fn begin_console_video_init(initialized: &AtomicBool) -> bool {
    initialized
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ConsoleFont {
    pub gfx: *mut u8,
    pub ascii_offset: u16,
    pub num_chars: u16,
}

pub type ConsolePrint = Option<unsafe extern "C" fn(con: *mut c_void, c: i32) -> bool>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PrintConsole {
    pub font: ConsoleFont,
    pub destbuffer: *mut c_void,
    pub con_xres: i32,
    pub con_yres: i32,
    pub con_stride: i32,
    pub target_x: i32,
    pub target_y: i32,
    pub tgt_stride: i32,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub prev_cursor_x: i32,
    pub prev_cursor_y: i32,
    pub con_cols: i32,
    pub con_rows: i32,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub tab_size: i32,
    pub fg: u32,
    pub bg: u32,
    pub flags: u32,
    pub print_char: ConsolePrint,
    pub console_initialised: bool,
}

impl PrintConsole {
    const fn new() -> Self {
        Self {
            font: ConsoleFont {
                gfx: core::ptr::null_mut(),
                ascii_offset: 0,
                num_chars: 256,
            },
            destbuffer: core::ptr::null_mut(),
            con_xres: 0,
            con_yres: 0,
            con_stride: 0,
            target_x: 0,
            target_y: 0,
            tgt_stride: 0,
            cursor_x: 0,
            cursor_y: 0,
            prev_cursor_x: 0,
            prev_cursor_y: 0,
            con_cols: 80,
            con_rows: 30,
            window_x: 0,
            window_y: 0,
            window_width: 80,
            window_height: 30,
            tab_size: 3,
            fg: CONSOLE_COLOR_WHITE,
            bg: CONSOLE_COLOR_BLACK,
            flags: 0,
            print_char: None,
            console_initialised: false,
        }
    }
}

#[repr(i32)]
#[derive(Copy, Clone)]
pub enum DebugDevice {
    Null = 0,
    Exi = 1,
    Console = 2,
}

#[derive(Copy, Clone)]
struct EscapeColor {
    flags: u32,
    fg: u32,
    bg: u32,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum EscapeState {
    None,
    Start,
    BuildingUnknown,
    BuildingFormatFg,
    BuildingFormatBg,
    BuildingFormatFgNonRgb,
    BuildingFormatBgNonRgb,
    BuildingFormatFgRgb,
    BuildingFormatBgRgb,
}

#[derive(Copy, Clone)]
struct EscapeSeq {
    color: EscapeColor,
    arg_idx: usize,
    args: [i32; ANSI_MAX_ARGS],
    color_arg_count: usize,
    color_args: [u32; 3],
    has_arg: bool,
    state: EscapeState,
}

impl EscapeSeq {
    const fn new() -> Self {
        Self {
            color: EscapeColor {
                flags: 0,
                fg: CONSOLE_COLOR_WHITE,
                bg: CONSOLE_COLOR_BLACK,
            },
            arg_idx: 0,
            args: [0; ANSI_MAX_ARGS],
            color_arg_count: 0,
            color_args: [0; 3],
            has_arg: false,
            state: EscapeState::None,
        }
    }
}

const COLOR_TABLE: [u32; 24] = [
    0x10801080, 0x317031b1, 0x51605157, 0x734f7387, 0x1cb11c79, 0x3ea03ea9, 0x5e905e4f, 0xc080c080,
    0x10801080, 0x535f53e2, 0x9340932e, 0xd61ed68f, 0x29e22971, 0x6cc06cd2, 0xaca1ac1e, 0xf080f080,
    0x10801080, 0x2377239c, 0x356e3569, 0x48644884, 0x179c177c, 0x2a922a97, 0x3c893c64, 0x70807080,
];

const COLOR_CUBE: [u8; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
const GRAY_SCALE: [u8; 24] = [
    0x08, 0x12, 0x1c, 0x26, 0x30, 0x3a, 0x44, 0x4e, 0x58, 0x62, 0x6c, 0x76, 0x80, 0x8a, 0x94, 0x9e,
    0xa8, 0xb2, 0xbc, 0xc6, 0xd0, 0xda, 0xe4, 0xee,
];

const FRAMEBUFFER_ALIGNMENT: usize = 32;

#[cfg(target_arch = "powerpc")]
unsafe fn allocate_aligned_zeroed(size: usize) -> *mut c_void {
    unsafe { framebuffer_pool_alloc(size) }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn allocate_aligned_zeroed(size: usize) -> *mut c_void {
    let Ok(layout) = Layout::from_size_align(size.max(1), FRAMEBUFFER_ALIGNMENT) else {
        return core::ptr::null_mut();
    };
    alloc_zeroed(layout).cast()
}

fn mem_k0_to_k1(ptr: *mut c_void) -> *mut c_void {
    let addr = ptr as usize;
    if (addr & 0xE000_0000) == SYS_BASE_CACHED {
        (addr + (SYS_BASE_UNCACHED - SYS_BASE_CACHED)) as *mut c_void
    } else {
        ptr
    }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn free_aligned(ptr: *mut c_void, size: usize) {
    if ptr.is_null() {
        return;
    }
    let Ok(layout) = Layout::from_size_align(size.max(1), FRAMEBUFFER_ALIGNMENT) else {
        return;
    };
    dealloc(ptr.cast(), layout);
}

#[cfg(target_arch = "powerpc")]
unsafe fn framebuffer_pool_alloc(size: usize) -> *mut c_void {
    unsafe { framebuffer_pool_alloc_with_range(size).0 }
}

#[cfg(target_arch = "powerpc")]
unsafe fn framebuffer_pool_alloc_with_range(size: usize) -> (*mut c_void, usize, usize) {
    unsafe {
        let size = size.max(1);
        let base = core::ptr::addr_of_mut!(FRAMEBUFFER_POOL.0) as *mut u8 as usize;
        let mut offset = FRAMEBUFFER_POOL_NEXT.load(Ordering::Acquire);
        loop {
            let aligned =
                (base + offset + (FRAMEBUFFER_ALIGNMENT - 1)) & !(FRAMEBUFFER_ALIGNMENT - 1);
            let next = aligned.saturating_add(size).saturating_sub(base);
            if next > FRAMEBUFFER_POOL_BYTES {
                return (core::ptr::null_mut(), 0, 0);
            }
            match FRAMEBUFFER_POOL_NEXT.compare_exchange(
                offset,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let ptr = aligned as *mut u8;
                    core::ptr::write_bytes(ptr, 0, size);
                    return (ptr.cast(), offset, next);
                }
                Err(current) => offset = current,
            }
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn allocate_console_buffer(size: usize) -> *mut c_void {
    let (ptr, start, end) = unsafe { framebuffer_pool_alloc_with_range(size) };
    if !ptr.is_null() {
        CONSOLE_BUFFER_POOL_START.store(start, Ordering::Release);
        CONSOLE_BUFFER_POOL_END.store(end, Ordering::Release);
    }
    ptr
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn allocate_console_buffer(size: usize) -> *mut c_void {
    unsafe { allocate_aligned_zeroed(size) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn free_console_buffer(ptr: *mut c_void, _size: usize) {
    if ptr.is_null() {
        return;
    }
    // BUG FIX: release this bump-pool allocation when it is still the last
    // allocation. Reinitialization also reuses its existing capacity below.
    let end = CONSOLE_BUFFER_POOL_END.load(Ordering::Acquire);
    let start = CONSOLE_BUFFER_POOL_START.load(Ordering::Acquire);
    let _ = FRAMEBUFFER_POOL_NEXT.compare_exchange(end, start, Ordering::AcqRel, Ordering::Acquire);
    CONSOLE_BUFFER_POOL_END.store(0, Ordering::Release);
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn free_console_buffer(ptr: *mut c_void, size: usize) {
    unsafe { free_aligned(ptr, size) }
}

pub fn init() {
    #[cfg(target_arch = "powerpc")]
    sys::init()
}

pub fn flush() {
    #[cfg(target_arch = "powerpc")]
    sys::flush()
}

pub fn set_black(black: bool) {
    VIDEO_BLACK.store(black, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    sys::set_black(black as Bool)
}

pub fn get_next_field() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = sys::get_next_field();
        LAST_NEXT_FIELD.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_NEXT_FIELD.load(Ordering::Acquire)
    }
}

pub fn get_current_line() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = sys::get_current_line();
        LAST_CURRENT_LINE.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_CURRENT_LINE.load(Ordering::Acquire)
    }
}

pub fn get_current_tv_mode() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = sys::get_current_tv_mode();
        LAST_CURRENT_TV_MODE.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_CURRENT_TV_MODE.load(Ordering::Acquire)
    }
}

/// # Safety
///
/// `rmode` must point to a valid render mode structure.
pub unsafe fn configure(rmode: *mut GxrModeObj) {
    unsafe {
        let _ = rmode;
        #[cfg(target_arch = "powerpc")]
        sys::configure(rmode)
    }
}

/// # Safety
///
/// `rmode` must point to a valid render mode structure.
pub unsafe fn get_frame_buffer_size(rmode: *mut GxrModeObj) -> u32 {
    unsafe {
        #[cfg(target_arch = "powerpc")]
        {
            sys::get_frame_buffer_size(rmode)
        }
        #[cfg(not(target_arch = "powerpc"))]
        {
            if rmode.is_null() {
                0
            } else {
                unsafe {
                    let width = ((*rmode).fb_width.wrapping_add(15)) & !15;
                    let height = (*rmode).xfb_height as u32 + if (*rmode).aa != 0 { 4 } else { 0 };
                    width as u32 * height * 2
                }
            }
        }
    }
}

/// # Safety
///
/// `rmode` and `fb` must be valid for the requested clear operation.
pub unsafe fn clear_frame_buffer(rmode: *mut GxrModeObj, fb: *mut c_void, color: u32) {
    unsafe {
        #[cfg(target_arch = "powerpc")]
        sys::clear_frame_buffer(rmode, fb, color);
        #[cfg(not(target_arch = "powerpc"))]
        {
            if rmode.is_null() || fb.is_null() {
                return;
            }
            let size = get_frame_buffer_size(rmode) as usize;
            let word_count = size / core::mem::size_of::<u32>();
            let pixels = unsafe { core::slice::from_raw_parts_mut(fb.cast::<u32>(), word_count) };
            pixels.fill(color);
        }
    }
}

pub fn wait_vsync() {
    #[cfg(target_arch = "powerpc")]
    sys::wait_v_sync();
    #[cfg(not(target_arch = "powerpc"))]
    {
        let retrace_cnt = HOST_VSYNC_COUNT
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        if let Some(callback) = decode_callback(PRE_RETRACE_CALLBACK_PTR.load(Ordering::Acquire)) {
            unsafe {
                callback(retrace_cnt);
            }
        }
        let next = LAST_NEXT_FRAMEBUFFER.load(Ordering::Acquire);
        if next != 0 {
            LAST_FRAMEBUFFER.store(next, Ordering::Release);
        }
        LAST_NEXT_FIELD.fetch_xor(1, Ordering::AcqRel);
        LAST_CURRENT_LINE.fetch_add(1, Ordering::AcqRel);
        if let Some(callback) = decode_callback(POST_RETRACE_CALLBACK_PTR.load(Ordering::Acquire)) {
            unsafe {
                callback(retrace_cnt);
            }
        }
    }
}

/// Wait for the retrace counter to advance without sleeping on the LWP queue.
///
/// Return `false` when no retrace interrupt advances the counter before the
/// timeout. This is intended for bounded diagnostics of VI interrupt waits.
pub fn wait_vsync_timeout_us(timeout_us: u64) -> bool {
    sys::wait_v_sync_timeout_us(timeout_us)
}

/// # Safety
///
/// `fb` must be valid for VI framebuffer use.
pub unsafe fn set_framebuffer(fb: *mut c_void) {
    unsafe {
        LAST_FRAMEBUFFER.store(fb as usize, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        sys::set_framebuffer(fb)
    }
}

/// # Safety
///
/// `fb` must be valid for VI framebuffer use.
pub unsafe fn set_next_framebuffer(fb: *mut c_void) {
    unsafe {
        LAST_NEXT_FRAMEBUFFER.store(fb as usize, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        sys::set_next_framebuffer(fb)
    }
}

/// # Safety
///
/// `fb` must be valid for VI framebuffer use.
pub unsafe fn set_next_right_framebuffer(fb: *mut c_void) {
    unsafe {
        LAST_NEXT_RIGHT_FRAMEBUFFER.store(fb as usize, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        sys::set_next_right_framebuffer(fb)
    }
}

pub fn have_component_cable() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = sys::have_component_cable();
        LAST_HAVE_COMPONENT_CABLE.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_HAVE_COMPONENT_CABLE.load(Ordering::Acquire)
    }
}

/// # Safety
///
/// `mode` may be null or must point to writable render mode storage.
pub unsafe fn get_preferred_mode(mode: *mut GxrModeObj) -> *mut GxrModeObj {
    unsafe {
        #[cfg(target_arch = "powerpc")]
        {
            sys::get_preferred_mode(mode)
        }
        #[cfg(not(target_arch = "powerpc"))]
        {
            mode
        }
    }
}

pub fn get_video_scan_mode() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        let value = sys::get_video_scan_mode();
        LAST_VIDEO_SCAN_MODE.store(value, Ordering::Release);
        value
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_VIDEO_SCAN_MODE.load(Ordering::Acquire)
    }
}

pub fn set_pre_retrace_callback(callback: ViRetraceCallback) -> ViRetraceCallback {
    #[cfg(target_arch = "powerpc")]
    {
        sys::set_pre_retrace_callback(callback)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let previous =
            PRE_RETRACE_CALLBACK_PTR.swap(callback.map_or(0, |cb| cb as usize), Ordering::AcqRel);
        decode_callback(previous)
    }
}

pub fn set_post_retrace_callback(callback: ViRetraceCallback) -> ViRetraceCallback {
    #[cfg(target_arch = "powerpc")]
    {
        sys::set_post_retrace_callback(callback)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let previous =
            POST_RETRACE_CALLBACK_PTR.swap(callback.map_or(0, |cb| cb as usize), Ordering::AcqRel);
        decode_callback(previous)
    }
}

#[cfg(not(target_arch = "powerpc"))]
fn decode_callback(ptr: usize) -> ViRetraceCallback {
    if ptr == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(u32)>(ptr) })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoSnapshot {
    pub black: bool,
    pub current_tv_mode: u32,
    pub video_scan_mode: u32,
    pub have_component_cable: u32,
    pub current_line: u32,
    pub next_field: u32,
    pub framebuffer: usize,
    pub next_framebuffer: usize,
    pub next_right_framebuffer: usize,
    pub pre_retrace_callback: usize,
    pub post_retrace_callback: usize,
}

pub fn snapshot() -> VideoSnapshot {
    VideoSnapshot {
        black: VIDEO_BLACK.load(Ordering::Acquire),
        current_tv_mode: LAST_CURRENT_TV_MODE.load(Ordering::Acquire),
        video_scan_mode: LAST_VIDEO_SCAN_MODE.load(Ordering::Acquire),
        have_component_cable: LAST_HAVE_COMPONENT_CABLE.load(Ordering::Acquire),
        current_line: LAST_CURRENT_LINE.load(Ordering::Acquire),
        next_field: LAST_NEXT_FIELD.load(Ordering::Acquire),
        framebuffer: LAST_FRAMEBUFFER.load(Ordering::Acquire),
        next_framebuffer: LAST_NEXT_FRAMEBUFFER.load(Ordering::Acquire),
        next_right_framebuffer: LAST_NEXT_RIGHT_FRAMEBUFFER.load(Ordering::Acquire),
        pre_retrace_callback: PRE_RETRACE_CALLBACK_PTR.load(Ordering::Acquire),
        post_retrace_callback: POST_RETRACE_CALLBACK_PTR.load(Ordering::Acquire),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoHardwareProof {
    pub registers: VideoRegisterSnapshot,
    pub control_register_matches: bool,
    pub next_framebuffer_registers_match: bool,
}

/// Reads video state back from the hardware-facing sys layer.
///
/// On Wii this reads VI registers directly through the target sys layer.
/// Non-PowerPC runtime builds are rejected at crate root.
///
/// # Safety
/// `rmode` must point to a valid initialized mode object and a non-null
/// `expected_next_framebuffer` must be valid for that mode.
pub unsafe fn hardware_proof(
    rmode: *mut GxrModeObj,
    expected_next_framebuffer: *mut c_void,
) -> VideoHardwareProof {
    VideoHardwareProof {
        registers: sys::read_register_snapshot(),
        control_register_matches: unsafe { sys::control_register_matches(rmode) } != 0,
        next_framebuffer_registers_match: unsafe {
            sys::framebuffer_registers_match(expected_next_framebuffer, rmode)
        } != 0,
    }
}

pub fn read_register_snapshot() -> VideoRegisterSnapshot {
    sys::read_register_snapshot()
}

/// Checks the programmed framebuffer registers.
///
/// # Safety
/// `rmode` must be valid and a non-null `fb` must identify a framebuffer for it.
pub unsafe fn framebuffer_registers_match(fb: *mut c_void, rmode: *mut GxrModeObj) -> bool {
    (unsafe { sys::framebuffer_registers_match(fb, rmode) }) != 0
}

/// Checks the programmed VI control register.
///
/// # Safety
/// `rmode` must point to a valid initialized mode object.
pub unsafe fn control_register_matches(rmode: *mut GxrModeObj) -> bool {
    (unsafe { sys::control_register_matches(rmode) }) != 0
}

pub fn console_get_default() -> *mut PrintConsole {
    unsafe {
        DEFAULT_CONSOLE.font.gfx = CONSOLE_FONT_8X16.as_ptr() as *mut u8;
        DEFAULT_CONSOLE.font.ascii_offset = 0;
        DEFAULT_CONSOLE.font.num_chars = 256;
    }
    core::ptr::addr_of_mut!(DEFAULT_CONSOLE)
}

/// # Safety
///
/// `console` must be null or point to a valid `PrintConsole` that remains alive and is not incompatibly aliased while selected.
pub unsafe fn console_select(console: *mut PrintConsole) -> *mut PrintConsole {
    unsafe {
        let previous = CURRENT_CONSOLE_PTR;
        CURRENT_CONSOLE_PTR = console;
        previous
    }
}

/// # Safety
///
/// `console` must be null or a valid console pointer. `font` must point to a
/// valid font descriptor for the duration of the call.
pub unsafe fn console_set_font(console: *mut PrintConsole, font: *const ConsoleFont) {
    if font.is_null() {
        return;
    }
    unsafe {
        let console = if console.is_null() {
            CURRENT_CONSOLE_PTR
        } else {
            console
        };
        if console.is_null() {
            return;
        }
        (*console).font = *font;
    }
}

/// # Safety
///
/// `console` must be null or point to a valid writable `PrintConsole`; a null pointer requires the selected console to remain valid.
pub unsafe fn console_set_window(
    console: *mut PrintConsole,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    unsafe {
        let console = if console.is_null() {
            CURRENT_CONSOLE_PTR
        } else {
            console
        };
        if console.is_null() {
            return;
        }
        let console = &mut *console;
        // BUG FIX: C keeps these arguments unsigned through its bounds and
        // clipping arithmetic. Cast only when storing into the C int fields.
        let x = x.max(1);
        let y = y.max(1);
        let cols = console.con_cols as u32;
        let rows = console.con_rows as u32;
        if x >= cols || y >= rows {
            return;
        }
        let mut width = width;
        let mut height = height;
        if x.wrapping_add(width) > cols.wrapping_add(1) {
            width = cols.wrapping_add(1).wrapping_sub(x);
        }
        if y.wrapping_add(height) > rows.wrapping_add(1) {
            height = rows.wrapping_add(1).wrapping_sub(y);
        }
        console.window_x = x as i32;
        console.window_y = y as i32;
        console.window_width = width as i32;
        console.window_height = height as i32;
        console.cursor_x = 1;
        console.cursor_y = 1;
    }
}

pub fn console_get_metrics(cols: &mut i32, rows: &mut i32) {
    unsafe {
        if !CURRENT_CONSOLE_PTR.is_null() {
            *cols = (*CURRENT_CONSOLE_PTR).con_cols;
            *rows = (*CURRENT_CONSOLE_PTR).con_rows;
        }
    }
}

pub fn console_get_position(col: &mut i32, row: &mut i32) {
    unsafe {
        if !CURRENT_CONSOLE_PTR.is_null() {
            *col = (*CURRENT_CONSOLE_PTR).cursor_x;
            *row = (*CURRENT_CONSOLE_PTR).cursor_y;
        }
    }
}

pub fn console_clear() {
    unsafe {
        clear_console();
    }
}

/// Blend an RGBA image into the active console buffer.
///
/// Coordinates are relative to the active console. The image is scaled to
/// `width` by `height` and clipped at the console bounds.
pub fn console_blit_rgba(
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    source_width: u16,
    source_height: u16,
    rgba: &[u8],
) -> Result<(), i32> {
    let (width, height) = (usize::from(width), usize::from(height));
    let (source_width, source_height) = (usize::from(source_width), usize::from(source_height));
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(-1)?;
    if width == 0
        || height == 0
        || source_width == 0
        || source_height == 0
        || rgba.len() < source_len
    {
        return Err(-1);
    }

    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return Err(-1);
        }
        let console = &*CURRENT_CONSOLE_PTR;
        if console.destbuffer.is_null() || console.con_stride <= 0 || console.tgt_stride <= 0 {
            return Err(-1);
        }
        let left = x.max(0) as usize;
        let top = y.max(0) as usize;
        let right = x.saturating_add(width as i32).clamp(0, console.con_xres) as usize;
        let bottom = y.saturating_add(height as i32).clamp(0, console.con_yres) as usize;
        if left >= right || top >= bottom {
            return Ok(());
        }

        // The software console has a compact row stride. Direct consoles use
        // the XFB stride and origin instead.
        let (framebuffer, stride) = if !CONSOLE_BUFFER.is_null() {
            (CONSOLE_BUFFER, console.con_stride as usize)
        } else {
            (
                offset_by_pixels(
                    console.destbuffer.cast(),
                    console.target_y,
                    console.tgt_stride,
                    console.target_x,
                ),
                console.tgt_stride as usize,
            )
        };

        let pair_start = left & !1;
        let pair_end = (right + 1) & !1;
        for py in top..bottom {
            let source_y = ((py as i32 - y) as usize) * source_height / height;
            for pair_x in (pair_start..pair_end).step_by(2) {
                if pair_x + 1 >= console.con_xres as usize {
                    continue;
                }
                let pixel = framebuffer.add(py * stride + pair_x * 2);
                let mut rgb = [
                    xfb_color::ycbcr_to_rgb(*pixel, *pixel.add(1), *pixel.add(3)),
                    xfb_color::ycbcr_to_rgb(*pixel.add(2), *pixel.add(1), *pixel.add(3)),
                ];
                let mut touched = [false; 2];
                for lane in 0..2 {
                    let screen_x = pair_x + lane;
                    if screen_x < left || screen_x >= right {
                        continue;
                    }
                    let source_x = ((screen_x as i32 - x) as usize) * source_width / width;
                    let source = (source_y * source_width + source_x) * 4;
                    let alpha = u32::from(rgba[source + 3]);
                    if alpha == 0 {
                        continue;
                    }
                    touched[lane] = true;
                    for channel in 0..3 {
                        rgb[lane][channel] = ((u32::from(rgba[source + channel]) * alpha
                            + u32::from(rgb[lane][channel]) * (255 - alpha)
                            + 127)
                            / 255) as u8;
                    }
                }
                if touched[0] || touched[1] {
                    let original = [*pixel, *pixel.add(1), *pixel.add(2), *pixel.add(3)];
                    let packed = xfb_color::pack_ycbcr_pair(original, rgb, touched);
                    *pixel = packed[0];
                    *pixel.add(1) = packed[1];
                    *pixel.add(2) = packed[2];
                    *pixel.add(3) = packed[3];
                }
            }
        }
    }
    Ok(())
}

/// # Safety
///
/// `console` must be null or point to a valid writable `PrintConsole` that remains alive while selected.
pub unsafe fn console_init(console: *mut PrintConsole) -> *mut PrintConsole {
    unsafe {
        DEFAULT_CONSOLE.font.gfx = CONSOLE_FONT_8X16.as_ptr() as *mut u8;
        DEFAULT_CONSOLE.font.ascii_offset = 0;
        DEFAULT_CONSOLE.font.num_chars = 256;
        if begin_console_video_init(&CONSOLE_VIDEO_INITIALIZED) {
            // BUG FIX: C's consoleInit performs VI and framebuffer setup only
            // on its first call. Repeating it allocates another framebuffer
            // and resets the active video state.
            let rmode = get_preferred_mode(null_mut());
            if rmode.is_null() {
                CONSOLE_VIDEO_INITIALIZED.store(false, Ordering::Release);
                return null_mut();
            }
            let xfb_size = get_frame_buffer_size(rmode) as usize;
            let xfb = allocate_aligned_zeroed(xfb_size);
            if xfb.is_null() {
                CONSOLE_VIDEO_INITIALIZED.store(false, Ordering::Release);
                return null_mut();
            }
            let xfb_uncached = mem_k0_to_k1(xfb);
            console_init_raw(
                xfb_uncached,
                0,
                0,
                (*rmode).fb_width as i32,
                (*rmode).xfb_height as i32,
                (*rmode).fb_width as i32 * VI_DISPLAY_PIX_SZ as i32,
            );
            configure(rmode);
            set_next_framebuffer(xfb_uncached);
            set_black(false);
            flush();
            wait_vsync();
            if ((*rmode).vi_tv_mode & 1) != 0 {
                wait_vsync();
            }
        }
        let chosen = if console.is_null() {
            CURRENT_CONSOLE_PTR
        } else {
            console
        };
        if !chosen.is_null() {
            CURRENT_CONSOLE_PTR = chosen;
            *chosen = DEFAULT_CONSOLE;
            (*chosen).console_initialised = true;
            clear_console();
        }
        CURRENT_CONSOLE_PTR
    }
}

/// # Safety
///
/// `rmode` must point to a valid render mode structure.
pub unsafe fn console_init_ex(
    rmode: *mut GxrModeObj,
    con_x_origin: i32,
    con_y_origin: i32,
    con_width: i32,
    con_height: i32,
) -> Result<(), i32> {
    unsafe {
        set_post_retrace_callback(None);
        if rmode.is_null() {
            if !CONSOLE_BUFFER.is_null() {
                free_console_buffer(
                    CONSOLE_BUFFER.cast(),
                    CONSOLE_BUFFER_SIZE.swap(0, Ordering::AcqRel),
                );
                CONSOLE_BUFFER = null_mut();
            }
            return Err(-1);
        }
        let mut width = con_width;
        let mut height = con_height;
        if ((*rmode).fb_width as i32) < con_x_origin + width {
            width = (*rmode).fb_width as i32 - con_x_origin;
        }
        if ((*rmode).xfb_height as i32) < con_y_origin + height {
            height = (*rmode).xfb_height as i32 - con_y_origin;
        }
        width = (width / FONT_XSIZE as i32) * FONT_XSIZE as i32;
        height = (height / FONT_YSIZE as i32) * FONT_YSIZE as i32;
        let size = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(VI_DISPLAY_PIX_SZ);
        let capacity = CONSOLE_BUFFER_SIZE.load(Ordering::Acquire);
        if !CONSOLE_BUFFER.is_null() && capacity >= size.max(1) {
            // BUG FIX: C frees and replaces this buffer on every init. Reuse
            // the existing allocation when it can hold the new console.
            core::ptr::write_bytes(CONSOLE_BUFFER, 0, size);
        } else {
            if !CONSOLE_BUFFER.is_null() {
                free_console_buffer(
                    CONSOLE_BUFFER.cast(),
                    CONSOLE_BUFFER_SIZE.swap(0, Ordering::AcqRel),
                );
                CONSOLE_BUFFER = null_mut();
            }
            CONSOLE_BUFFER = allocate_console_buffer(size).cast();
            if !CONSOLE_BUFFER.is_null() {
                CONSOLE_BUFFER_SIZE.store(size.max(1), Ordering::Release);
            }
        }
        if CONSOLE_BUFFER.is_null() {
            return Err(-1);
        }
        console_init_ex_raw(
            CONSOLE_BUFFER.cast(),
            con_x_origin,
            con_y_origin,
            (*rmode).fb_width as i32 * VI_DISPLAY_PIX_SZ as i32,
            width,
            height,
            width * VI_DISPLAY_PIX_SZ as i32,
        );
        Ok(())
    }
}

/// Initializes a console that renders directly into an existing framebuffer.
///
/// This avoids the retrace-callback copy used by [`console_init_ex`] and is
/// suitable for early boot diagnostics before VI interrupt delivery is known
/// to work.
///
/// # Safety
///
/// `rmode` must point to a valid render mode. `framebuffer` must be writable
/// for the framebuffer size described by `rmode` and remain valid while the
/// console is in use.
pub unsafe fn console_init_framebuffer(
    rmode: *mut GxrModeObj,
    framebuffer: *mut c_void,
    con_x_origin: i32,
    con_y_origin: i32,
    con_width: i32,
    con_height: i32,
) -> Result<(), i32> {
    unsafe {
        set_post_retrace_callback(None);
        if !CONSOLE_BUFFER.is_null() {
            free_console_buffer(
                CONSOLE_BUFFER.cast(),
                CONSOLE_BUFFER_SIZE.swap(0, Ordering::AcqRel),
            );
            CONSOLE_BUFFER = null_mut();
        }
        if rmode.is_null() || framebuffer.is_null() {
            return Err(-1);
        }
        let width = con_width
            .min((*rmode).fb_width as i32 - con_x_origin)
            .max(0)
            / FONT_XSIZE as i32
            * FONT_XSIZE as i32;
        let height = con_height
            .min((*rmode).xfb_height as i32 - con_y_origin)
            .max(0)
            / FONT_YSIZE as i32
            * FONT_YSIZE as i32;
        if width == 0 || height == 0 {
            return Err(-1);
        }
        set_default_console(
            framebuffer,
            con_x_origin,
            con_y_origin,
            (*rmode).fb_width as i32 * VI_DISPLAY_PIX_SZ as i32,
            width,
            height,
            (*rmode).fb_width as i32 * VI_DISPLAY_PIX_SZ as i32,
        );
        Ok(())
    }
}

pub fn console_write(bytes: &[u8]) -> Result<usize, i32> {
    unsafe { console_write_inner(bytes) }
}

unsafe fn console_init_raw(
    framebuffer: *mut c_void,
    xstart: i32,
    ystart: i32,
    xres: i32,
    yres: i32,
    stride: i32,
) {
    unsafe {
        let total_words = (yres as usize).saturating_mul(stride as usize) / 4;
        let ptr = framebuffer.cast::<u32>();
        for i in 0..total_words {
            {
                *ptr.add(i) = COLOR_TABLE[0];
            }
        }
        let xres = ((xres - xstart) / FONT_XSIZE as i32) * FONT_XSIZE as i32;
        let yres = ((yres - ystart) / FONT_YSIZE as i32) * FONT_YSIZE as i32;
        set_default_console(framebuffer, xstart, ystart, stride, xres, yres, stride);
    }
}

unsafe fn console_init_ex_raw(
    destbuffer: *mut c_void,
    tgt_xstart: i32,
    tgt_ystart: i32,
    tgt_stride: i32,
    con_xres: i32,
    con_yres: i32,
    con_stride: i32,
) {
    unsafe {
        set_default_console(
            destbuffer, tgt_xstart, tgt_ystart, tgt_stride, con_xres, con_yres, con_stride,
        );
        set_post_retrace_callback(Some(console_vipostcb));
    }
}

unsafe fn set_default_console(
    destbuffer: *mut c_void,
    xstart: i32,
    ystart: i32,
    tgt_stride: i32,
    con_xres: i32,
    con_yres: i32,
    con_stride: i32,
) {
    unsafe {
        let con = core::ptr::addr_of_mut!(DEFAULT_CONSOLE);
        (*con).destbuffer = destbuffer;
        (*con).target_x = xstart;
        (*con).target_y = ystart;
        (*con).con_xres = con_xres;
        (*con).con_yres = con_yres;
        (*con).tgt_stride = tgt_stride;
        (*con).con_stride = con_stride;
        (*con).con_cols = con_xres / FONT_XSIZE as i32;
        (*con).con_rows = con_yres / FONT_YSIZE as i32;
        (*con).window_width = (*con).con_cols;
        (*con).window_height = (*con).con_rows;
        (*con).window_x = 1;
        (*con).window_y = 1;
        (*con).cursor_x = 1;
        (*con).cursor_y = 1;
        (*con).prev_cursor_x = 1;
        (*con).prev_cursor_y = 1;
        (*con).font.gfx = CONSOLE_FONT_8X16.as_ptr() as *mut u8;
        (*con).font.ascii_offset = 0;
        (*con).font.num_chars = 256;
        (*con).fg = CONSOLE_COLOR_WHITE;
        (*con).bg = CONSOLE_COLOR_BLACK;
        (*con).flags = 0;
        (*con).tab_size = 3;
        (*con).console_initialised = true;
        CURRENT_COPY = *con;
        CURRENT_CONSOLE_PTR = core::ptr::addr_of_mut!(CURRENT_COPY);
        clear_console();
    }
}

unsafe extern "C" fn console_vipostcb(_retrace_cnt: u32) {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() || CONSOLE_BUFFER.is_null() {
            return;
        }
        DO_XFB_COPY.store(true, Ordering::Release);
        let console = { &*CURRENT_CONSOLE_PTR };
        let mut ptr = CONSOLE_BUFFER;
        let mut fb = offset_by_pixels(
            sys::get_current_framebuffer().cast(),
            console.target_y,
            console.tgt_stride,
            console.target_x,
        );
        let line_count = console.con_yres as usize;
        let bytes_per_line = console.con_xres as usize * VI_DISPLAY_PIX_SZ;
        for _ in 0..line_count {
            copy_nonoverlapping(ptr, fb, bytes_per_line);
            ptr = ptr.add(console.con_stride as usize);
            fb = fb.add(console.tgt_stride as usize);
        }
        DO_XFB_COPY.store(false, Ordering::Release);
    }
}

unsafe fn console_write_inner(bytes: &[u8]) -> Result<usize, i32> {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return Err(-1);
        }
        let mut count = 0;
        for &byte in bytes {
            count += 1;
            match ESCAPE_SEQ.state {
                EscapeState::None => {
                    if byte == 0x1b {
                        ESCAPE_SEQ.state = EscapeState::Start;
                    } else {
                        print_char(byte as i32);
                    }
                }
                EscapeState::Start => {
                    if byte == b'[' {
                        ESCAPE_SEQ = EscapeSeq::new();
                        ESCAPE_SEQ.color.bg = (*CURRENT_CONSOLE_PTR).bg;
                        ESCAPE_SEQ.color.fg = (*CURRENT_CONSOLE_PTR).fg;
                        ESCAPE_SEQ.color.flags = (*CURRENT_CONSOLE_PTR).flags;
                        ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
                    } else {
                        print_char(0x1b);
                        print_char(byte as i32);
                        ESCAPE_SEQ.state = EscapeState::None;
                    }
                }
                _ => {
                    handle_escape_byte(byte);
                }
            }
        }
        Ok(count)
    }
}

unsafe fn handle_escape_byte(byte: u8) {
    unsafe {
        match byte {
            b'0'..=b'9' => {
                ESCAPE_SEQ.has_arg = true;
                ESCAPE_SEQ.args[ESCAPE_SEQ.arg_idx] =
                    ESCAPE_SEQ.args[ESCAPE_SEQ.arg_idx] * 10 + i32::from(byte - b'0');
            }
            b';' => {
                if ESCAPE_SEQ.has_arg && ESCAPE_SEQ.arg_idx + 1 < ANSI_MAX_ARGS {
                    ESCAPE_SEQ.arg_idx += 1;
                }
                ESCAPE_SEQ.has_arg = false;
            }
            b'A' => move_cursor_y(-escape_default_arg()),
            b'B' => move_cursor_y(escape_default_arg()),
            b'C' => move_cursor_x(escape_default_arg()),
            b'D' => move_cursor_x(-escape_default_arg()),
            b'H' | b'f' => {
                let row = ESCAPE_SEQ.args[0];
                let col = ESCAPE_SEQ.args[1];
                position_cursor(col, row);
                ESCAPE_SEQ.state = EscapeState::None;
            }
            b'J' => {
                clear_mode(if ESCAPE_SEQ.arg_idx == 0 && !ESCAPE_SEQ.has_arg {
                    0
                } else {
                    ESCAPE_SEQ.args[0]
                });
                ESCAPE_SEQ.state = EscapeState::None;
            }
            b'K' => {
                clear_line_mode(if ESCAPE_SEQ.arg_idx == 0 && !ESCAPE_SEQ.has_arg {
                    0
                } else {
                    ESCAPE_SEQ.args[0]
                });
                ESCAPE_SEQ.state = EscapeState::None;
            }
            b's' => {
                (*CURRENT_CONSOLE_PTR).prev_cursor_x = (*CURRENT_CONSOLE_PTR).cursor_x;
                (*CURRENT_CONSOLE_PTR).prev_cursor_y = (*CURRENT_CONSOLE_PTR).cursor_y;
                ESCAPE_SEQ.state = EscapeState::None;
            }
            b'u' => {
                (*CURRENT_CONSOLE_PTR).cursor_x = (*CURRENT_CONSOLE_PTR).prev_cursor_x;
                (*CURRENT_CONSOLE_PTR).cursor_y = (*CURRENT_CONSOLE_PTR).prev_cursor_y;
                ESCAPE_SEQ.state = EscapeState::None;
            }
            b'm' => {
                if ESCAPE_SEQ.arg_idx == 0 && !ESCAPE_SEQ.has_arg {
                    ESCAPE_SEQ.args[0] = 0;
                    ESCAPE_SEQ.arg_idx = 1;
                } else if ESCAPE_SEQ.has_arg {
                    ESCAPE_SEQ.arg_idx += 1;
                }
                handle_color_escape(ESCAPE_SEQ.arg_idx);
                ESCAPE_SEQ.state = EscapeState::None;
            }
            _ => {
                ESCAPE_SEQ.state = EscapeState::None;
            }
        }
    }
}

unsafe fn move_cursor_x(delta: i32) {
    unsafe {
        (*CURRENT_CONSOLE_PTR).cursor_x =
            ((*CURRENT_CONSOLE_PTR).cursor_x + delta).clamp(1, (*CURRENT_CONSOLE_PTR).window_width);
        ESCAPE_SEQ.state = EscapeState::None;
    }
}

unsafe fn move_cursor_y(delta: i32) {
    unsafe {
        (*CURRENT_CONSOLE_PTR).cursor_y = ((*CURRENT_CONSOLE_PTR).cursor_y + delta)
            .clamp(1, (*CURRENT_CONSOLE_PTR).window_height);
        ESCAPE_SEQ.state = EscapeState::None;
    }
}

unsafe fn position_cursor(x: i32, y: i32) {
    unsafe {
        if x < 0 || y < 0 {
            return;
        }
        let console = &mut *CURRENT_CONSOLE_PTR;
        console.cursor_x = x.max(1).min(console.window_width);
        console.cursor_y = y.max(1).min(console.window_height);
    }
}

unsafe fn print_char(c: i32) {
    unsafe {
        if c == 0 || CURRENT_CONSOLE_PTR.is_null() {
            return;
        }
        let console = &mut *CURRENT_CONSOLE_PTR;
        match c {
            8 => {
                console.cursor_x -= 1;
                if console.cursor_x < 1 {
                    if console.cursor_y > 1 {
                        console.cursor_x = console.window_width;
                        console.cursor_y -= 1;
                    } else {
                        console.cursor_x = 1;
                    }
                }
                draw_char(b' ' as i32);
            }
            9 => {
                let mut tabspaces = console.tab_size - ((console.cursor_x - 1) % console.tab_size);
                if console.cursor_x + tabspaces > console.window_width {
                    tabspaces = console.window_width - console.cursor_x;
                }
                for _ in 0..tabspaces {
                    print_char(b' ' as i32);
                }
            }
            10 => {
                new_row();
                console.cursor_x = 1;
            }
            13 => {
                console.cursor_x = 1;
            }
            _ => {
                if console.cursor_x > console.window_width {
                    console.cursor_x = 1;
                    new_row();
                }
                draw_char(c);
                console.cursor_x += 1;
            }
        }
    }
}

unsafe fn draw_char(c: i32) {
    unsafe {
        if DO_XFB_COPY.load(Ordering::Acquire) || CURRENT_CONSOLE_PTR.is_null() {
            return;
        }
        let console = &mut *CURRENT_CONSOLE_PTR;
        let glyph = c - i32::from(console.font.ascii_offset);
        if glyph < 0 || glyph > i32::from(console.font.num_chars) {
            return;
        }
        let ptr = get_cursor_start_ptr();
        if ptr.is_null() {
            return;
        }
        let pbits = {
            core::slice::from_raw_parts(
                console.font.gfx.add(glyph as usize * FONT_YSIZE),
                FONT_YSIZE,
            )
        };
        let nextline = console.con_stride as usize / core::mem::size_of::<u32>() - 4;
        let mut fgcolor = resolve_fg(console);
        let mut bgcolor = resolve_bg(console);
        if (console.flags & CONSOLE_COLOR_REVERSE) != 0 {
            core::mem::swap(&mut fgcolor, &mut bgcolor);
        }
        let ptr_words = ptr.cast::<u32>();
        let mut word_index = 0usize;
        for (ay, &mut_bits) in pbits.iter().enumerate() {
            let mut bits = mut_bits;
            if (console.flags & CONSOLE_CROSSED_OUT) != 0 && (ay == 7 || ay == 8) {
                bits = 0xff;
            }
            if (console.flags & CONSOLE_UNDERLINE) != 0 && (ay == 14 || ay == 15) {
                bits = 0xff;
            }
            for pair in 0..4 {
                let hi = 0x80 >> (pair * 2);
                let lo = 0x40 >> (pair * 2);
                let mut color = if (bits & hi) != 0 {
                    fgcolor & 0xffff00ff
                } else {
                    bgcolor & 0xffff00ff
                };
                color |= if (bits & lo) != 0 {
                    fgcolor & 0x0000ff00
                } else {
                    bgcolor & 0x0000ff00
                };
                {
                    *ptr_words.add(word_index) = color;
                }
                word_index += 1;
            }
            word_index += nextline;
        }
    }
}

unsafe fn new_row() {
    unsafe {
        let console = &mut *CURRENT_CONSOLE_PTR;
        console.cursor_y += 1;
        if console.cursor_y > console.window_height {
            let ptr = get_window_start_ptr();
            if ptr.is_null() {
                return;
            }
            let line_width = console.window_width as usize * FONT_XSIZE * VI_DISPLAY_PIX_SZ;
            let scroll_height = (console.window_height as usize - 1) * FONT_YSIZE;
            let row_bytes = console.con_stride as usize * FONT_YSIZE;
            let mut line_ptr = ptr;
            for _ in 0..scroll_height {
                {
                    core::ptr::copy(line_ptr.add(row_bytes), line_ptr, line_width);
                }
                line_ptr = line_ptr.add(console.con_stride as usize);
            }
            clear_line(console.window_height, 1, console.window_width + 1);
            console.cursor_y = console.window_height;
        }
    }
}

unsafe fn clear_console() {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return;
        }
        let console = &mut *CURRENT_CONSOLE_PTR;
        let mut p = get_window_start_ptr();
        if p.is_null() {
            return;
        }
        let view_height = console.window_height as usize * FONT_YSIZE;
        let line_width = console.window_width as usize * FONT_XSIZE * VI_DISPLAY_PIX_SZ;
        let bgcolor = resolve_bg(console);
        for _ in 0..view_height {
            for x in (0..line_width).step_by(core::mem::size_of::<u32>()) {
                {
                    *(p.add(x).cast::<u32>()) = bgcolor;
                }
            }
            p = p.add(console.con_stride as usize);
        }
        console.cursor_x = 1;
        console.cursor_y = 1;
        console.prev_cursor_x = 1;
        console.prev_cursor_y = 1;
    }
}

unsafe fn clear_mode(mode: i32) {
    unsafe {
        match mode {
            0 => clear_from_cursor(),
            1 => clear_to_cursor(),
            2 | 3 => clear_console(),
            _ => {}
        }
    }
}

unsafe fn clear_line_mode(mode: i32) {
    unsafe {
        let console = &*CURRENT_CONSOLE_PTR;
        match mode {
            0 => clear_line(console.cursor_y, console.cursor_x, console.window_width + 1),
            1 => clear_line(console.cursor_y, 1, console.cursor_x),
            2 => clear_line(console.cursor_y, 1, console.window_width + 1),
            _ => {}
        }
    }
}

unsafe fn clear_from_cursor() {
    unsafe {
        let console = &*CURRENT_CONSOLE_PTR;
        let mut row = console.cursor_y;
        clear_line(row, console.cursor_x, console.window_width + 1);
        while row < console.window_height {
            row += 1;
            clear_line(row, 1, console.window_width + 1);
        }
    }
}

unsafe fn clear_to_cursor() {
    unsafe {
        let console = &*CURRENT_CONSOLE_PTR;
        let mut row = console.cursor_y;
        clear_line(row, 1, console.cursor_x);
        while row > 1 {
            row -= 1;
            clear_line(row, 1, console.window_width + 1);
        }
    }
}

unsafe fn clear_line(line: i32, from: i32, to: i32) {
    unsafe {
        let console = &*CURRENT_CONSOLE_PTR;
        let line_width = (to - from) as usize * FONT_XSIZE * VI_DISPLAY_PIX_SZ;
        let bgcolor = resolve_bg(console);
        let mut p = get_window_start_ptr();
        if p.is_null() {
            return;
        }
        p = offset_by_cursor(p, line, console.con_stride, from);
        for _ in 0..FONT_YSIZE {
            for x in (0..line_width).step_by(core::mem::size_of::<u32>()) {
                {
                    *(p.add(x).cast::<u32>()) = bgcolor;
                }
            }
            p = p.add(console.con_stride as usize);
        }
    }
}

unsafe fn resolve_fg(console: &PrintConsole) -> u32 {
    if (console.flags & CONSOLE_FG_CUSTOM) == 0 {
        let mut fg = console.fg;
        if (console.flags & (CONSOLE_COLOR_BOLD | CONSOLE_COLOR_FG_BRIGHT)) != 0 {
            fg += 8;
        } else if (console.flags & CONSOLE_COLOR_FAINT) != 0 {
            fg += 16;
        }
        COLOR_TABLE[fg as usize]
    } else {
        console.fg
    }
}

unsafe fn resolve_bg(console: &PrintConsole) -> u32 {
    if (console.flags & CONSOLE_BG_CUSTOM) == 0 {
        let mut bg = console.bg;
        if (console.flags & CONSOLE_COLOR_BG_BRIGHT) != 0 {
            bg += 8;
        }
        COLOR_TABLE[bg as usize]
    } else {
        console.bg
    }
}

fn rgb8_to_ycbcr(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> u32 {
    let clamp = |value: u8| value.clamp(16, 240);
    let (r1, g1, b1, r2, g2, b2) = (
        clamp(r1),
        clamp(g1),
        clamp(b1),
        clamp(r2),
        clamp(g2),
        clamp(b2),
    );
    let y1 = (77 * u32::from(r1) + 150 * u32::from(g1) + 29 * u32::from(b1)) / 256;
    let y2 = (77 * u32::from(r2) + 150 * u32::from(g2) + 29 * u32::from(b2)) / 256;
    let cb = (112 * (u32::from(b1) + u32::from(b2))
        - 74 * (u32::from(g1) + u32::from(g2))
        - 38 * (u32::from(r1) + u32::from(r2)))
        / 512
        + 128;
    let cr = (112 * (u32::from(r1) + u32::from(r2))
        - 94 * (u32::from(g1) + u32::from(g2))
        - 18 * (u32::from(b1) + u32::from(b2)))
        / 512
        + 128;
    (y1 << 24) | (cb << 16) | (y2 << 8) | cr
}

fn single_rgb8_to_ycbcr(r: u8, g: u8, b: u8) -> u32 {
    rgb8_to_ycbcr(r, g, b, r, g, b)
}

unsafe fn handle_color_escape(arg_count: usize) {
    unsafe {
        ESCAPE_SEQ.color.bg = (*CURRENT_CONSOLE_PTR).bg;
        ESCAPE_SEQ.color.fg = (*CURRENT_CONSOLE_PTR).fg;
        ESCAPE_SEQ.color.flags = (*CURRENT_CONSOLE_PTR).flags;
        for arg in 0..arg_count {
            let code = core::ptr::addr_of!(ESCAPE_SEQ.args)
                .cast::<i32>()
                .add(arg)
                .read();
            match ESCAPE_SEQ.state {
                EscapeState::BuildingUnknown => set_color_state(code),
                EscapeState::BuildingFormatFg => {
                    ESCAPE_SEQ.state = match code {
                        5 => EscapeState::BuildingFormatFgNonRgb,
                        2 => EscapeState::BuildingFormatFgRgb,
                        _ => EscapeState::BuildingUnknown,
                    };
                }
                EscapeState::BuildingFormatBg => {
                    ESCAPE_SEQ.state = match code {
                        5 => EscapeState::BuildingFormatBgNonRgb,
                        2 => EscapeState::BuildingFormatBgRgb,
                        _ => EscapeState::BuildingUnknown,
                    };
                }
                EscapeState::BuildingFormatFgNonRgb => {
                    set_non_rgb(true, code as u32);
                    ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
                }
                EscapeState::BuildingFormatBgNonRgb => {
                    set_non_rgb(false, code as u32);
                    ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
                }
                EscapeState::BuildingFormatFgRgb => {
                    ESCAPE_SEQ.color_args[ESCAPE_SEQ.color_arg_count] = code as u32;
                    ESCAPE_SEQ.color_arg_count += 1;
                    if ESCAPE_SEQ.color_arg_count == 3 {
                        ESCAPE_SEQ.color.fg = single_rgb8_to_ycbcr(
                            ESCAPE_SEQ.color_args[0] as u8,
                            ESCAPE_SEQ.color_args[1] as u8,
                            ESCAPE_SEQ.color_args[2] as u8,
                        );
                        ESCAPE_SEQ.color.flags |= CONSOLE_FG_CUSTOM;
                        ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
                        ESCAPE_SEQ.color_arg_count = 0;
                    }
                }
                EscapeState::BuildingFormatBgRgb => {
                    ESCAPE_SEQ.color_args[ESCAPE_SEQ.color_arg_count] = code as u32;
                    ESCAPE_SEQ.color_arg_count += 1;
                    if ESCAPE_SEQ.color_arg_count == 3 {
                        ESCAPE_SEQ.color.bg = single_rgb8_to_ycbcr(
                            ESCAPE_SEQ.color_args[0] as u8,
                            ESCAPE_SEQ.color_args[1] as u8,
                            ESCAPE_SEQ.color_args[2] as u8,
                        );
                        ESCAPE_SEQ.color.flags |= CONSOLE_BG_CUSTOM;
                        ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
                        ESCAPE_SEQ.color_arg_count = 0;
                    }
                }
                _ => {}
            }
        }
        (*CURRENT_CONSOLE_PTR).bg = ESCAPE_SEQ.color.bg;
        (*CURRENT_CONSOLE_PTR).fg = ESCAPE_SEQ.color.fg;
        (*CURRENT_CONSOLE_PTR).flags = ESCAPE_SEQ.color.flags;
        ESCAPE_SEQ.arg_idx = 0;
        ESCAPE_SEQ.state = EscapeState::BuildingUnknown;
    }
}

unsafe fn set_non_rgb(is_fg: bool, code: u32) {
    unsafe {
        if code <= 15 {
            if is_fg {
                ESCAPE_SEQ.color.fg = code;
                ESCAPE_SEQ.color.flags &= !CONSOLE_FG_CUSTOM;
            } else {
                ESCAPE_SEQ.color.bg = code;
                ESCAPE_SEQ.color.flags &= !CONSOLE_BG_CUSTOM;
            }
        } else if code <= 231 {
            let code = code - 16;
            let r = code / 36;
            let g = (code - r * 36) / 6;
            let b = code - r * 36 - g * 6;
            let value = single_rgb8_to_ycbcr(
                COLOR_CUBE[r as usize],
                COLOR_CUBE[g as usize],
                COLOR_CUBE[b as usize],
            );
            if is_fg {
                ESCAPE_SEQ.color.fg = value;
                ESCAPE_SEQ.color.flags |= CONSOLE_FG_CUSTOM;
            } else {
                ESCAPE_SEQ.color.bg = value;
                ESCAPE_SEQ.color.flags |= CONSOLE_BG_CUSTOM;
            }
        } else if code <= 255 {
            let gray = GRAY_SCALE[(code - 232) as usize];
            let value = single_rgb8_to_ycbcr(gray, gray, gray);
            if is_fg {
                ESCAPE_SEQ.color.fg = value;
                ESCAPE_SEQ.color.flags |= CONSOLE_FG_CUSTOM;
            } else {
                ESCAPE_SEQ.color.bg = value;
                ESCAPE_SEQ.color.flags |= CONSOLE_BG_CUSTOM;
            }
        }
    }
}

unsafe fn set_color_state(code: i32) {
    unsafe {
        match code {
            0 => {
                ESCAPE_SEQ.color.flags = 0;
                ESCAPE_SEQ.color.bg = 0;
                ESCAPE_SEQ.color.fg = 7;
            }
            1 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_FAINT;
                ESCAPE_SEQ.color.flags |= CONSOLE_COLOR_BOLD;
            }
            2 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_BOLD;
                ESCAPE_SEQ.color.flags |= CONSOLE_COLOR_FAINT;
            }
            3 => ESCAPE_SEQ.color.flags |= CONSOLE_ITALIC,
            4 => ESCAPE_SEQ.color.flags |= CONSOLE_UNDERLINE,
            5 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_BLINK_FAST;
                ESCAPE_SEQ.color.flags |= CONSOLE_BLINK_SLOW;
            }
            6 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_BLINK_SLOW;
                ESCAPE_SEQ.color.flags |= CONSOLE_BLINK_FAST;
            }
            7 => ESCAPE_SEQ.color.flags |= CONSOLE_COLOR_REVERSE,
            8 => ESCAPE_SEQ.color.flags |= CONSOLE_CONCEAL,
            9 => ESCAPE_SEQ.color.flags |= CONSOLE_CROSSED_OUT,
            21 => ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_BOLD,
            22 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_BOLD;
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_FAINT;
            }
            23 => ESCAPE_SEQ.color.flags &= !CONSOLE_ITALIC,
            24 => ESCAPE_SEQ.color.flags &= !CONSOLE_UNDERLINE,
            25 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_BLINK_SLOW;
                ESCAPE_SEQ.color.flags &= !CONSOLE_BLINK_FAST;
            }
            27 => ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_REVERSE,
            29 => ESCAPE_SEQ.color.flags &= !CONSOLE_CROSSED_OUT,
            30..=37 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_FG_CUSTOM;
                ESCAPE_SEQ.color.fg = (code - 30) as u32;
            }
            38 => {
                ESCAPE_SEQ.state = EscapeState::BuildingFormatFg;
                ESCAPE_SEQ.color_arg_count = 0;
            }
            39 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_FG_CUSTOM;
                ESCAPE_SEQ.color.fg = 7;
            }
            40..=47 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_BG_CUSTOM;
                ESCAPE_SEQ.color.bg = (code - 40) as u32;
            }
            48 => {
                ESCAPE_SEQ.state = EscapeState::BuildingFormatBg;
                ESCAPE_SEQ.color_arg_count = 0;
            }
            49 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_BG_CUSTOM;
                ESCAPE_SEQ.color.bg = 0;
            }
            90..=97 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_FAINT;
                ESCAPE_SEQ.color.flags |= CONSOLE_COLOR_FG_BRIGHT;
                ESCAPE_SEQ.color.flags &= !CONSOLE_FG_CUSTOM;
                ESCAPE_SEQ.color.fg = (code - 90) as u32;
            }
            100..=107 => {
                ESCAPE_SEQ.color.flags &= !CONSOLE_COLOR_FAINT;
                ESCAPE_SEQ.color.flags |= CONSOLE_COLOR_BG_BRIGHT;
                ESCAPE_SEQ.color.flags &= !CONSOLE_BG_CUSTOM;
                ESCAPE_SEQ.color.bg = (code - 100) as u32;
            }
            _ => {}
        }
    }
}

fn escape_default_arg() -> i32 {
    unsafe {
        let arg = if !ESCAPE_SEQ.has_arg && ESCAPE_SEQ.arg_idx == 0 {
            1
        } else {
            ESCAPE_SEQ.args[0]
        };
        ESCAPE_SEQ.state = EscapeState::None;
        arg
    }
}

unsafe fn get_window_start_ptr() -> *mut u8 {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return null_mut();
        }
        let con = &*CURRENT_CONSOLE_PTR;
        offset_by_cursor(
            get_buffer_start_ptr(),
            con.window_y,
            con.con_stride,
            con.window_x,
        )
    }
}

unsafe fn get_cursor_start_ptr() -> *mut u8 {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return null_mut();
        }
        let con = &*CURRENT_CONSOLE_PTR;
        offset_by_cursor(
            get_buffer_start_ptr(),
            con.window_y + con.cursor_y - 1,
            con.con_stride,
            con.window_x + con.cursor_x - 1,
        )
    }
}

unsafe fn get_buffer_start_ptr() -> *mut u8 {
    unsafe {
        if CURRENT_CONSOLE_PTR.is_null() {
            return null_mut();
        }
        let con = &*CURRENT_CONSOLE_PTR;
        if !CONSOLE_BUFFER.is_null() {
            CONSOLE_BUFFER
        } else {
            offset_by_pixels(
                con.destbuffer.cast(),
                con.target_y,
                con.tgt_stride,
                con.target_x,
            )
        }
    }
}

fn offset_by_cursor(ptr: *mut u8, cursor_y: i32, stride_bytes: i32, cursor_x: i32) -> *mut u8 {
    offset_by_pixels(
        ptr,
        (cursor_y - 1) * FONT_YSIZE as i32,
        stride_bytes,
        (cursor_x - 1) * FONT_XSIZE as i32,
    )
}

fn offset_by_pixels(ptr: *mut u8, dy_pixels: i32, stride_bytes: i32, dx_pixels: i32) -> *mut u8 {
    if ptr.is_null() {
        return null_mut();
    }
    let dy_bytes = dy_pixels as isize * stride_bytes as isize;
    let dx_bytes = dx_pixels as isize * VI_DISPLAY_PIX_SZ as isize;
    unsafe { ptr.offset(dy_bytes + dx_bytes) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn console_video_setup_runs_only_once() {
        let initialized = AtomicBool::new(false);
        assert!(begin_console_video_init(&initialized));
        assert!(!begin_console_video_init(&initialized));
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn();
        let _ = flush as fn();
        let _ = set_black as fn(bool);
        let _ = wait_vsync as fn();
        let _ = have_component_cable as fn() -> u32;
        let _ = get_video_scan_mode as fn() -> u32;
        let _ = console_write as fn(&[u8]) -> Result<usize, i32>;
        let _ =
            console_init_ex as unsafe fn(*mut GxrModeObj, i32, i32, i32, i32) -> Result<(), i32>;
    }

    #[test]
    fn console_font_shape_matches_c_asset() {
        assert_eq!(CONSOLE_FONT_8X16.len(), 256 * 16);
        assert_eq!(CONSOLE_FONT_8X16[0], 0x00);
        assert_eq!(CONSOLE_FONT_8X16[18], 0x7e);
    }

    #[test]
    fn console_window_clamps_to_console_bounds() {
        let mut console = PrintConsole::new();
        console.con_cols = 80;
        console.con_rows = 30;

        unsafe { console_set_window(&mut console, 10, 5, 100, 100) };

        assert_eq!(console.window_x, 10);
        assert_eq!(console.window_y, 5);
        assert_eq!(console.window_width, 71);
        assert_eq!(console.window_height, 26);
        assert_eq!(console.cursor_x, 1);
        assert_eq!(console.cursor_y, 1);
    }

    #[test]
    fn console_window_rejects_out_of_range_origin() {
        let mut console = PrintConsole::new();
        console.con_cols = 80;
        console.con_rows = 30;
        let before = console;

        unsafe { console_set_window(&mut console, 81, 1, 10, 10) };

        assert_eq!(console.window_x, before.window_x);
        assert_eq!(console.window_y, before.window_y);
        assert_eq!(console.window_width, before.window_width);
        assert_eq!(console.window_height, before.window_height);
    }

    #[test]
    fn console_window_uses_unsigned_c_bounds_and_clipping() {
        let mut console = PrintConsole::new();
        console.con_cols = 80;
        console.con_rows = 30;
        let before = console;

        unsafe { console_set_window(&mut console, 0x8000_0000, 1, 10, 10) };
        assert_eq!(console.window_x, before.window_x);
        assert_eq!(console.window_width, before.window_width);

        unsafe { console_set_window(&mut console, 1, 1, 0x8000_0000, 10) };
        assert_eq!(console.window_x, 1);
        assert_eq!(console.window_width, 80);
    }

    #[test]
    fn console_init_ex_reuses_existing_buffer_capacity() {
        let mut rmode = GxrModeObj {
            vi_tv_mode: 0,
            fb_width: 320,
            efb_height: 240,
            xfb_height: 240,
            vi_x_origin: 0,
            vi_y_origin: 0,
            vi_width: 320,
            vi_height: 240,
            xfb_mode: 0,
            field_rendering: 0,
            aa: 0,
            sample_pattern: [[0; 2]; 12],
            vfilter: [0; 7],
        };

        unsafe {
            console_init_ex(&mut rmode, 0, 0, 320, 240).unwrap();
            let first_buffer = core::ptr::addr_of!(CONSOLE_BUFFER).read();
            console_init_ex(&mut rmode, 0, 0, 320, 240).unwrap();
            let second_buffer = core::ptr::addr_of!(CONSOLE_BUFFER).read();
            assert_eq!(second_buffer, first_buffer);
            assert_eq!(
                console_init_ex(core::ptr::null_mut(), 0, 0, 320, 240),
                Err(-1)
            );
        }
    }

    #[test]
    fn snapshot_tracks_owned_state() {
        set_black(true);
        let _ = get_current_tv_mode();
        let _ = get_video_scan_mode();
        let _ = have_component_cable();
        let _ = get_next_field();
        let _ = get_current_line();
        let _ = set_pre_retrace_callback(None);
        let _ = set_post_retrace_callback(None);

        assert_eq!(
            snapshot(),
            VideoSnapshot {
                black: true,
                current_tv_mode: get_current_tv_mode(),
                video_scan_mode: get_video_scan_mode(),
                have_component_cable: have_component_cable(),
                current_line: get_current_line(),
                next_field: get_next_field(),
                framebuffer: 0,
                next_framebuffer: 0,
                next_right_framebuffer: 0,
                pre_retrace_callback: 0,
                post_retrace_callback: 0,
            }
        );
    }

    #[test]
    fn wait_vsync_promotes_next_framebuffer_and_dispatches_callbacks() {
        use core::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

        static CALLBACK_PHASE: AtomicU32 = AtomicU32::new(0);
        static PRE_COUNT: AtomicU32 = AtomicU32::new(0);
        static POST_COUNT: AtomicU32 = AtomicU32::new(0);

        unsafe extern "C" fn pre(_cnt: u32) {
            PRE_COUNT.fetch_add(1, AtomicOrdering::AcqRel);
            let _ = CALLBACK_PHASE.compare_exchange(
                0,
                1,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            );
        }

        unsafe extern "C" fn post(_cnt: u32) {
            POST_COUNT.fetch_add(1, AtomicOrdering::AcqRel);
            let _ = CALLBACK_PHASE.compare_exchange(
                1,
                2,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            );
        }

        let _ = set_pre_retrace_callback(Some(pre));
        let _ = set_post_retrace_callback(Some(post));

        let mut dummy_a = [0u8; 8];
        let mut dummy_b = [0u8; 8];
        unsafe {
            set_framebuffer(dummy_a.as_mut_ptr().cast());
            set_next_framebuffer(dummy_b.as_mut_ptr().cast());
        }

        CALLBACK_PHASE.store(0, AtomicOrdering::Release);
        PRE_COUNT.store(0, AtomicOrdering::Release);
        POST_COUNT.store(0, AtomicOrdering::Release);

        wait_vsync();

        assert_eq!(CALLBACK_PHASE.load(AtomicOrdering::Acquire), 2);
        assert_eq!(PRE_COUNT.load(AtomicOrdering::Acquire), 1);
        assert_eq!(POST_COUNT.load(AtomicOrdering::Acquire), 1);
        assert_eq!(snapshot().framebuffer, snapshot().next_framebuffer);
        assert_eq!(snapshot().framebuffer, dummy_b.as_mut_ptr() as usize);

        let _ = set_pre_retrace_callback(None);
        let _ = set_post_retrace_callback(None);
    }

    #[test]
    fn retrace_callback_setters_return_previous_callbacks() {
        unsafe extern "C" fn pre_one(_: u32) {}
        unsafe extern "C" fn pre_two(_: u32) {}
        unsafe extern "C" fn post_one(_: u32) {}
        unsafe extern "C" fn post_two(_: u32) {}
        let pre_one_cb: usize = pre_one as *const () as usize;
        let pre_two_cb: usize = pre_two as *const () as usize;
        let post_one_cb: usize = post_one as *const () as usize;
        let post_two_cb: usize = post_two as *const () as usize;

        assert!(set_pre_retrace_callback(Some(pre_one)).is_none());
        assert_eq!(
            set_pre_retrace_callback(Some(pre_two)).map(|cb| cb as usize),
            Some(pre_one_cb)
        );
        assert_eq!(
            set_pre_retrace_callback(None).map(|cb| cb as usize),
            Some(pre_two_cb)
        );

        assert!(set_post_retrace_callback(Some(post_one)).is_none());
        assert_eq!(
            set_post_retrace_callback(Some(post_two)).map(|cb| cb as usize),
            Some(post_one_cb)
        );
        assert_eq!(
            set_post_retrace_callback(None).map(|cb| cb as usize),
            Some(post_two_cb)
        );
    }

    #[test]
    fn clear_frame_buffer_fills_host_buffer() {
        let mut rmode = GxrModeObj {
            vi_tv_mode: 0,
            fb_width: 2,
            efb_height: 2,
            xfb_height: 2,
            vi_x_origin: 0,
            vi_y_origin: 0,
            vi_width: 2,
            vi_height: 2,
            xfb_mode: 0,
            field_rendering: 0,
            aa: 0,
            sample_pattern: [[0; 2]; 12],
            vfilter: [0; 7],
        };
        let mut framebuffer = [0u32; 2];

        unsafe {
            clear_frame_buffer(&mut rmode, framebuffer.as_mut_ptr().cast(), 0x1122_3344);
        }

        assert_eq!(framebuffer, [0x1122_3344, 0x1122_3344]);
    }

    #[test]
    fn framebuffer_size_pads_width_like_libogc() {
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

        let size = unsafe { get_frame_buffer_size(&mut rmode) };

        assert_eq!(size, 448);
    }
}
