#![no_std]

mod framebuffer_state;
mod platform_timing;
mod retrace_wait;

use framebuffer_state::{
    adjusted_display_y_origin, can_apply_flushed_mode_change, framebuffer_to_latch,
    framebuffer_to_publish, line_from_half_line, next_field_after_current,
    right_framebuffer_to_latch, television_standard,
};

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use ogc_core::mem_virtual_to_physical;
#[cfg(target_arch = "powerpc")]
use ogc_irq::{self as irq, IRQ_PI_VI};
#[cfg(target_arch = "powerpc")]
use ogc_lwp_sys as lwp;

pub use ogc_sys::Bool;
pub use platform_timing::{
    PlatformClock, VideoStandard, retrace_period_ticks, ticks_to_microseconds,
    video_standard_for_mode,
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GxrModeObj {
    pub vi_tv_mode: u32,
    pub fb_width: u16,
    pub efb_height: u16,
    pub xfb_height: u16,
    pub vi_x_origin: u16,
    pub vi_y_origin: u16,
    pub vi_width: u16,
    pub vi_height: u16,
    pub xfb_mode: u32,
    pub field_rendering: u8,
    pub aa: u8,
    pub sample_pattern: [[u8; 2]; 12],
    pub vfilter: [u8; 7],
}

pub type ViRetraceCallback = Option<unsafe extern "C" fn(retrace_cnt: u32)>;
pub type ViPositionCallback = Option<unsafe extern "C" fn(pos_x: u32, pos_y: u32)>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoRegisterSnapshot {
    pub control: u16,
    pub vertical_timing: u16,
    pub top_field_high: u16,
    pub top_field_low: u16,
    pub bottom_field_high: u16,
    pub bottom_field_low: u16,
    pub current_framebuffer: usize,
    pub next_framebuffer: usize,
}

static VIDEO_BLACK: AtomicBool = AtomicBool::new(false);
static LAST_CURRENT_TV_MODE: AtomicU32 = AtomicU32::new(0);
static LAST_VIDEO_SCAN_MODE: AtomicU32 = AtomicU32::new(0);
static LAST_HAVE_COMPONENT_CABLE: AtomicU32 = AtomicU32::new(0);
static LAST_CURRENT_LINE: AtomicU32 = AtomicU32::new(0);
static LAST_NEXT_FIELD: AtomicU32 = AtomicU32::new(0);
static LAST_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static LAST_RIGHT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static DISPLAY_OFFSET_H: AtomicU32 = AtomicU32::new(0);
static mut CONFIGURED_MODE: GxrModeObj = TV_NTSC_480_INT_DF;
static mut FLUSHED_MODE: GxrModeObj = TV_NTSC_480_INT_DF;
static CONFIGURED_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static LAST_NEXT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static FLUSHED_NEXT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static LAST_NEXT_RIGHT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static FLUSHED_NEXT_RIGHT_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);
static PRE_RETRACE_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
static POST_RETRACE_CALLBACK_PTR: AtomicUsize = AtomicUsize::new(0);
static RETRACE_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static RETRACE_WAIT_QUEUE: AtomicU32 = AtomicU32::new(lwp::LWP_TQUEUE_NULL);
static FRAMEBUFFER_FLUSH_PENDING: AtomicBool = AtomicBool::new(false);
static CONFIGURED_MODE_DIRTY: AtomicBool = AtomicBool::new(false);
static FLUSHED_MODE_PENDING: AtomicBool = AtomicBool::new(false);
static FLUSHED_MODE_CHANGE_PENDING: AtomicBool = AtomicBool::new(false);
static VIDEO_BLACK_DIRTY: AtomicBool = AtomicBool::new(false);
static FLUSHED_VIDEO_BLACK: AtomicBool = AtomicBool::new(false);
static FLUSHED_BLACK_PENDING: AtomicBool = AtomicBool::new(false);

const VI_INTERLACE: u32 = 0;
const VI_NON_INTERLACE: u32 = 1;
const VI_PROGRESSIVE: u32 = 2;
const VI_NTSC: u32 = 0;
const VI_XFBMODE_DF: u32 = 1;
const VI_MAX_WIDTH_NTSC: u16 = 720;
const VI_MAX_HEIGHT_NTSC: u16 = 480;
const VI_DEFAULT_FILTER_TAPS: [u16; 26] = [
    0x01f0, 0x01dc, 0x01ae, 0x0174, 0x0129, 0x00db, 0x008e, 0x0046, 0x000c, 0x00e2, 0x00cb, 0x00c0,
    0x00c4, 0x00cf, 0x00de, 0x00ec, 0x00fc, 0x0008, 0x000f, 0x0013, 0x0013, 0x000f, 0x000c, 0x0008,
    0x0001, 0x0000,
];
#[cfg(target_arch = "powerpc")]
const VI_REG_BASE: usize = 0xCC00_2000;

const fn vi_tvmode(fmt: u32, mode: u32) -> u32 {
    (fmt << 2) + mode
}

fn decode_vi_mode(control: u16, progressive: bool) -> Option<u32> {
    if control & 1 == 0 {
        return None;
    }
    // C libogc reads the VI television format from bits 8-9 only.
    let format = u32::from((control >> 8) & 0x3);
    let scan = if progressive {
        2
    } else {
        u32::from((control >> 2) & 1)
    };
    Some((format << 2) | scan)
}

fn video_scan_mode_from_registers(clock_select: u16, control: u16) -> u32 {
    if clock_select & 1 != 0 {
        VI_PROGRESSIVE
    } else if (control >> 2) & 1 != 0 {
        VI_NON_INTERLACE
    } else {
        VI_INTERLACE
    }
}

fn encode_default_filter_registers() -> [u16; 14] {
    let t = VI_DEFAULT_FILTER_TAPS;
    [
        (t[1] >> 6) | (t[2] << 4),
        t[0] | (t[1] << 10),
        (t[4] >> 6) | (t[5] << 4),
        t[3] | (t[4] << 10),
        (t[7] >> 6) | (t[8] << 4),
        t[6] | (t[7] << 10),
        t[11] | (t[12] << 8),
        t[9] | (t[10] << 8),
        t[15] | (t[16] << 8),
        t[13] | (t[14] << 8),
        t[19] | (t[20] << 8),
        t[17] | (t[18] << 8),
        t[23] | (t[24] << 8),
        t[21] | (t[22] << 8),
    ]
}

fn vi_needs_timing_init(control: u16) -> bool {
    control & 1 == 0
}

pub const TV_NTSC_480_INT_DF: GxrModeObj = GxrModeObj {
    vi_tv_mode: vi_tvmode(VI_NTSC, VI_INTERLACE),
    fb_width: 640,
    efb_height: 480,
    xfb_height: 480,
    vi_x_origin: (VI_MAX_WIDTH_NTSC - 640) / 2,
    vi_y_origin: (VI_MAX_HEIGHT_NTSC - 480) / 2,
    vi_width: 640,
    vi_height: 480,
    xfb_mode: VI_XFBMODE_DF,
    field_rendering: 0,
    aa: 0,
    sample_pattern: [[6, 6]; 12],
    vfilter: [8, 8, 10, 12, 10, 8, 8],
};

