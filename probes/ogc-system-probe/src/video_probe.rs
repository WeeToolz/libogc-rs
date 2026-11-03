use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use ogc_core::mem_k0_to_k1;
use ogc_system::{self as system};
use ogc_ui::GxCanvas;
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
    probe_texture_queries(logger);
    probe_texture_indexed_lod(logger);
    probe_texture_regions(logger);
    probe_fifo_pointers(logger);

    let rmode = init_display(logger);
    logger.record("video init display", !rmode.is_null());
    if rmode.is_null() {
        return;
    }

    probe_gx_canvas_dimensions(logger, rmode);

    let initial_proof = unsafe { video::hardware_proof(rmode, DISPLAY_FRAMEBUFFER) };
    if !initial_proof.next_framebuffer_registers_match {
        logger.note_hex32("VI expected framebuffer", unsafe {
            DISPLAY_FRAMEBUFFER as usize as u32
        });
        logger.note_hex32("VI top high", initial_proof.registers.top_field_high as u32);
        logger.note_hex32("VI top low", initial_proof.registers.top_field_low as u32);
        logger.note_hex32(
            "VI bottom high",
            initial_proof.registers.bottom_field_high as u32,
        );
        logger.note_hex32(
            "VI bottom low",
            initial_proof.registers.bottom_field_low as u32,
        );
    }
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

fn probe_texture_regions(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let mut cache = gx::GxTexRegion {
        val: [0xdead_beef, 0xdead_beef, 0x5566_7788, 0x1122_3344],
    };
    // SAFETY: These are live writable descriptors. No region is submitted to GX.
    unsafe {
        gx::init_tex_cache_region(
            &mut cache,
            1,
            0x2000,
            gx::GX_TEXCACHE_32K,
            0x4000,
            gx::GX_TEXCACHE_128K,
        );
        logger.record(
            "GX texture cache region C layout",
            cache.val == [0x000d_8100, 0x0012_0200, 0x5566_7788, 0x0101_3344],
        );
        gx::init_tex_preload_region(&mut cache, 0x2000, 0x800, 0x4000, 0x1000);
        logger.record(
            "GX texture preload region C layout",
            cache.val == [0x0020_0100, 0x0000_0200, 0x0040_0080, 0x0000_3344],
        );
        let mut region = gx::GxTlutRegion {
            val: [0, 0x1122_3344, 0x5566_7788, 0x99aa_bbcc],
        };
        gx::init_tlut_region(&mut region, 0x80a00, 3);
        logger.record(
            "GX palette region preserves cached fields",
            region.val == [0x6500_0c05, 0x1122_3344, 0x5566_7788, 0x99aa_bbcc],
        );
        #[repr(align(32))]
        struct Palette([u8; 32]);
        let mut palette = Palette([0; 32]);
        let mut object = gx::GxTlutObj::default();
        gx::init_tlut_obj(&mut object, palette.0.as_mut_ptr().cast(), 5, 256);
        let expected_address = ((palette.0.as_mut_ptr() as usize) & 0x1fff_ffe0) as *mut c_void;
        logger.record(
            "GX palette object masks format",
            gx::tlut_obj_snapshot(&object).is_some_and(|s| {
                s.format == 1
                    && s.tlut_fmt == 0x400
                    && s.image_ptr == expected_address
                    && s.nentries == 256
            }),
        );
    }
}

