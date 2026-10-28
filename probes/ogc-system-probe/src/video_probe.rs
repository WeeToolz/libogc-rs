use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use ogc_core::mem_k0_to_k1;
use ogc_system::{self as system};
use ogc_video as video;

use crate::probe_support::ProbeLogger;

static PRE_COUNT: AtomicU32 = AtomicU32::new(0);
static POST_COUNT: AtomicU32 = AtomicU32::new(0);
static CALLBACK_PHASE: AtomicU32 = AtomicU32::new(0);
static PREVIOUS_PRE_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static PREVIOUS_POST_CALLBACK: AtomicUsize = AtomicUsize::new(0);

static mut FRAMEBUFFER_A: *mut c_void = null_mut();
static mut FRAMEBUFFER_B: *mut c_void = null_mut();
static mut DRAW_FRAMEBUFFER: *mut c_void = null_mut();
static mut DISPLAY_FRAMEBUFFER: *mut c_void = null_mut();
static mut DISPLAY_MODE: *mut video::GxrModeObj = null_mut();

#[repr(align(32))]
struct ProbeFifoStorage([u8; 65536]);
static mut PROBE_FIFO_STORAGE: ProbeFifoStorage = ProbeFifoStorage([0; 65536]);

const COLOR_BG: u32 = 0x0080_0080;
const COLOR_PANEL: u32 = 0x515A_51F0;
const COLOR_TEXT: u32 = 0xEB80_EB80;
const RETRACE_WAIT_TIMEOUT_US: u64 = 100_000;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("VIDEO PROBE");
    probe_texture_descriptors(logger);
    probe_fifo_pointers(logger);

    let rmode = init_display(logger);
    logger.record("video init display", !rmode.is_null());
    if rmode.is_null() {
        return;
    }

    let initial_proof = unsafe { video::hardware_proof(rmode, DISPLAY_FRAMEBUFFER) };
    logger.record(
        "video control register readback",
        initial_proof.control_register_matches,
    );
    logger.record(
        "video framebuffer register readback",
        initial_proof.next_framebuffer_registers_match,
    );
    let expected_scan_mode = unsafe { u32::from((*rmode).vi_tv_mode & 0x3 != 0) };
    logger.record(
        "video scan mode matches selected VI mode",
        video::get_video_scan_mode() == expected_scan_mode,
    );
    check_selected_mode_retrace(logger, unsafe { (*rmode).vi_tv_mode });

    let previous_pre = video::set_pre_retrace_callback(Some(pre_callback));
    let previous_post = video::set_post_retrace_callback(Some(post_callback));
    PREVIOUS_PRE_CALLBACK.store(callback_to_usize(previous_pre), Ordering::Release);
    PREVIOUS_POST_CALLBACK.store(callback_to_usize(previous_post), Ordering::Release);

    PRE_COUNT.store(0, Ordering::Release);
    POST_COUNT.store(0, Ordering::Release);
    CALLBACK_PHASE.store(0, Ordering::Release);
    let retrace_before = video::retrace_count();
    let retrace_arrived = video::wait_vsync_timeout_us(RETRACE_WAIT_TIMEOUT_US);
    let retrace_after = video::retrace_count();
    logger.note_u64("VI retrace count before wait", retrace_before as u64);
    logger.note_u64("VI retrace count after wait", retrace_after as u64);
    logger.record(
        "VI retrace interrupt advances counter",
        retrace_arrived && retrace_after != retrace_before,
    );
    logger.record(
        "pre retrace callback fired",
        PRE_COUNT.load(Ordering::Acquire) > 0,
    );
    logger.record(
        "post retrace callback fired",
        POST_COUNT.load(Ordering::Acquire) > 0,
    );
    logger.record(
        "retrace callback order",
        CALLBACK_PHASE.load(Ordering::Acquire) == 2,
    );

    let register_snapshot = video::read_register_snapshot();
    logger.record(
        "video next framebuffer observable",
        register_snapshot.next_framebuffer != 0,
    );
    logger.record(
        "component cable state sane",
        video::have_component_cable() <= 1,
    );

    draw_rect(24, 24, 560, 44, COLOR_PANEL);
    draw_rect(24, 84, 560, 44, COLOR_TEXT);
    logger.record(
        "framebuffer draw memory readback",
        framebuffer_word(24, 24) == Some(COLOR_PANEL)
            && framebuffer_word(24, 84) == Some(COLOR_TEXT),
    );

    let swap_retrace_arrived = swap_framebuffers();
    let swap_proof = unsafe { video::hardware_proof(rmode, DISPLAY_FRAMEBUFFER) };
    let display_framebuffer = unsafe { DISPLAY_FRAMEBUFFER };
    logger.record(
        "framebuffer swap register readback",
        swap_retrace_arrived
            && swap_proof.next_framebuffer_registers_match
            && video::get_current_framebuffer() == display_framebuffer,
    );

    logger.record(
        "console write video banner",
        crate::probe_support::screen_write(b"ogc-system-probe video\n").is_ok(),
    );
    logger.record(
        "console write video footer",
        crate::probe_support::screen_write(b"video probe validated retrace and swap behavior\n")
            .is_ok(),
    );

    let _ = video::set_pre_retrace_callback(previous_pre);
    let _ = video::set_post_retrace_callback(previous_post);
    PREVIOUS_PRE_CALLBACK.store(0, Ordering::Release);
    PREVIOUS_POST_CALLBACK.store(0, Ordering::Release);
}