static mut DEFAULT_MODE: GxrModeObj = TV_NTSC_480_INT_DF;
static mut PREFERRED_MODE: GxrModeObj = PREFERRED_NTSC_INTERLACED;

const fn preferred_mode(
    tv_mode: u32,
    efb_height: u16,
    xfb_height: u16,
    vi_height: u16,
    xfb_mode: u32,
    y_origin: u16,
    vfilter: [u8; 7],
) -> GxrModeObj {
    GxrModeObj {
        vi_tv_mode: tv_mode,
        fb_width: 640,
        efb_height,
        xfb_height,
        vi_x_origin: 40,
        vi_y_origin: y_origin,
        vi_width: 640,
        vi_height,
        xfb_mode,
        field_rendering: 0,
        aa: 0,
        sample_pattern: [[6, 6]; 12],
        vfilter,
    }
}

const DEFAULT_VIDEO_FILTER: [u8; 7] = [8, 8, 10, 12, 10, 8, 8];
const PROGRESSIVE_VIDEO_FILTER: [u8; 7] = [0, 0, 21, 22, 21, 0, 0];
const PREFERRED_NTSC_PROGRESSIVE: GxrModeObj =
    preferred_mode(2, 480, 480, 480, 0, 0, PROGRESSIVE_VIDEO_FILTER);
const PREFERRED_NTSC_INTERLACED: GxrModeObj =
    preferred_mode(0, 480, 480, 480, 1, 0, DEFAULT_VIDEO_FILTER);
const PREFERRED_PAL_PROGRESSIVE: GxrModeObj =
    preferred_mode(6, 480, 576, 576, 0, 0, PROGRESSIVE_VIDEO_FILTER);
const PREFERRED_PAL_INTERLACED: GxrModeObj =
    preferred_mode(4, 480, 576, 576, 1, 0, DEFAULT_VIDEO_FILTER);
const PREFERRED_MPAL_PROGRESSIVE: GxrModeObj =
    preferred_mode(10, 480, 480, 480, 0, 0, PROGRESSIVE_VIDEO_FILTER);
const PREFERRED_MPAL_INTERLACED: GxrModeObj =
    preferred_mode(8, 480, 480, 480, 1, 0, DEFAULT_VIDEO_FILTER);
const PREFERRED_EURGB60_PROGRESSIVE: GxrModeObj =
    preferred_mode(22, 480, 480, 480, 0, 0, PROGRESSIVE_VIDEO_FILTER);
const PREFERRED_EURGB60_INTERLACED: GxrModeObj =
    preferred_mode(20, 480, 480, 480, 1, 0, DEFAULT_VIDEO_FILTER);