fn probe_texture_indexed_lod(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    #[repr(align(32))]
    struct Image([u8; 64]);
    let mut image = Image([0; 64]);
    let mut first = gx::GxTexObj::default();
    let mut second = gx::GxTexObj::default();
    let init = gx::GxTexObjInit {
        wd: 16,
        ht: 8,
        fmt: gx::GX_TF_CI8,
        wrap_s: 1,
        wrap_t: 2,
        mipmap: 0,
    };
    // SAFETY: The image and both descriptor objects are live and writable.
    // The descriptors are inspected only; no image data is submitted to GX.
    unsafe {
        gx::init_tex_obj_ci(&mut first, image.0.as_mut_ptr().cast(), init, 0x1234);
        gx::init_tex_obj_ci_from_init(&mut second, image.0.as_mut_ptr().cast(), init, 0x5678);
        let first_ok = gx::tex_obj_snapshot(&first).is_some_and(|s| {
            s.width == 16
                && s.height == 8
                && s.format == gx::GX_TF_CI8
                && s.tex_tlut == 0x1234
                && s.tex_flag & 2 == 0
        });
        let second_ok = gx::tex_obj_snapshot(&second).is_some_and(|s| {
            s.width == 16
                && s.height == 8
                && s.format == gx::GX_TF_CI8
                && s.tex_tlut == 0x5678
                && s.tex_flag & 2 == 0
        });
        gx::init_tex_obj_tlut(&mut second, 0x9abc);
        logger.record(
            "GX indexed texture descriptors and palette",
            first_ok
                && second_ok
                && gx::tex_obj_snapshot(&second).is_some_and(|s| s.tex_tlut == 0x9abc),
        );

        gx::init_tex_obj_lod_from_params(
            &mut first,
            gx::GxTexObjLod {
                minfilt: gx::GX_NEAR_MIP_NEAR,
                magfilt: gx::GX_LINEAR,
                minlod: -1.0,
                maxlod: 12.0,
                lodbias: 1.5,
                biasclamp: 1,
                edgelod: 0,
                maxaniso: 2,
            },
        );
        // Fixed register words follow the libogc C field layout and scale.
        let grouped = gx::tex_obj_snapshot(&first)
            .is_some_and(|s| s.tex_filt == 0x0030_6139 && s.tex_lod == 0x0000_a000);
        gx::init_tex_obj_edge_lod(&mut first, 1);
        gx::init_tex_obj_lod_bias(&mut first, 0.5);
        logger.record(
            "GX texture LOD fields and clamps",
            grouped
                && gx::tex_obj_snapshot(&first)
                    .is_some_and(|s| s.tex_filt == 0x0030_2039 && s.tex_lod == 0x0000_a000),
        );
    }
}

fn probe_texture_queries(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    #[repr(align(32))]
    struct Image([u8; 64]);
    let mut images = [Image([0; 64]), Image([0; 64])];
    let first = images[0].0.as_mut_ptr().cast::<c_void>();
    let second = images[1].0.as_mut_ptr().cast::<c_void>();
    let expected_first = ((first as usize) & 0x1fff_ffe0) as *mut c_void;
    let expected_second = ((second as usize) & 0x1fff_ffe0) as *mut c_void;
    let mut object = gx::GxTexObj::default();
    let mut user_data = 17u32;
    let user_ptr = (&mut user_data as *mut u32).cast::<c_void>();
    // SAFETY: Both aligned image buffers and all descriptor/output objects
    // remain live and exclusively borrowed for their respective calls. This
    // checks descriptor metadata and does not submit the images to GX.
    unsafe {
        gx::init_tex_obj_from_init(
            &mut object,
            first,
            gx::GxTexObjInit {
                wd: 16,
                ht: 8,
                fmt: gx::GX_TF_RGB565,
                wrap_s: 1,
                wrap_t: 2,
                mipmap: 1,
            },
        );
        gx::init_tex_obj_user_data(&mut object, user_ptr);
        let initial = gx::get_tex_obj_data(&object) == expected_first
            && gx::get_tex_obj_mip_map(&object) == 1
            && gx::get_tex_obj_user_data(&object) == user_ptr;
        gx::init_tex_obj_data(&mut object, second);
        logger.record(
            "GX texture image and user pointers",
            initial
                && gx::get_tex_obj_data(&object) == expected_second
                && gx::get_tex_obj_user_data(&object) == user_ptr,
        );

        gx::init_tex_obj_filter_mode(&mut object, gx::GX_NEAR_MIP_NEAR, gx::GX_NEAR);
        gx::init_tex_obj_min_lod(&mut object, 1.25);
        gx::init_tex_obj_max_lod(&mut object, 3.5);
        let (mut min_filter, mut mag_filter) = (255, 255);
        let (mut min_lod, mut max_lod) = (-1.0, -1.0);
        gx::get_tex_obj_filter_mode(&object, &mut min_filter, &mut mag_filter);
        gx::get_tex_obj_lod(&object, &mut min_lod, &mut max_lod);
        logger.record(
            "GX texture filter and LOD queries",
            min_filter == gx::GX_NEAR_MIP_NEAR
                && mag_filter == gx::GX_NEAR
                && min_lod == 1.25
                && max_lod == 3.5,
        );

        let mut info = gx::GxTexObjInfo::default();
        gx::get_tex_obj_info(&object, &mut info);
        let (mut image, mut width, mut height, mut format, mut wrap_s, mut wrap_t, mut mipmap) =
            (null_mut(), 0, 0, 0, 0, 0, 0);
        gx::get_tex_obj_all(
            &object,
            gx::GxTexObjOutputs {
                image_ptr: &mut image,
                width: &mut width,
                height: &mut height,
                format: &mut format,
                wrap_s: &mut wrap_s,
                wrap_t: &mut wrap_t,
                mipmap: &mut mipmap,
            },
        );
        logger.record(
            "GX texture grouped queries",
            info.image_ptr == expected_second
                && info.width == 16
                && info.height == 8
                && info.format == gx::GX_TF_RGB565
                && info.wrap_s == 1
                && info.wrap_t == 2
                && info.mipmap == 1
                && image == expected_second
                && width == 16
                && height == 8
                && format == gx::GX_TF_RGB565
                && wrap_s == 1
                && wrap_t == 2
                && mipmap == 1,
        );
    }
}