fn probe_fifo_pointers(logger: &mut ProbeLogger) {
    use ogc_gx::{self as gx, GxFifoObj};

    // SAFETY: This probe exclusively owns the static FIFO storage.
    let base = unsafe { core::ptr::addr_of_mut!(PROBE_FIFO_STORAGE.0).cast::<u8>() };
    let mut fifo = GxFifoObj::default();
    let mut read = null_mut();
    let mut write = null_mut();
    logger.report_step("GX wrapped FIFO pointer metadata");
    // SAFETY: The aligned static storage is exclusively used by this metadata
    // check. Both pointers are within its 64 KiB range; GX is not started.
    unsafe {
        gx::init_fifo_base(&mut fifo, base.cast(), 65536);
        gx::init_fifo_ptrs(&mut fifo, base.add(0xf000).cast(), base.add(0x1000).cast());
        gx::get_fifo_ptrs(&fifo, &mut read, &mut write);
    }
    logger.record(
        "GX wrapped FIFO keeps real pointer addresses and distance",
        read == unsafe { base.add(0xf000).cast() }
            && write == unsafe { base.add(0x1000).cast() }
            && unsafe { gx::get_fifo_count(&fifo) } == 0x2000,
    );
    logger.report_step("GX invalid FIFO pointer rejection");
    // SAFETY: The one-past-end pointer is compared as a value and is not read.
    unsafe {
        gx::init_fifo_ptrs(&mut fifo, base.add(65536).cast(), base.add(0x1000).cast());
        gx::get_fifo_ptrs(&fifo, &mut read, &mut write);
    }
    logger.record(
        "GX invalid FIFO pointer leaves prior metadata unchanged",
        read == unsafe { base.add(0xf000).cast() }
            && write == unsafe { base.add(0x1000).cast() }
            && unsafe { gx::get_fifo_count(&fifo) } == 0x2000,
    );
}

fn probe_texture_descriptors(logger: &mut ProbeLogger) {
    use core::ffi::c_void;
    use core::mem::MaybeUninit;

    const TEX_WRAP_CLAMP: u8 = 0;
    use ogc_gx::{self as gx, GX_TF_RGB565, GxTexObj};

    let mut image = [0u8; 32 * 32 * 2];
    let mut tex = MaybeUninit::<GxTexObj>::uninit();
    let mut user_data = 0x4758_5052usize;
    let tex_ptr = tex.as_mut_ptr();

    // SAFETY: `tex_ptr` points to writable storage for one GX texture object.
    // The image pointer is stored as a descriptor address and is not read here.
    unsafe {
        gx::init_tex_obj(
            tex_ptr,
            image.as_mut_ptr().cast::<c_void>(),
            gx::GxTexObjInit {
                wd: 32,
                ht: 32,
                fmt: GX_TF_RGB565,
                wrap_s: TEX_WRAP_CLAMP,
                wrap_t: TEX_WRAP_CLAMP,
                mipmap: 0,
            },
        );
    }
    // SAFETY: `init_tex_obj` initializes the complete opaque descriptor.
    let tex = unsafe { tex.assume_init() };
    let tex_ptr = core::ptr::addr_of!(tex);

    // SAFETY: `tex` is initialized and remains live for each descriptor read.
    let initial = unsafe { gx::tex_obj_snapshot(tex_ptr) };
    let dimensions_match = unsafe {
        gx::get_tex_obj_width(tex_ptr) == 32
            && gx::get_tex_obj_height(tex_ptr) == 32
            && gx::get_tex_obj_fmt(tex_ptr) == u32::from(GX_TF_RGB565)
    };
    logger.record(
        "GX texture descriptor dimensions and format",
        initial.is_some_and(|value| value.width == 32 && value.height == 32) && dimensions_match,
    );

    // SAFETY: `tex_ptr` is writable and valid. `user_data` stays live through
    // the following snapshot and comparison.
    unsafe {
        gx::init_tex_obj_wrap_mode(tex_ptr.cast_mut(), TEX_WRAP_CLAMP, TEX_WRAP_CLAMP);
        gx::init_tex_obj_user_data(tex_ptr.cast_mut(), (&mut user_data as *mut usize).cast());
    }
    // SAFETY: `tex` remains a valid initialized descriptor.
    let configured = unsafe { gx::tex_obj_snapshot(tex_ptr) };
    logger.record(
        "GX texture descriptor wrap and user data",
        configured.is_some_and(|value| {
            value.wrap_s == TEX_WRAP_CLAMP
                && value.wrap_t == TEX_WRAP_CLAMP
                && value.user_data == (&mut user_data as *mut usize).cast::<c_void>()
        }) && unsafe {
            gx::get_tex_obj_wrap_s(tex_ptr) == TEX_WRAP_CLAMP
                && gx::get_tex_obj_wrap_t(tex_ptr) == TEX_WRAP_CLAMP
        },
    );

    logger.report_step("GX display copy scale valid dimensions");
    let y_scale = gx::get_y_scale_factor(480, 360);
    logger.record(
        "GX display copy scale ratio",
        (y_scale - 0.75).abs() < f32::EPSILON,
    );
    logger.report_step("GX display copy scale unsupported dimensions");
    logger.record(
        "GX display copy scale rejects unsupported dimensions",
        gx::get_y_scale_factor(480, 240) == 1.0,
    );
}