/// Standard modes from the canonical libogc `video.c` render-mode table.
pub const TV_NTSC_480_PROG: GxrModeObj = PREFERRED_NTSC_PROGRESSIVE;
pub const TV_EURGB60_480_PROG: GxrModeObj = PREFERRED_EURGB60_PROGRESSIVE;
pub const TV_EURGB60_480_INT_DF: GxrModeObj = PREFERRED_EURGB60_INTERLACED;
pub const TV_PAL_528_INT_DF: GxrModeObj =
    preferred_mode(4, 528, 528, 528, 1, 24, DEFAULT_VIDEO_FILTER);

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn vi_write_reg(index: usize, value: u16) {
    unsafe {
        core::ptr::write_volatile((VI_REG_BASE as *mut u16).add(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn vi_read_reg(index: usize) -> u16 {
    unsafe { core::ptr::read_volatile((VI_REG_BASE as *const u16).add(index)) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn apply_horizontal_regs(rmode: &GxrModeObj) {
    unsafe {
        let Some(timing) = platform_timing::vi_timing_for_mode(rmode.vi_tv_mode) else {
            return;
        };
        let requested_pos_x =
            rmode.vi_x_origin as i32 + DISPLAY_OFFSET_H.load(Ordering::Acquire) as i32;
        let max_pos_x = 720i32 - rmode.vi_width as i32;
        let disp_pos_x = requested_pos_x.clamp(0, max_pos_x.max(0)) as u32;
        let disp_size_x = rmode.vi_width as u32;
        let val1 = (timing.hbe640 as u32)
            .wrapping_add(disp_pos_x)
            .wrapping_sub(40)
            & 0x01ff;
        let val2 = (timing.hbs640 as u32)
            .wrapping_add(disp_pos_x)
            .wrapping_add(40)
            .wrapping_sub(720u32.wrapping_sub(disp_size_x));
        vi_write_reg(2, (timing.hcs << 8) | timing.hce);
        vi_write_reg(3, timing.hlw);
        vi_write_reg(4, ((val1 >> 9) | (val2 << 1)) as u16);
        vi_write_reg(5, ((val1 << 7) | timing.hsy as u32) as u16);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn apply_picture_config(rmode: &GxrModeObj) {
    unsafe {
        let word_per_line = (rmode.fb_width.wrapping_add(15)) / 16;
        let pan_size_x = rmode.fb_width as u32;
        let three_d = (rmode.vi_tv_mode & 3) == 3;
        let pan_size_x = if three_d { pan_size_x << 1 } else { pan_size_x };
        let mut std = word_per_line as u32;
        if rmode.xfb_mode == VI_XFBMODE_DF {
            std <<= 1;
        }
        let wpl = pan_size_x.div_ceil(16);
        vi_write_reg(36, ((wpl << 8) | std) as u16);
        if pan_size_x < rmode.vi_width as u32 {
            let scale =
                ((pan_size_x << 8).wrapping_sub(1) + rmode.vi_width as u32) / rmode.vi_width as u32;
            vi_write_reg(37, (0x1000 | scale) as u16);
            vi_write_reg(56, pan_size_x as u16);
        } else {
            vi_write_reg(37, 0x0100);
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn apply_timing_regs(rmode: &GxrModeObj) {
    unsafe {
        let Some(timing) = platform_timing::vi_timing_for_mode(rmode.vi_tv_mode) else {
            return;
        };
        let half_line = if timing.nhlines & 1 != 0 {
            timing.hlw + 1
        } else {
            1
        };
        vi_write_reg(24, 0x1000 | ((timing.nhlines / 2) + 1));
        vi_write_reg(25, half_line);
        vi_write_reg(10, (timing.be3 << 5) | timing.bs3);
        vi_write_reg(11, (timing.be1 << 5) | timing.bs1);
        vi_write_reg(12, (timing.be4 << 5) | timing.bs4);
        vi_write_reg(13, (timing.be2 << 5) | timing.bs2);

        let mode = rmode.vi_tv_mode;
        let progressive = mode & 2 != 0;
        let three_d = mode & 3 == 3;
        let format = (mode >> 2) & 7;
        let mut control = vi_read_reg(1) & !0x030c;
        control |= u16::from(three_d) << 3;
        if progressive || three_d {
            control |= 0x0004;
        } else {
            control |= ((mode & 1) as u16) << 2;
        }
        if format != 5 {
            control |= (format as u16) << 8;
        }
        vi_write_reg(1, control);
        vi_write_reg(54, u16::from(progressive));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn vi_init(vimode: u32, initialize_retrace: bool) {
    unsafe {
        let Some(timing) = platform_timing::vi_timing_for_mode(vimode) else {
            return;
        };
        let Some(control) = platform_timing::vi_control_register_for_mode(vimode) else {
            return;
        };
        vi_write_reg(1, 0x0002);
        let mut cnt = 0;
        while cnt < 1000 {
            core::sync::atomic::compiler_fence(Ordering::SeqCst);
            cnt += 1;
        }
        vi_write_reg(1, 0x0000);
        vi_write_reg(2, (timing.hcs << 8) | timing.hce);
        vi_write_reg(3, timing.hlw);
        vi_write_reg(4, timing.hbs640 << 1);
        vi_write_reg(5, (timing.hbe640 << 7) | timing.hsy);
        vi_write_reg(0, timing.equ);
        vi_write_reg(6, timing.psb_odd.wrapping_add(2));
        vi_write_reg(
            7,
            timing
                .prb_odd
                .wrapping_add((timing.acv << 1).wrapping_sub(2)),
        );
        vi_write_reg(8, timing.psb_even.wrapping_add(2));
        vi_write_reg(
            9,
            timing
                .prb_even
                .wrapping_add((timing.acv << 1).wrapping_sub(2)),
        );
        vi_write_reg(10, (timing.be3 << 5) | timing.bs3);
        vi_write_reg(11, (timing.be1 << 5) | timing.bs1);
        vi_write_reg(12, (timing.be4 << 5) | timing.bs4);
        vi_write_reg(13, (timing.be2 << 5) | timing.bs2);
        vi_write_reg(24, 0x1000 | ((timing.nhlines / 2).wrapping_add(1)));
        vi_write_reg(25, timing.hlw.wrapping_add(1));
        vi_write_reg(26, 0x1001);
        vi_write_reg(27, 0x0001);
        vi_write_reg(36, 0x2850);
        vi_write_reg(56, 640);
        for (index, value) in encode_default_filter_registers().into_iter().enumerate() {
            vi_write_reg(38 + index, value);
        }
        vi_write_reg(1, control);
        vi_write_reg(
            54,
            u16::from(platform_timing::vi_mode_is_progressive(vimode)),
        );
        if initialize_retrace {
            RETRACE_COUNT.store(0, Ordering::Release);
        }
        LAST_CURRENT_TV_MODE.store(vimode, Ordering::Release);
        LAST_VIDEO_SCAN_MODE.store(
            platform_timing::vi_scan_mode_for_mode(vimode),
            Ordering::Release,
        );
        if initialize_retrace {
            request_video_retrace_irq();
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn request_video_retrace_irq() {
    let _ = unsafe { irq::request(IRQ_PI_VI, Some(video_retrace_irq), core::ptr::null_mut()) };
    irq::unmask(1 << IRQ_PI_VI);
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn video_retrace_irq(_id: u32, _user: *mut c_void) {
    unsafe {
        for index in [24usize, 26, 28, 30] {
            let status = vi_read_reg(index);
            if status & 0x8000 != 0 {
                vi_write_reg(index, status & !0x8000);
            }
        }
        // C libogc advances its retrace counter for every dispatched VI IRQ.
        // Do the same even when a status bit was already cleared by hardware;
        // otherwise a waiter can sleep forever after a valid retrace interrupt.
        video_retrace_tick();
    }
}

fn video_retrace_tick() {
    let retrace_count = RETRACE_COUNT.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    if let Some(callback) = decode_callback(PRE_RETRACE_CALLBACK_PTR.load(Ordering::Acquire)) {
        unsafe { callback(retrace_count) };
    }
    let flush_pending = FRAMEBUFFER_FLUSH_PENDING.load(Ordering::Acquire);
    let mode_change = FLUSHED_MODE_CHANGE_PENDING.load(Ordering::Acquire);
    let apply_flushed =
        !flush_pending || can_apply_flushed_mode_change(mode_change, current_field_even_odd());
    let flushed = flush_pending && apply_flushed;
    if flushed {
        FRAMEBUFFER_FLUSH_PENDING.store(false, Ordering::Release);
    }
    let mode_pending = flushed && FLUSHED_MODE_PENDING.swap(false, Ordering::AcqRel);
    let black_pending = flushed && FLUSHED_BLACK_PENDING.swap(false, Ordering::AcqRel);
    if mode_pending {
        FLUSHED_MODE_CHANGE_PENDING.store(false, Ordering::Release);
        unsafe {
            let mode = *core::ptr::addr_of!(FLUSHED_MODE);
            DEFAULT_MODE = mode;
            apply_timing_regs(&mode);
            apply_horizontal_regs(&mode);
            apply_picture_config(&mode);
            apply_vertical_regs(&mode, FLUSHED_VIDEO_BLACK.load(Ordering::Acquire));
            let fb = CONFIGURED_FRAMEBUFFER.load(Ordering::Acquire) as *mut c_void;
            program_framebuffer_registers(fb, &mode);
        }
        let mode = unsafe { (*core::ptr::addr_of!(FLUSHED_MODE)).vi_tv_mode };
        LAST_CURRENT_TV_MODE.store(mode, Ordering::Release);
        LAST_VIDEO_SCAN_MODE.store(
            platform_timing::vi_scan_mode_for_mode(mode),
            Ordering::Release,
        );
    } else if black_pending {
        unsafe {
            let mode = &*core::ptr::addr_of!(DEFAULT_MODE);
            apply_vertical_regs(mode, FLUSHED_VIDEO_BLACK.load(Ordering::Acquire));
        }
    }
    let next = FLUSHED_NEXT_FRAMEBUFFER.load(Ordering::Acquire);
    let current = LAST_FRAMEBUFFER.load(Ordering::Acquire);
    if let Some(next) = framebuffer_to_latch(flushed, next, current) {
        #[cfg(target_arch = "powerpc")]
        unsafe {
            let mode = &*core::ptr::addr_of!(DEFAULT_MODE);
            program_framebuffer_registers(next as *mut c_void, mode);
        }
        LAST_FRAMEBUFFER.store(next, Ordering::Release);
    }
    let next_right = FLUSHED_NEXT_RIGHT_FRAMEBUFFER.load(Ordering::Acquire);
    let current_right = LAST_RIGHT_FRAMEBUFFER.load(Ordering::Acquire);
    let scan_mode = LAST_CURRENT_TV_MODE.load(Ordering::Acquire) & 3;
    if let Some(next_right) =
        right_framebuffer_to_latch(flushed, scan_mode, next_right, current_right)
    {
        #[cfg(target_arch = "powerpc")]
        unsafe {
            let mode = &*core::ptr::addr_of!(DEFAULT_MODE);
            program_right_framebuffer_registers(next_right as *mut c_void, mode);
        }
        LAST_RIGHT_FRAMEBUFFER.store(next_right, Ordering::Release);
    }
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let half_line = (vi_read_reg(22) & 0x07ff) as u32;
        let vimode = LAST_CURRENT_TV_MODE.load(Ordering::Acquire);
        if let Some(timing) = platform_timing::vi_timing_for_mode(vimode) {
            let field_start = timing.nhlines as u32;
            let current_field = u32::from(half_line >= field_start);
            LAST_NEXT_FIELD.store(current_field, Ordering::Release);
            LAST_CURRENT_LINE.store(
                if half_line >= field_start {
                    half_line - field_start
                } else {
                    half_line
                } >> 1,
                Ordering::Release,
            );
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_NEXT_FIELD.fetch_xor(1, Ordering::AcqRel);
        LAST_CURRENT_LINE.fetch_add(1, Ordering::AcqRel);
    }
    if let Some(callback) = decode_callback(POST_RETRACE_CALLBACK_PTR.load(Ordering::Acquire)) {
        unsafe { callback(retrace_count) };
    }
    #[cfg(target_arch = "powerpc")]
    {
        let queue = RETRACE_WAIT_QUEUE.load(Ordering::Acquire);
        if queue != lwp::LWP_TQUEUE_NULL {
            // Waiters mask interrupts across the count check and queue entry,
            // as VIDEO_WaitVSync does in C. Broadcast to all registered waiters.
            lwp::thread_broadcast(queue);
        }
    }
}

fn reset_video_init_state() {
    PRE_RETRACE_CALLBACK_PTR.store(0, Ordering::Release);
    POST_RETRACE_CALLBACK_PTR.store(0, Ordering::Release);
    RETRACE_COUNT.store(0, Ordering::Release);
    FRAMEBUFFER_FLUSH_PENDING.store(false, Ordering::Release);
    CONFIGURED_MODE_DIRTY.store(false, Ordering::Release);
    FLUSHED_MODE_PENDING.store(false, Ordering::Release);
    FLUSHED_MODE_CHANGE_PENDING.store(false, Ordering::Release);
    VIDEO_BLACK.store(true, Ordering::Release);
    FLUSHED_VIDEO_BLACK.store(true, Ordering::Release);
    VIDEO_BLACK_DIRTY.store(false, Ordering::Release);
    FLUSHED_BLACK_PENDING.store(false, Ordering::Release);
}

fn current_field_even_odd() -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let vimode = LAST_CURRENT_TV_MODE.load(Ordering::Acquire);
        let Some(timing) = platform_timing::vi_timing_for_mode(vimode) else {
            return false;
        };
        let Some(half_line) = current_half_line(timing.hlw) else {
            return false;
        };
        half_line < u32::from(timing.nhlines)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_NEXT_FIELD.load(Ordering::Acquire) == 0
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn current_half_line(half_line_width: u16) -> Option<u32> {
    unsafe {
        if half_line_width == 0 {
            return None;
        }
        let mut vpos = u32::from(vi_read_reg(22) & 0x07ff);
        let mut hpos;
        loop {
            let previous_vpos = vpos;
            hpos = u32::from(vi_read_reg(23) & 0x07ff);
            vpos = u32::from(vi_read_reg(22) & 0x07ff);
            if previous_vpos == vpos {
                break;
            }
        }
        Some(
            vpos.wrapping_sub(1)
                .wrapping_mul(2)
                .wrapping_add(hpos.wrapping_sub(1) / u32::from(half_line_width)),
        )
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn apply_vertical_regs(rmode: &GxrModeObj, black: bool) {
    unsafe {
        let Some(timing) = platform_timing::vi_timing_for_mode(rmode.vi_tv_mode) else {
            return;
        };
        let mode = rmode.vi_tv_mode;
        let non_interlace = mode & 3 == 1;
        let progressive = mode & 2 != 0;
        let disp_pos_y = (rmode.vi_y_origin as u32) * if non_interlace { 2 } else { 1 };
        let pan_size_y = rmode.xfb_height as u32;
        let disp_size_y = if progressive {
            pan_size_y
        } else if rmode.xfb_mode == 0 {
            pan_size_y << 1
        } else {
            pan_size_y
        };
        let (div1, div2) = if timing.equ >= 10 {
            (1u32, 2u32)
        } else {
            (2u32, 1u32)
        };
        let field = disp_pos_y & 1;
        let adjusted_disp_pos_y = disp_pos_y.max(field);
        let max_disp_size_y = (timing.acv as u32 * 2).saturating_sub(field);
        let end = disp_pos_y + disp_size_y;
        let clipped_end = end.saturating_sub(max_disp_size_y);
        let start_adjustment = if disp_pos_y < field {
            disp_pos_y as i32 - field as i32
        } else {
            0
        };
        let adjusted_disp_size_y =
            (disp_size_y as i32 + start_adjustment - clipped_end as i32) as u32;
        let prb = div2.wrapping_mul(adjusted_disp_pos_y);
        let psb = div2.wrapping_mul(
            ((timing.acv as u32).wrapping_mul(div1))
                .wrapping_sub(adjusted_disp_size_y)
                .wrapping_sub(adjusted_disp_pos_y),
        );
        let mut prb_odd = timing.prb_odd as u32 + prb;
        let mut prb_even = timing.prb_even as u32 + prb;
        let mut psb_odd = timing.psb_odd as u32 + psb;
        let mut psb_even = timing.psb_even as u32 + psb;
        let mut tmp = adjusted_disp_size_y / div1;

        if black {
            prb_odd = prb_odd.wrapping_add((tmp << 1).wrapping_sub(2));
            prb_even = prb_even.wrapping_add((tmp << 1).wrapping_sub(2));
            psb_odd = psb_odd.wrapping_add(2);
            psb_even = psb_even.wrapping_add(2);
            tmp = 0;
        }

        vi_write_reg(0, (((tmp << 4) & !0x0f) | timing.equ as u32) as u16);
        vi_write_reg(6, psb_odd as u16);
        vi_write_reg(7, prb_odd as u16);
        vi_write_reg(8, psb_even as u16);
        vi_write_reg(9, prb_even as u16);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn program_framebuffer_registers(fb: *mut c_void, rmode: &GxrModeObj) {
    unsafe {
        let (top_high, top_low, bottom_high, bottom_low) =
            encode_framebuffer_regs(fb as usize, rmode);
        vi_write_reg(14, top_high);
        vi_write_reg(15, top_low);
        vi_write_reg(18, bottom_high);
        vi_write_reg(19, bottom_low);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn program_right_framebuffer_registers(fb: *mut c_void, rmode: &GxrModeObj) {
    unsafe {
        let (top_high, top_low, bottom_high, bottom_low) =
            encode_framebuffer_regs(fb as usize, rmode);
        vi_write_reg(16, top_high & 0x00ff);
        vi_write_reg(17, top_low);
        vi_write_reg(20, bottom_high & 0x00ff);
        vi_write_reg(21, bottom_low);
    }
}

fn encode_framebuffer_regs(fb: usize, rmode: &GxrModeObj) -> (u16, u16, u16, u16) {
    let word_per_line = rmode.fb_width.wrapping_add(15) / 16;
    let bytes_per_line = (word_per_line as u32) << 5;
    let mut top = mem_virtual_to_physical(fb);
    let mut bottom = top;

    if rmode.xfb_mode == VI_XFBMODE_DF {
        bottom = bottom.wrapping_add(bytes_per_line);
    }

    if (rmode.vi_y_origin & 1) != 0 {
        core::mem::swap(&mut top, &mut bottom);
    }

    let mut flag = 1u32;
    if top < 0x0100_0000 && bottom < 0x0100_0000 {
        flag = 0;
    }
    if flag != 0 {
        top >>= 5;
        bottom >>= 5;
    }

    (
        (((flag & 1) << 12) | ((top >> 16) & 0xff)) as u16,
        (top & 0xffff) as u16,
        (((flag & 1) << 12) | ((bottom >> 16) & 0xff)) as u16,
        (bottom & 0xffff) as u16,
    )
}

fn expected_control_reg(rmode: &GxrModeObj) -> u16 {
    platform_timing::vi_control_register_for_mode(rmode.vi_tv_mode).unwrap_or(0)
}

pub fn get_next_framebuffer() -> *mut c_void {
    LAST_NEXT_FRAMEBUFFER.load(Ordering::Acquire) as *mut c_void
}

pub fn get_current_framebuffer() -> *mut c_void {
    LAST_FRAMEBUFFER.load(Ordering::Acquire) as *mut c_void
}

/// Returns the number of VI retrace interrupts handled since video init.
pub fn retrace_count() -> u32 {
    RETRACE_COUNT.load(Ordering::Acquire)
}

/// Returns the Broadway timer frequency for the detected console.
pub fn timer_clock_hz() -> u64 {
    #[cfg(target_arch = "powerpc")]
    {
        if ogc_core::powerpc::hollywood_revision() != 0 {
            return platform_timing::timer_clock_hz(PlatformClock::Wii);
        }
    }
    platform_timing::timer_clock_hz(PlatformClock::GameCube)
}

pub fn read_register_snapshot() -> VideoRegisterSnapshot {
    unsafe {
        let current_framebuffer = LAST_FRAMEBUFFER.load(Ordering::Acquire);
        let next_framebuffer = LAST_NEXT_FRAMEBUFFER.load(Ordering::Acquire);

        #[cfg(target_arch = "powerpc")]
        {
            VideoRegisterSnapshot {
                control: vi_read_reg(1),
                vertical_timing: vi_read_reg(0),
                top_field_high: vi_read_reg(14),
                top_field_low: vi_read_reg(15),
                bottom_field_high: vi_read_reg(18),
                bottom_field_low: vi_read_reg(19),
                current_framebuffer,
                next_framebuffer,
            }
        }

        #[cfg(not(target_arch = "powerpc"))]
        {
            let mode = &*core::ptr::addr_of!(DEFAULT_MODE);
            let (top_field_high, top_field_low, bottom_field_high, bottom_field_low) =
                if next_framebuffer == 0 {
                    (0, 0, 0, 0)
                } else {
                    encode_framebuffer_regs(next_framebuffer, mode)
                };
            VideoRegisterSnapshot {
                control: expected_control_reg(mode),
                vertical_timing: 0,
                top_field_high,
                top_field_low,
                bottom_field_high,
                bottom_field_low,
                current_framebuffer,
                next_framebuffer,
            }
        }
    }
}

/// Checks whether the VI registers describe the supplied framebuffer and mode.
///
/// # Safety
/// `rmode` must be null or point to a valid initialized mode object. `fb` must
/// be null or identify a framebuffer valid for that mode.
pub unsafe fn framebuffer_registers_match(fb: *mut c_void, rmode: *mut GxrModeObj) -> Bool {
    if fb.is_null() || rmode.is_null() {
        return 0;
    }
    let expected = unsafe { encode_framebuffer_regs(fb as usize, &*rmode) };
    let observed = read_register_snapshot();
    if observed.top_field_high == expected.0
        && observed.top_field_low == expected.1
        && observed.bottom_field_high == expected.2
        && observed.bottom_field_low == expected.3
    {
        1
    } else {
        0
    }
}

/// Checks whether the VI control register matches a mode.
///
/// # Safety
/// `rmode` must be null or point to a valid initialized mode object.
pub unsafe fn control_register_matches(rmode: *mut GxrModeObj) -> Bool {
    if rmode.is_null() {
        return 0;
    }
    let expected = unsafe { expected_control_reg(&*rmode) };
    if read_register_snapshot().control == expected {
        1
    } else {
        0
    }
}

pub fn init() {
    #[cfg(target_arch = "powerpc")]
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    // BUG FIX: C VIDEO_Init clears callbacks, pending register changes, and
    // initializes HorVer.black to true. Reset the matching Rust state on reinit.
    reset_video_init_state();
    #[cfg(target_arch = "powerpc")]
    {
        unsafe {
            let mut display_offset_h = 0;
            if ogc_conf::get_display_offset_h(&mut display_offset_h) != 0 {
                display_offset_h = 0;
            }
            DISPLAY_OFFSET_H.store(display_offset_h as i32 as u32, Ordering::Release);
            let control = vi_read_reg(1);
            let needs_timing_init = vi_needs_timing_init(control);
            let vimode = if needs_timing_init {
                TV_NTSC_480_INT_DF.vi_tv_mode
            } else {
                decode_vi_mode(control, vi_read_reg(54) & 1 != 0)
                    .filter(|mode| platform_timing::vi_timing_for_mode(*mode).is_some())
                    .unwrap_or(TV_NTSC_480_INT_DF.vi_tv_mode)
            };
            if needs_timing_init {
                vi_init(vimode, true);
            } else {
                for (index, value) in encode_default_filter_registers().into_iter().enumerate() {
                    vi_write_reg(38 + index, value);
                }
                vi_write_reg(56, 640);
                RETRACE_COUNT.store(0, Ordering::Release);
                LAST_CURRENT_TV_MODE.store(vimode, Ordering::Release);
                LAST_VIDEO_SCAN_MODE.store(
                    platform_timing::vi_scan_mode_for_mode(vimode),
                    Ordering::Release,
                );
                request_video_retrace_irq();
            }
            LAST_HAVE_COMPONENT_CABLE.store(u32::from(vi_read_reg(55) & 1 != 0), Ordering::Release);
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_CURRENT_TV_MODE.store(TV_NTSC_480_INT_DF.vi_tv_mode, Ordering::Release);
        LAST_VIDEO_SCAN_MODE.store(VI_NON_INTERLACE, Ordering::Release);
    }
}

pub fn flush() {
    if CONFIGURED_MODE_DIRTY.swap(false, Ordering::AcqRel) {
        unsafe {
            FLUSHED_MODE = *core::ptr::addr_of!(CONFIGURED_MODE);
        }
        let active_mode = LAST_CURRENT_TV_MODE.load(Ordering::Acquire);
        let configured_mode = unsafe { (*core::ptr::addr_of!(FLUSHED_MODE)).vi_tv_mode };
        if (active_mode & 0x3) != (configured_mode & 0x3) {
            FLUSHED_MODE_CHANGE_PENDING.store(true, Ordering::Release);
        }
        FLUSHED_MODE_PENDING.store(true, Ordering::Release);
    }
    if VIDEO_BLACK_DIRTY.swap(false, Ordering::AcqRel) {
        FLUSHED_VIDEO_BLACK.store(VIDEO_BLACK.load(Ordering::Acquire), Ordering::Release);
        FLUSHED_BLACK_PENDING.store(true, Ordering::Release);
    }
    let configured = CONFIGURED_FRAMEBUFFER.load(Ordering::Acquire);
    let current_next = LAST_NEXT_FRAMEBUFFER.load(Ordering::Acquire);
    if let Some(next) = framebuffer_to_publish(true, configured, current_next) {
        FLUSHED_NEXT_FRAMEBUFFER.store(next, Ordering::Release);
        LAST_NEXT_FRAMEBUFFER.store(next, Ordering::Release);
    }
    FLUSHED_NEXT_RIGHT_FRAMEBUFFER.store(
        LAST_NEXT_RIGHT_FRAMEBUFFER.load(Ordering::Acquire),
        Ordering::Release,
    );
    FRAMEBUFFER_FLUSH_PENDING.store(true, Ordering::Release);
}

pub fn set_black(black: Bool) {
    let black = black != 0;
    #[cfg(target_arch = "powerpc")]
    {
        // BUG FIX: C VIDEO_SetBlack updates the vertical VI registers at once;
        // it does not wait for VIDEO_Flush or the next retrace.
        let _interrupt_guard = ogc_sys::InterruptGuard::new();
        VIDEO_BLACK.store(black, Ordering::Release);
        VIDEO_BLACK_DIRTY.store(false, Ordering::Release);
        FLUSHED_VIDEO_BLACK.store(black, Ordering::Release);
        FLUSHED_BLACK_PENDING.store(false, Ordering::Release);
        unsafe {
            apply_vertical_regs(&*core::ptr::addr_of!(CONFIGURED_MODE), black);
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        VIDEO_BLACK.store(black, Ordering::Release);
        VIDEO_BLACK_DIRTY.store(true, Ordering::Release);
    }
}

pub fn get_next_field() -> u32 {
    #[cfg(target_arch = "powerpc")]
    // BUG FIX: C samples the current field with interrupts disabled, so a
    // retrace cannot change VI timing during the read.
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    let mode = unsafe { &*core::ptr::addr_of!(CONFIGURED_MODE) };
    let adjusted_y_origin = adjusted_display_y_origin(mode.vi_tv_mode, mode.vi_y_origin);
    next_field_after_current(current_field_even_odd(), adjusted_y_origin)
}

pub fn get_current_line() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        // BUG FIX: C protects the current half-line read from the VI retrace
        // handler changing timing at the same time.
        let _interrupt_guard = ogc_sys::InterruptGuard::new();
        let mode = LAST_CURRENT_TV_MODE.load(Ordering::Acquire);
        if let Some(timing) = platform_timing::vi_timing_for_mode(mode)
            && let Some(half_line) = unsafe { current_half_line(timing.hlw) }
        {
            return line_from_half_line(half_line, u32::from(timing.nhlines));
        }
    }
    LAST_CURRENT_LINE.load(Ordering::Acquire)
}

pub fn get_current_tv_mode() -> u32 {
    television_standard(LAST_CURRENT_TV_MODE.load(Ordering::Acquire))
}

/// Configures VI from a mode object.
///
/// # Safety
/// `rmode` must be null or point to a valid mode object for the duration of the call.
pub unsafe fn configure(rmode: *mut GxrModeObj) {
    unsafe {
        if !rmode.is_null() && platform_timing::vi_timing_for_mode((*rmode).vi_tv_mode).is_some() {
            CONFIGURED_MODE = *rmode;
            CONFIGURED_MODE_DIRTY.store(true, Ordering::Release);
        }
    }
}

/// Returns the framebuffer size required by a mode.
///
/// # Safety
/// `rmode` must be null or point to a valid initialized mode object.
pub unsafe fn get_frame_buffer_size(rmode: *mut GxrModeObj) -> u32 {
    unsafe {
        if rmode.is_null() {
            return 0;
        }
        let width = ((*rmode).fb_width.wrapping_add(15)) & !15;
        let height = (*rmode).xfb_height as u32 + if (*rmode).aa != 0 { 4 } else { 0 };
        width as u32 * height * 2
    }
}

/// Clears an entire framebuffer.
///
/// # Safety
/// `rmode` must describe `fb`, and `fb` must be writable for at least the size
/// returned by [`get_frame_buffer_size`]. No other access may overlap the write.
pub unsafe fn clear_frame_buffer(rmode: *mut GxrModeObj, fb: *mut c_void, color: u32) {
    unsafe {
        if rmode.is_null() || fb.is_null() {
            return;
        }
        let size = get_frame_buffer_size(rmode) as usize;
        let word_count = size / core::mem::size_of::<u32>();
        let pixels = core::slice::from_raw_parts_mut(fb.cast::<u32>(), word_count);
        pixels.fill(color);
    }
}

pub fn wait_v_sync() {
    #[cfg(target_arch = "powerpc")]
    {
        let queue = retrace_wait_queue();
        let previous = RETRACE_COUNT.load(Ordering::Acquire);
        while retrace_is_unchanged(previous, RETRACE_COUNT.load(Ordering::Acquire)) {
            if queue == lwp::LWP_TQUEUE_NULL {
                lwp::yield_thread();
            } else {
                match retrace_wait::sleep_if_unchanged(
                    previous,
                    ogc_sys::InterruptGuard::new,
                    || RETRACE_COUNT.load(Ordering::Acquire),
                    || {
                        let result = lwp::thread_sleep(queue);
                        if result == lwp::LWP_SUCCESSFUL {
                            Ok(())
                        } else {
                            Err(result)
                        }
                    },
                ) {
                    Ok(true) => break,
                    Ok(false) => {}
                    // A closed queue cannot provide a wake. Poll real VI
                    // interrupts after the critical section has ended.
                    Err(_) => while !wait_v_sync_timeout_us(100_000) {},
                }
            }
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        video_retrace_tick();
    }
}

/// Poll the retrace counter for a bounded time without sleeping on the LWP queue.
///
/// This is useful for diagnosing a failed interrupt wake path. A `false` result
/// means no VI interrupt advanced the counter before the timeout.
pub fn wait_v_sync_timeout_us(timeout_us: u64) -> bool {
    #[cfg(target_arch = "powerpc")]
    {
        let previous = RETRACE_COUNT.load(Ordering::Acquire);
        let started = ogc_core::powerpc::read_timebase();
        let clock_hz = timer_clock_hz();
        loop {
            if !retrace_is_unchanged(previous, RETRACE_COUNT.load(Ordering::Acquire)) {
                return true;
            }
            let elapsed = ogc_core::powerpc::read_timebase().wrapping_sub(started);
            if ticks_to_microseconds(elapsed, clock_hz) >= timeout_us {
                return false;
            }
            core::hint::spin_loop();
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = timeout_us;
        video_retrace_tick();
        true
    }
}

fn retrace_is_unchanged(previous: u32, current: u32) -> bool {
    current == previous
}

#[cfg(target_arch = "powerpc")]
fn retrace_wait_queue() -> lwp::LwpQueue {
    let current = RETRACE_WAIT_QUEUE.load(Ordering::Acquire);
    if current != lwp::LWP_TQUEUE_NULL {
        return current;
    }

    let mut candidate = lwp::LWP_TQUEUE_NULL;
    if unsafe { lwp::init_queue(&mut candidate) } != lwp::LWP_SUCCESSFUL {
        return lwp::LWP_TQUEUE_NULL;
    }
    match RETRACE_WAIT_QUEUE.compare_exchange(
        lwp::LWP_TQUEUE_NULL,
        candidate,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => candidate,
        Err(existing) => {
            lwp::close_queue(candidate);
            existing
        }
    }
}

/// Sets the current framebuffer address.
///
/// # Safety
/// A non-null `fb` must remain a valid framebuffer while VI may scan it out.
pub unsafe fn set_framebuffer(fb: *mut c_void) {
    unsafe {
        CONFIGURED_FRAMEBUFFER.store(fb as usize, Ordering::Release);
        #[cfg(target_arch = "powerpc")]
        {
            let current_mode = &*core::ptr::addr_of!(CONFIGURED_MODE);
            program_framebuffer_registers(fb, current_mode);
        }
    }
}

/// Schedules the next framebuffer address.
///
/// # Safety
/// A non-null `fb` must remain a valid framebuffer while VI may scan it out.
pub unsafe fn set_next_framebuffer(fb: *mut c_void) {
    CONFIGURED_FRAMEBUFFER.store(fb as usize, Ordering::Release);
}

/// Schedules the right-eye framebuffer address.
///
/// # Safety
/// A non-null `fb` must remain a valid framebuffer while VI may scan it out.
pub unsafe fn set_next_right_framebuffer(fb: *mut c_void) {
    LAST_NEXT_RIGHT_FRAMEBUFFER.store(fb as usize, Ordering::Release);
}

pub fn set_pre_retrace_callback(callback: ViRetraceCallback) -> ViRetraceCallback {
    let previous =
        PRE_RETRACE_CALLBACK_PTR.swap(callback.map_or(0, |cb| cb as usize), Ordering::AcqRel);
    decode_callback(previous)
}

pub fn set_post_retrace_callback(callback: ViRetraceCallback) -> ViRetraceCallback {
    let previous =
        POST_RETRACE_CALLBACK_PTR.swap(callback.map_or(0, |cb| cb as usize), Ordering::AcqRel);
    decode_callback(previous)
}

pub fn have_component_cable() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        u32::from(vi_read_reg(55) & 1 != 0)
    }
    #[cfg(not(target_arch = "powerpc"))]
    LAST_HAVE_COMPONENT_CABLE.load(Ordering::Acquire)
}

fn gamecube_preferred_mode(tv_mode: u32, progressive: bool) -> GxrModeObj {
    match (tv_mode, progressive) {
        (0, true) => PREFERRED_NTSC_PROGRESSIVE,
        (1, true) => PREFERRED_PAL_PROGRESSIVE,
        (2, true) => PREFERRED_MPAL_PROGRESSIVE,
        (5, true) => PREFERRED_EURGB60_PROGRESSIVE,
        (0, false) => PREFERRED_NTSC_INTERLACED,
        (1, false) => PREFERRED_PAL_INTERLACED,
        (2, false) => PREFERRED_MPAL_INTERLACED,
        (5, false) => PREFERRED_EURGB60_INTERLACED,
        _ => PREFERRED_NTSC_INTERLACED,
    }
}

/// Copies or returns the preferred video mode.
///
/// # Safety
/// `mode` must be null or point to writable, properly aligned storage for one
/// [`GxrModeObj`]. A null argument returns library-owned mutable state.
pub unsafe fn get_preferred_mode(mode: *mut GxrModeObj) -> *mut GxrModeObj {
    unsafe {
        let selected = if ogc_core::powerpc::hollywood_revision() == 0 {
            // BUG FIX: C selects GameCube modes from the active VI standard
            // and cable. Using Wii CONF values selected the wrong GC mode.
            gamecube_preferred_mode(get_current_tv_mode(), have_component_cable() != 0)
        } else {
            let video = ogc_conf::get_video();
            let progressive = ogc_conf::get_progressive_scan() > 0 && have_component_cable() != 0;
            let eu_rgb60 = ogc_conf::get_eu_rgb60() > 0;
            if progressive {
                if video == ogc_conf::CONF_VIDEO_PAL as i32 {
                    if eu_rgb60 {
                        PREFERRED_EURGB60_PROGRESSIVE
                    } else {
                        PREFERRED_PAL_PROGRESSIVE
                    }
                } else if video == ogc_conf::CONF_VIDEO_MPAL as i32 {
                    PREFERRED_MPAL_PROGRESSIVE
                } else {
                    PREFERRED_NTSC_PROGRESSIVE
                }
            } else {
                if video == ogc_conf::CONF_VIDEO_PAL as i32 {
                    if eu_rgb60 {
                        PREFERRED_EURGB60_INTERLACED
                    } else {
                        PREFERRED_PAL_INTERLACED
                    }
                } else if video == ogc_conf::CONF_VIDEO_MPAL as i32 {
                    PREFERRED_MPAL_INTERLACED
                } else {
                    PREFERRED_NTSC_INTERLACED
                }
            }
        };
        if mode.is_null() {
            PREFERRED_MODE = selected;
            core::ptr::addr_of_mut!(PREFERRED_MODE)
        } else {
            *mode = selected;
            mode
        }
    }
}

pub fn get_video_scan_mode() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        // BUG FIX: C reads the current VI registers. The cached value could be
        // stale after another component changed the scan mode.
        // Keep the register pair consistent if a VI interrupt occurs.
        let _interrupt_guard = ogc_sys::InterruptGuard::new();
        unsafe { video_scan_mode_from_registers(vi_read_reg(54), vi_read_reg(1)) }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        LAST_VIDEO_SCAN_MODE.load(Ordering::Acquire)
    }
}

fn decode_callback(ptr: usize) -> ViRetraceCallback {
    if ptr == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(u32)>(ptr) })
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn exported_standard_modes_match_c_table_fields() {
        assert_eq!(TV_NTSC_480_PROG.vi_tv_mode, 2);
        assert_eq!(
            (TV_NTSC_480_PROG.fb_width, TV_NTSC_480_PROG.efb_height),
            (640, 480)
        );
        assert_eq!(TV_NTSC_480_PROG.xfb_mode, 0);
        assert_eq!(TV_NTSC_480_PROG.vfilter, [0, 0, 21, 22, 21, 0, 0]);

        assert_eq!(TV_EURGB60_480_PROG.vi_tv_mode, 22);
        assert_eq!(TV_EURGB60_480_PROG.xfb_mode, 0);
        assert_eq!(TV_EURGB60_480_PROG.vfilter, [0, 0, 21, 22, 21, 0, 0]);

        assert_eq!(TV_EURGB60_480_INT_DF.vi_tv_mode, 20);
        assert_eq!(TV_EURGB60_480_INT_DF.xfb_mode, VI_XFBMODE_DF);
        assert_eq!(TV_EURGB60_480_INT_DF.vfilter, [8, 8, 10, 12, 10, 8, 8]);

        assert_eq!(TV_PAL_528_INT_DF.vi_tv_mode, 4);
        assert_eq!(
            (
                TV_PAL_528_INT_DF.efb_height,
                TV_PAL_528_INT_DF.xfb_height,
                TV_PAL_528_INT_DF.vi_height,
                TV_PAL_528_INT_DF.vi_y_origin,
            ),
            (528, 528, 528, 24)
        );
        assert_eq!(TV_PAL_528_INT_DF.xfb_mode, VI_XFBMODE_DF);
    }

    unsafe extern "C" fn retrace_test_callback(_: u32) {}

    #[test]
    fn wait_rechecks_retrace_count_after_queue_wakeup() {
        let before = u32::MAX;
        assert!(retrace_is_unchanged(before, before));
        assert!(!retrace_is_unchanged(before, before.wrapping_add(1)));
    }

    #[test]
    fn high_memory_framebuffer_sets_address_flag_on_both_fields() {
        let regs = encode_framebuffer_regs(0xc100_0000, &TV_NTSC_480_INT_DF);

        assert_eq!(regs, (0x1008, 0x0000, 0x1008, 0x0028));
    }

    #[test]
    fn low_memory_framebuffer_does_not_set_address_flag() {
        let regs = encode_framebuffer_regs(0xc000_2000, &TV_NTSC_480_INT_DF);

        assert_eq!(regs, (0x0000, 0x2000, 0x0000, 0x2500));
    }

    #[test]
    fn framebuffer_changes_latch_only_after_flush_and_retrace() {
        assert_eq!(
            framebuffer_to_publish(false, 0xc000_2000, 0xc000_1000),
            None
        );
        assert_eq!(
            framebuffer_to_publish(true, 0xc000_2000, 0xc000_1000),
            Some(0xc000_2000)
        );
        assert_eq!(framebuffer_to_publish(true, 0xc000_1000, 0xc000_1000), None);
        assert_eq!(framebuffer_to_latch(false, 0xc000_2000, 0xc000_1000), None);
        assert_eq!(
            framebuffer_to_latch(true, 0xc000_2000, 0xc000_1000),
            Some(0xc000_2000)
        );
        assert_eq!(framebuffer_to_latch(true, 0xc000_1000, 0xc000_1000), None);
        assert_eq!(framebuffer_to_latch(true, 0, 0xc000_1000), Some(0));
    }

    #[test]
    fn right_framebuffer_latches_only_for_flushed_progressive_3d_mode() {
        assert_eq!(
            right_framebuffer_to_latch(true, 3, 0xc000_2000, 0xc000_1000),
            Some(0xc000_2000)
        );
        assert_eq!(right_framebuffer_to_latch(false, 3, 0xc000_2000, 0), None);
        assert_eq!(right_framebuffer_to_latch(true, 1, 0xc000_2000, 0), None);
    }

    #[test]
    fn active_vi_control_decodes_supported_scan_modes() {
        assert_eq!(decode_vi_mode(0x0101, false), Some(4));
        assert_eq!(decode_vi_mode(0x0105, false), Some(5));
        assert_eq!(decode_vi_mode(0x0105, true), Some(6));
        assert_eq!(decode_vi_mode(0x0501, false), Some(4));
        assert_eq!(decode_vi_mode(0x0000, false), None);
    }

    #[test]
    fn video_scan_mode_matches_vi_register_priority() {
        assert_eq!(video_scan_mode_from_registers(1, 0), VI_PROGRESSIVE);
        assert_eq!(video_scan_mode_from_registers(0, 0x0004), VI_NON_INTERLACE);
        assert_eq!(video_scan_mode_from_registers(0, 0), VI_INTERLACE);
    }

    #[test]
    fn adjusted_noninterlace_origin_matches_c_u16_wrapping() {
        assert_eq!(adjusted_display_y_origin(1, 0x8001), 2);
        assert_eq!(adjusted_display_y_origin(0, 0x8001), 0x8001);
    }

    #[test]
    fn gamecube_preferred_mode_uses_active_standard_and_component_cable() {
        assert_eq!(
            gamecube_preferred_mode(0, true).vi_tv_mode,
            PREFERRED_NTSC_PROGRESSIVE.vi_tv_mode
        );
        assert_eq!(
            gamecube_preferred_mode(1, true).vi_tv_mode,
            PREFERRED_PAL_PROGRESSIVE.vi_tv_mode
        );
        assert_eq!(
            gamecube_preferred_mode(5, true).vi_tv_mode,
            PREFERRED_EURGB60_PROGRESSIVE.vi_tv_mode
        );
        assert_eq!(
            gamecube_preferred_mode(2, false).vi_tv_mode,
            PREFERRED_MPAL_INTERLACED.vi_tv_mode
        );
    }

    #[test]
    fn initialized_vi_keeps_its_existing_timing_registers() {
        assert!(vi_needs_timing_init(0));
        assert!(!vi_needs_timing_init(0x0001));
    }

    #[test]
    fn init_resets_retrace_callbacks_and_counter() {
        set_pre_retrace_callback(Some(retrace_test_callback));
        set_post_retrace_callback(Some(retrace_test_callback));
        RETRACE_COUNT.store(5, Ordering::Release);
        FRAMEBUFFER_FLUSH_PENDING.store(true, Ordering::Release);
        CONFIGURED_MODE_DIRTY.store(true, Ordering::Release);
        FLUSHED_MODE_PENDING.store(true, Ordering::Release);
        FLUSHED_MODE_CHANGE_PENDING.store(true, Ordering::Release);
        VIDEO_BLACK.store(false, Ordering::Release);
        FLUSHED_VIDEO_BLACK.store(false, Ordering::Release);
        VIDEO_BLACK_DIRTY.store(true, Ordering::Release);
        FLUSHED_BLACK_PENDING.store(true, Ordering::Release);

        reset_video_init_state();

        assert!(set_pre_retrace_callback(None).is_none());
        assert!(set_post_retrace_callback(None).is_none());
        assert_eq!(RETRACE_COUNT.load(Ordering::Acquire), 0);
        assert!(!FRAMEBUFFER_FLUSH_PENDING.load(Ordering::Acquire));
        assert!(!CONFIGURED_MODE_DIRTY.load(Ordering::Acquire));
        assert!(!FLUSHED_MODE_PENDING.load(Ordering::Acquire));
        assert!(!FLUSHED_MODE_CHANGE_PENDING.load(Ordering::Acquire));
        assert!(VIDEO_BLACK.load(Ordering::Acquire));
        assert!(FLUSHED_VIDEO_BLACK.load(Ordering::Acquire));
        assert!(!VIDEO_BLACK_DIRTY.load(Ordering::Acquire));
        assert!(!FLUSHED_BLACK_PENDING.load(Ordering::Acquire));
    }

    #[test]
    fn default_filter_taps_match_libogc_register_packing() {
        assert_eq!(
            encode_default_filter_registers(),
            [
                0x1ae7, 0x71f0, 0x0db4, 0xa574, 0x00c1, 0x188e, 0xc4c0, 0xcbe2, 0xfcec, 0xdecf,
                0x1313, 0x0f08, 0x0108, 0x0c0f,
            ]
        );
    }
}