fn probe_gx_canvas_dimensions(logger: &mut ProbeLogger, rmode: *mut video::GxrModeObj) {
    use ogc_gx::GxFifo;

    // SAFETY: This video probe owns GX for this run and keeps the FIFO alive
    // while GxCanvas configures the production GX backend.
    let _fifo = match unsafe { GxFifo::initialize_default_mem2() } {
        Ok(fifo) => fifo,
        Err(error) => {
            logger.report_step("GX canvas FIFO initialization");
            logger.record("GX canvas FIFO initialization", false);
            logger.note_u32("GX canvas FIFO error code", error as u32);
            return;
        }
    };

    // SAFETY: `rmode` is the live preferred mode initialized above, and this
    // probe owns the GX FIFO for the duration of the canvas check.
    let (width, height) = unsafe { ((*rmode).fb_width, (*rmode).efb_height) };
    let canvas = unsafe { GxCanvas::from_video_mode(&*rmode) };
    logger.record(
        "GX canvas dimensions and validation",
        canvas.is_ok_and(|canvas| {
            canvas.size() == (width, height)
                && unsafe { GxCanvas::new(0, height).is_err() }
                && unsafe { GxCanvas::new(width, 0).is_err() }
                && unsafe { GxCanvas::new(1025, height).is_err() }
                && unsafe { GxCanvas::new(width, 1025).is_err() }
        }),
    );
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
    }
    logger.record("GX FIFO base size and initial count", unsafe {
        gx::get_fifo_base(&fifo) == base.cast()
            && gx::get_fifo_size(&fifo) == 65536
            && gx::get_fifo_count(&fifo) == 0
            && gx::get_fifo_wrap(&fifo) == 0
    });
    let before_limits = fifo;
    // The C __gxfifo layout has 32-bit high and low watermarks at byte
    // offsets 12 and 16. The target stores these values in big-endian order.
    unsafe { gx::init_fifo_limits(&mut fifo, 0xc000, 0x4000) };
    logger.record(
        "GX FIFO watermarks and adjacent fields",
        fifo.pad[12..20] == [0, 0, 0xc0, 0, 0, 0, 0x40, 0]
            && fifo.pad[..12] == before_limits.pad[..12]
            && fifo.pad[20..] == before_limits.pad[20..],
    );
    let before_rejected_init = fifo;
    // SAFETY: The object and base are valid; the short size must be rejected.
    unsafe { gx::init_fifo_base(&mut fifo, base.cast(), 65535) };
    logger.record(
        "GX FIFO short initialization preserves object",
        fifo == before_rejected_init,
    );
    // The C layout stores the wrap flag at offset 32. This exercises metadata
    // readback only; it does not simulate or claim a hardware FIFO wrap.
    fifo.pad[32] = 1;
    logger.record("GX FIFO wrap flag readback", unsafe {
        gx::get_fifo_wrap(&fifo) == 1
    });
    fifo.pad[32] = 0;
    unsafe {
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