fn check_selected_mode_retrace(logger: &mut ProbeLogger, mode: u32) {
    let Some(standard) = video::video_standard_for_mode(mode) else {
        logger.record("VI mode has a known field rate", false);
        return;
    };

    let clock_hz = video::timer_clock_hz();
    let platform = if clock_hz == 60_750_000 {
        video::PlatformClock::Wii
    } else {
        video::PlatformClock::GameCube
    };
    let expected_ticks = video::retrace_period_ticks(platform, standard);
    const SAMPLES: u64 = 8;
    let mut total_ticks = 0u64;
    for _ in 0..SAMPLES {
        let start = system::time();
        if !video::wait_vsync_timeout_us(RETRACE_WAIT_TIMEOUT_US) {
            logger.note_u64("VI retrace count at timeout", video::retrace_count() as u64);
            logger.record("VI wait matches selected mode timing", false);
            return;
        }
        total_ticks = total_ticks.saturating_add(system::time().wrapping_sub(start));
    }
    let average_ticks = total_ticks / SAMPLES;
    let delta = average_ticks.abs_diff(expected_ticks);
    let within_tolerance = delta.saturating_mul(100) <= expected_ticks.saturating_mul(8);

    logger.note_u64("VI selected mode", mode as u64);
    logger.note_u64("VI timing platform clock Hz", clock_hz);
    logger.note_u64(
        "VI expected retrace us",
        video::ticks_to_microseconds(expected_ticks, clock_hz),
    );
    logger.note_u64(
        "VI measured retrace average us",
        video::ticks_to_microseconds(average_ticks, clock_hz),
    );
    logger.record("VI wait matches selected mode timing", within_tolerance);
}

fn init_display(logger: &mut ProbeLogger) -> *mut video::GxrModeObj {
    video::init();

    // SAFETY: null requests the library-owned preferred mode object.
    let rmode = unsafe { video::get_preferred_mode(null_mut()) };
    if rmode.is_null() {
        return null_mut();
    }

    unsafe {
        FRAMEBUFFER_A = system::allocate_framebuffer(rmode);
        FRAMEBUFFER_B = system::allocate_framebuffer(rmode);
        if FRAMEBUFFER_A.is_null() || FRAMEBUFFER_B.is_null() {
            return null_mut();
        }

        DISPLAY_FRAMEBUFFER = FRAMEBUFFER_A;
        DRAW_FRAMEBUFFER = FRAMEBUFFER_B;
        DISPLAY_MODE = rmode;
    }

    check_framebuffer_cache_coherence(logger, rmode);
    clear_framebuffers(rmode, COLOR_BG);

    // SAFETY: `rmode` was returned by the video subsystem and remains live.
    unsafe { video::configure(rmode) };
    unsafe { video::set_next_framebuffer(DISPLAY_FRAMEBUFFER) };
    video::set_black(false);
    video::flush();
    for wait_number in 1..=2 {
        let retrace_before = video::retrace_count();
        let retrace_arrived = video::wait_vsync_timeout_us(RETRACE_WAIT_TIMEOUT_US);
        let retrace_after = video::retrace_count();
        logger.note_u64("VI initial retrace wait number", wait_number);
        logger.note_u64("VI initial retrace count before", retrace_before as u64);
        logger.note_u64("VI initial retrace count after", retrace_after as u64);
        if !retrace_arrived || retrace_after == retrace_before {
            logger.record("VI retrace interrupt arrives after flush", false);
            return null_mut();
        }
    }
    logger.record("VI retrace interrupt arrives after flush", true);

    rmode
}

fn check_framebuffer_cache_coherence(logger: &mut ProbeLogger, rmode: *mut video::GxrModeObj) {
    // Exercise the production allocator and both aliases on real hardware.
    // A dirty cached zero fill must not overwrite a later uncached clear.
    unsafe {
        let size = video::get_frame_buffer_size(rmode);
        for (label, framebuffer) in [
            ("XFB A survives cached alias flush", FRAMEBUFFER_A),
            ("XFB B survives cached alias flush", FRAMEBUFFER_B),
        ] {
            let uncached = mem_k0_to_k1(framebuffer as usize) as *mut c_void;
            video::clear_frame_buffer(rmode, uncached, COLOR_PANEL);
            ogc_cache::dc_flush_range(framebuffer.cast_const(), size);
            let words = uncached.cast::<u32>();
            let matches = (0..size as usize / core::mem::size_of::<u32>())
                .all(|index| words.add(index).read_volatile() == COLOR_PANEL);
            logger.record(label, matches);
        }
    }
}

fn clear_framebuffers(rmode: *mut video::GxrModeObj, color: u32) {
    if rmode.is_null() {
        return;
    }

    unsafe {
        if DISPLAY_FRAMEBUFFER.is_null() || DRAW_FRAMEBUFFER.is_null() {
            return;
        }

        // SYS_AllocateFramebuffer returns a cached K0 address. VI reads the
        // uncached K1 alias, so clear both buffers through that alias before
        // the first framebuffer is scheduled.
        let display = mem_k0_to_k1(DISPLAY_FRAMEBUFFER as usize) as *mut c_void;
        let draw = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *mut c_void;
        video::clear_frame_buffer(rmode, display, color);
        video::clear_frame_buffer(rmode, draw, color);
    }
}

fn swap_framebuffers() -> bool {
    unsafe {
        if DRAW_FRAMEBUFFER.is_null() || DISPLAY_FRAMEBUFFER.is_null() {
            return false;
        }

        core::ptr::swap(
            core::ptr::addr_of_mut!(DRAW_FRAMEBUFFER),
            core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER),
        );
    }

    unsafe { video::set_next_framebuffer(DISPLAY_FRAMEBUFFER) };
    video::flush();
    video::wait_vsync_timeout_us(RETRACE_WAIT_TIMEOUT_US)
}

fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    unsafe {
        let rmode = DISPLAY_MODE;
        if rmode.is_null() || DRAW_FRAMEBUFFER.is_null() {
            return;
        }

        let fb_width = (*rmode).fb_width as usize;
        let xfb_height = (*rmode).xfb_height as usize;
        if fb_width == 0 || xfb_height == 0 {
            return;
        }

        let x0 = x.min(fb_width.saturating_sub(1)) & !1;
        let y0 = y.min(xfb_height.saturating_sub(1));
        let x1 = x.saturating_add(w).min(fb_width) & !1;
        let y1 = y.saturating_add(h).min(xfb_height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let uncached = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *mut u32;
        let pair_width = fb_width / 2;

        for yy in y0..y1 {
            let row = yy * pair_width;
            let mut xx = x0;
            while xx < x1 {
                *uncached.add(row + xx / 2) = color;
                xx += 2;
            }
        }
    }
}

fn framebuffer_word(x: usize, y: usize) -> Option<u32> {
    unsafe {
        let rmode = DISPLAY_MODE;
        if rmode.is_null() || DRAW_FRAMEBUFFER.is_null() {
            return None;
        }
        let fb_width = (*rmode).fb_width as usize;
        let xfb_height = (*rmode).xfb_height as usize;
        if fb_width == 0 || xfb_height == 0 || x >= fb_width || y >= xfb_height {
            return None;
        }
        let uncached = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *const u32;
        let pair_width = fb_width / 2;
        Some(*uncached.add(y * pair_width + (x & !1) / 2))
    }
}

unsafe extern "C" fn pre_callback(_count: u32) {
    unsafe {
        if let Some(previous) = callback_from_usize(PREVIOUS_PRE_CALLBACK.load(Ordering::Acquire)) {
            previous(_count);
        }
        if PRE_COUNT.fetch_add(1, Ordering::AcqRel) == 0 {
            let _ = CALLBACK_PHASE.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
        }
    }
}

unsafe extern "C" fn post_callback(_count: u32) {
    unsafe {
        if let Some(previous) = callback_from_usize(PREVIOUS_POST_CALLBACK.load(Ordering::Acquire))
        {
            previous(_count);
        }
        if POST_COUNT.fetch_add(1, Ordering::AcqRel) == 0 {
            let _ = CALLBACK_PHASE.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
        }
    }
}

fn callback_to_usize(callback: video::ViRetraceCallback) -> usize {
    callback.map_or(0, |cb| cb as usize)
}

fn callback_from_usize(value: usize) -> video::ViRetraceCallback {
    if value == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(u32)>(value) })
    }
}
