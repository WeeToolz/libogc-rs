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
static GX_DONE_COUNT: AtomicU32 = AtomicU32::new(0);
static GX_TOKEN_COUNT: AtomicU32 = AtomicU32::new(0);
static GX_LAST_TOKEN: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn probe_gx_done_callback() {
    GX_DONE_COUNT.fetch_add(1, Ordering::Release);
}

unsafe extern "C" fn probe_gx_token_callback(token: u16) {
    GX_LAST_TOKEN.store(u32::from(token), Ordering::Relaxed);
    GX_TOKEN_COUNT.fetch_add(1, Ordering::Release);
}
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
    probe_tpl_memory(logger);
    probe_tpl_indexed(logger);
    probe_frame_config(logger);
    probe_texture_queries(logger);
    probe_texture_indexed_lod(logger);
    probe_texture_regions(logger);
    probe_texture_sizes(logger);
    probe_texture_conversion(logger);
    probe_light_object(logger);
    probe_light_falloff(logger);
    probe_specular_light(logger);
    probe_overscan(logger);
    probe_fog_table(logger);
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
    let expected_scan_mode = unsafe { (*rmode).vi_tv_mode & 0x3 };
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

fn probe_tpl_memory(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    // One 8x8 I4 image: 12-byte header, 8-byte descriptor, 36-byte image
    // header, padding to a 32-byte boundary, and one 32-byte texture tile.
    #[repr(align(32))]
    struct TplBytes([u8; 96]);
    let mut bytes = TplBytes([0; 96]);
    bytes.0[0..4].copy_from_slice(&0x0020_af30u32.to_be_bytes());
    bytes.0[4..8].copy_from_slice(&1u32.to_be_bytes());
    bytes.0[8..12].copy_from_slice(&12u32.to_be_bytes());
    bytes.0[12..16].copy_from_slice(&20u32.to_be_bytes());
    bytes.0[20..22].copy_from_slice(&8u16.to_be_bytes());
    bytes.0[22..24].copy_from_slice(&8u16.to_be_bytes());
    bytes.0[28..32].copy_from_slice(&64u32.to_be_bytes());
    let mut file = gx::TplFile {
        type_: 0,
        ntextures: 0,
        texdesc: null_mut(),
        tpl_file: null_mut(),
    };
    // SAFETY: The aligned, writable fixture and descriptor remain live until
    // close; the returned texture object is inspected only and not submitted.
    unsafe {
        logger.record(
            "GX TPL memory rejects null input",
            gx::tpl_open_tpl_from_memory(&mut file, null_mut(), 96) < 0,
        );
        let opened = gx::tpl_open_tpl_from_memory(&mut file, bytes.0.as_mut_ptr().cast(), 96);
        logger.record(
            "GX TPL memory image opens",
            opened == 1 && file.ntextures == 1,
        );
        if opened != 1 {
            return;
        }
        let (mut format, mut width, mut height) = (u32::MAX, 0, 0);
        let info = gx::tpl_get_texture_info(&mut file, 0, &mut format, &mut width, &mut height);
        logger.record(
            "GX TPL image metadata and invalid index",
            info == 0
                && (format, width, height) == (u32::from(gx::GX_TF_I4), 8, 8)
                && gx::tpl_get_texture_info(&mut file, 1, null_mut(), null_mut(), null_mut()) < 0,
        );
        let mut texture = gx::GxTexObj::default();
        let result = gx::tpl_get_texture(&mut file, 0, &mut texture);
        let expected_data = ((bytes.0.as_mut_ptr().add(64) as usize) & 0x1fff_ffe0) as *mut c_void;
        logger.record(
            "GX TPL creates texture descriptor",
            result == 0
                && gx::tex_obj_snapshot(&texture).is_some_and(|s| {
                    s.width == 8
                        && s.height == 8
                        && s.format == gx::GX_TF_I4
                        && s.image_ptr == expected_data
                })
                && gx::tpl_get_texture(&mut file, -1, &mut texture) < 0,
        );
        gx::tpl_close_tpl_file(&mut file);
        logger.record(
            "GX TPL close invalidates descriptor",
            file.ntextures == 0 && file.texdesc.is_null() && file.tpl_file.is_null(),
        );
    }
}

fn probe_tpl_indexed(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    // A CI8 8x4 tile and a 16-entry RGB565 palette, both 32-byte aligned.
    #[repr(align(32))]
    struct TplBytes([u8; 160]);
    let mut bytes = TplBytes([0; 160]);
    bytes.0[0..4].copy_from_slice(&0x0020_af30u32.to_be_bytes());
    bytes.0[4..8].copy_from_slice(&1u32.to_be_bytes());
    bytes.0[8..12].copy_from_slice(&12u32.to_be_bytes());
    bytes.0[12..16].copy_from_slice(&20u32.to_be_bytes());
    bytes.0[16..20].copy_from_slice(&96u32.to_be_bytes());
    bytes.0[20..22].copy_from_slice(&4u16.to_be_bytes());
    bytes.0[22..24].copy_from_slice(&8u16.to_be_bytes());
    bytes.0[24..28].copy_from_slice(&u32::from(gx::GX_TF_CI8).to_be_bytes());
    bytes.0[28..32].copy_from_slice(&64u32.to_be_bytes());
    bytes.0[96..98].copy_from_slice(&16u16.to_be_bytes());
    bytes.0[100..104].copy_from_slice(&u32::from(gx::GX_TL_RGB565).to_be_bytes());
    bytes.0[104..108].copy_from_slice(&128u32.to_be_bytes());
    let mut file = gx::TplFile {
        type_: 0,
        ntextures: 0,
        texdesc: null_mut(),
        tpl_file: null_mut(),
    };
    // SAFETY: The fixture is writable, aligned, and remains live until close.
    unsafe {
        let opened = gx::tpl_open_tpl_from_memory(&mut file, bytes.0.as_mut_ptr().cast(), 160);
        if opened != 1 {
            logger.record("GX indexed TPL fixture opens", false);
            return;
        }
        let mut texture = gx::GxTexObj::default();
        let mut palette = gx::GxTlutObj::default();
        let rejected = gx::tpl_get_texture_ci(&mut file, 1, &mut texture, &mut palette, 3) < 0
            && gx::tpl_get_texture_ci(&mut file, 0, &mut texture, null_mut(), 3) < 0;
        let result = gx::tpl_get_texture_ci(&mut file, 0, &mut texture, &mut palette, 3);
        let image = ((bytes.0.as_mut_ptr().add(64) as usize) & 0x1fff_ffe0) as *mut c_void;
        let colors = ((bytes.0.as_mut_ptr().add(128) as usize) & 0x1fff_ffe0) as *mut c_void;
        logger.record(
            "GX indexed TPL texture and palette",
            rejected
                && result == 0
                && gx::tex_obj_snapshot(&texture).is_some_and(|s| {
                    s.width == 8
                        && s.height == 4
                        && s.format == gx::GX_TF_CI8
                        && s.tex_tlut == 3
                        && s.image_ptr == image
                })
                && gx::tlut_obj_snapshot(&palette).is_some_and(|s| {
                    s.format == u32::from(gx::GX_TL_RGB565)
                        && s.nentries == 16
                        && s.image_ptr == colors
                }),
        );
        gx::tpl_close_tpl_file(&mut file);
    }
}

fn probe_frame_config(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    #[repr(align(32))]
    struct Aligned([u8; 32]);
    // Only descriptor fields are inspected; no framebuffer copy is submitted.
    let mut storage = Aligned([0; 32]);
    let framebuffer = storage.0.as_mut_ptr().cast::<c_void>();
    let initial = gx::FrameConfig::new(framebuffer, 640, 480, 480);
    let rejected = gx::FrameConfig::new(null_mut(), 640, 480, 480).is_none()
        && gx::FrameConfig::new(
            unsafe { storage.0.as_mut_ptr().add(1) }.cast(),
            640,
            480,
            480,
        )
        .is_none()
        && gx::FrameConfig::new(framebuffer, 0, 480, 480).is_none();
    logger.record(
        "GX frame configuration validates input",
        rejected
            && initial.is_some_and(|c| {
                c.xfb == framebuffer
                    && c.width == 640
                    && c.efb_height == 480
                    && c.xfb_height == 480
                    && !c.field_rendering
                    && !c.half_aspect_ratio
            }),
    );
    if let Some(config) = initial {
        let filter = gx::GxCopyFilter {
            antialias: true,
            sample_pattern: [[3, 4]; 12],
            vertical_enabled: true,
            vertical_coefficients: [1, 2, 3, 4, 5, 6, 7],
        };
        let config = gx::FrameConfig::with_copy_filter(config, filter);
        let config = gx::FrameConfig::with_field_mode(config, true, false);
        logger.record(
            "GX frame filter and field settings retained",
            config.copy_filter == filter
                && config.field_rendering
                && !config.half_aspect_ratio
                && config.xfb == framebuffer,
        );
    } else {
        logger.record("GX frame filter and field settings retained", false);
    }
}

fn probe_texture_conversion(logger: &mut ProbeLogger) {
    let mut source = [0u8; 4 * 1024];
    for row in 0..4 {
        for column in 0..16 {
            source[row * 1024 + column] = (row * 16 + column) as u8;
        }
    }
    let mut output = [0xa5u8; 72];
    // SAFETY: Source rows use the required 1024-byte stride. The output has
    // space for both complete tiles and does not overlap the source.
    unsafe { ogc_gx::make_texture565(source.as_ptr().cast(), output.as_mut_ptr().cast(), 9, 5) };
    let expected = [
        0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23, 32, 33, 34, 35, 36, 37, 38, 39, 48,
        49, 50, 51, 52, 53, 54, 55, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29, 30, 31,
        40, 41, 42, 43, 44, 45, 46, 47, 56, 57, 58, 59, 60, 61, 62, 63,
    ];
    let valid = output[..64] == expected && output[64..] == [0xa5; 8];
    unsafe { ogc_gx::make_texture565(source.as_ptr().cast(), output.as_mut_ptr().cast(), -1, 4) };
    logger.record(
        "GX RGB565 tiled conversion and partial dimensions",
        valid && output[..64] == expected && output[64..] == [0xa5; 8],
    );
}

fn probe_texture_sizes(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let cases = [
        (8, 8, gx::GX_TF_I4, 0, 0, 32),
        (9, 8, gx::GX_TF_I4, 0, 0, 64),
        (8, 5, gx::GX_TF_I8, 0, 0, 64),
        (5, 5, gx::GX_TF_RGB565, 0, 0, 128),
        (5, 5, gx::GX_TF_RGBA8, 0, 0, 256),
        (8, 8, gx::GX_TF_I4, 1, 0, 0),
        (8, 8, gx::GX_TF_I4, 1, 4, 128),
        (8, 8, gx::GX_TF_I4, 1, 10, 128),
    ];
    let mut matches = true;
    for (width, height, format, mipmap, levels, expected) in cases {
        matches &=
            gx::get_tex_buffer_size(width, height, u32::from(format), mipmap, levels) == expected;
    }
    logger.record("GX texture buffer tile and mip sizes", matches);
}

fn probe_fog_table(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let projection = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut table = gx::GxFogAdjTbl { r: [0xffff; 10] };
    // SAFETY: The projection and output table are live and valid.
    unsafe { gx::init_fog_adj_table(&mut table, 640, &projection) };
    let expected = [256, 257, 259, 262, 266, 270, 276, 281, 288, 295];
    let valid = table.r == expected;
    unsafe { gx::init_fog_adj_table(&mut table, 0, &projection) };
    logger.record(
        "GX fog adjustment table and zero width",
        valid && table.r == expected,
    );
    let perspective = [
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, -2.0],
        [0.0, 0.0, -1.0, 0.0],
    ];
    // SAFETY: The table and projection are valid. Null arguments are accepted
    // by the Rust API and must leave the table unchanged.
    unsafe {
        gx::init_fog_adj_table(&mut table, 640, &perspective);
        gx::init_fog_adj_table(&mut table, 640, core::ptr::null());
        gx::init_fog_adj_table(core::ptr::null_mut(), 640, &perspective);
    }
    logger.record(
        "GX perspective fog table and null inputs",
        table.r == [256, 257, 258, 261, 263, 267, 271, 275, 280, 286],
    );
}

fn probe_overscan(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let source = gx::GxrModeObj {
        vi_tv_mode: 0,
        fb_width: 640,
        efb_height: 480,
        xfb_height: 480,
        vi_x_origin: 40,
        vi_y_origin: 20,
        vi_width: 640,
        vi_height: 480,
        xfb_mode: 0,
        field_rendering: 1,
        aa: 0,
        sample_pattern: [[3, 4]; 12],
        vfilter: [5; 7],
    };
    let mut output = source;
    // SAFETY: Both mode objects are initialized. In-place use is supported.
    unsafe { gx::adjust_for_overscan(&source, &mut output, 10, 20) };
    let interlaced = output.fb_width == 620
        && output.efb_height == 440
        && output.xfb_height == 460
        && output.vi_width == 620
        && output.vi_height == 440
        && output.vi_x_origin == 50
        && output.vi_y_origin == 40
        && output.sample_pattern == [[3, 4]; 12]
        && output.vfilter == [5; 7];
    let mut progressive = source;
    progressive.vi_tv_mode = 2;
    let ptr = &mut progressive as *mut gx::GxrModeObj;
    unsafe { gx::adjust_for_overscan(ptr, ptr, 10, 20) };
    let progressive_ok = progressive.fb_width == 620
        && progressive.efb_height == 440
        && progressive.xfb_height == 440
        && progressive.vi_height == 400
        && progressive.vi_x_origin == 50
        && progressive.vi_y_origin == 40;
    let mut invalid = source;
    invalid.xfb_height = 0;
    let before = output;
    unsafe { gx::adjust_for_overscan(&invalid, &mut output, 10, 20) };
    logger.record(
        "GX overscan modes and in-place output",
        interlaced
            && progressive_ok
            && output.fb_width == before.fb_width
            && output.xfb_height == before.xfb_height
            && output.vi_height == before.vi_height,
    );
}

fn probe_specular_light(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let mut light = gx::GxLightObj::default();
    // SAFETY: The initialized light object is exclusively accessible.
    unsafe {
        gx::init_specular_dir_ha(&mut light, 1.0, 0.0, -1.0, 0.25, 0.5, 0.75);
        let explicit = gx::light_obj_snapshot(&light).is_some_and(|s| {
            s.position == [-1.0e18, 0.0, 1.0e18] && s.direction == [0.25, 0.5, 0.75]
        });
        gx::init_specular_dir(&mut light, 0.0, 0.0, -1.0);
        let computed = gx::light_obj_snapshot(&light)
            .is_some_and(|s| s.position == [0.0, 0.0, 1.0e18] && s.direction == [0.0, 0.0, 1.0]);
        gx::init_specular_dir(&mut light, 0.0, 0.0, 1.0);
        let zero = gx::light_obj_snapshot(&light)
            .is_some_and(|s| s.position == [0.0, 0.0, -1.0e18] && s.direction == [0.0, 0.0, 0.0]);
        logger.record("GX specular directions", explicit && computed && zero);
    }
}

fn probe_light_falloff(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let mut light = gx::GxLightObj::default();
    // SAFETY: The light object is initialized and exclusively accessible.
    unsafe {
        gx::init_light_dist_attn(&mut light, 2.0, 0.5, gx::GX_DA_GENTLE);
        let gentle =
            gx::light_obj_snapshot(&light).is_some_and(|s| s.attenuation_coeff == [1.0, 0.5, 0.0]);
        gx::init_light_dist_attn(&mut light, 2.0, 0.5, gx::GX_DA_MEDIUM);
        let medium = gx::light_obj_snapshot(&light)
            .is_some_and(|s| s.attenuation_coeff == [1.0, 0.25, 0.125]);
        gx::init_light_dist_attn(&mut light, 2.0, 0.5, gx::GX_DA_STEEP);
        let steep =
            gx::light_obj_snapshot(&light).is_some_and(|s| s.attenuation_coeff == [1.0, 0.0, 0.25]);
        gx::init_light_dist_attn(&mut light, -1.0, 0.5, gx::GX_DA_GENTLE);
        let invalid =
            gx::light_obj_snapshot(&light).is_some_and(|s| s.attenuation_coeff == [1.0, 0.0, 0.0]);
        logger.record(
            "GX light distance attenuation modes",
            gentle && medium && steep && invalid,
        );
        gx::init_light_spot(&mut light, 90.0, gx::GX_SP_FLAT);
        let flat = gx::light_obj_snapshot(&light)
            .is_some_and(|s| s.attenuation[0].abs() < 0.001 && s.attenuation[1..] == [1000.0, 0.0]);
        gx::init_light_spot(&mut light, 91.0, gx::GX_SP_FLAT);
        let invalid =
            gx::light_obj_snapshot(&light).is_some_and(|s| s.attenuation == [1.0, 0.0, 0.0]);
        logger.record("GX light spotlight cutoff", flat && invalid);
        let cases = [
            (gx::GX_SP_COS, [0.0, 1.0, 0.0]),
            (gx::GX_SP_COS2, [0.0, 0.0, 1.0]),
            (gx::GX_SP_SHARP, [0.0, 2.0, -1.0]),
            (gx::GX_SP_RING1, [0.0, 4.0, -4.0]),
            (gx::GX_SP_RING2, [1.0, 0.0, -2.0]),
        ];
        let mut modes_match = true;
        for (mode, expected) in cases {
            gx::init_light_spot(&mut light, 90.0, mode);
            modes_match &= gx::light_obj_snapshot(&light).is_some_and(|snapshot| {
                snapshot
                    .attenuation
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| (*actual - expected).abs() < 0.000_01)
            });
        }
        logger.record("GX light spotlight modes", modes_match);
    }
}

fn probe_light_object(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    let mut light = gx::GxLightObj::default();
    // SAFETY: The initialized light object is exclusively accessible.
    unsafe {
        gx::init_light_pos(&mut light, 1.0, -2.0, 3.5);
        gx::init_light_dir(&mut light, 4.0, -5.0, 6.0);
        gx::init_light_color(
            &mut light,
            gx::GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0x78,
            },
        );
        gx::init_light_attn(&mut light, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let initial = gx::light_obj_snapshot(&light).is_some_and(|s| {
            s.position == [1.0, -2.0, 3.5]
                && s.direction == [-4.0, 5.0, -6.0]
                && s.color
                    == gx::GxColor {
                        r: 0x12,
                        g: 0x34,
                        b: 0x56,
                        a: 0x78,
                    }
                && s.attenuation == [1.0, 2.0, 3.0]
                && s.attenuation_coeff == [4.0, 5.0, 6.0]
        });
        gx::init_light_attn_a(&mut light, 7.0, 8.0, 9.0);
        gx::init_light_attn_k(&mut light, 10.0, 11.0, 12.0);
        logger.record(
            "GX light object fields and attenuation",
            initial
                && gx::light_obj_snapshot(&light).is_some_and(|s| {
                    s.position == [1.0, -2.0, 3.5]
                        && s.direction == [-4.0, 5.0, -6.0]
                        && s.attenuation == [7.0, 8.0, 9.0]
                        && s.attenuation_coeff == [10.0, 11.0, 12.0]
                }),
        );
    }
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
    use ogc_gx::{GxFifo, GxFifoError};

    let arena_before = system::get_arena2_lo() as usize;
    // SAFETY: Invalid sizes are rejected before any FIFO or arena mutation.
    let invalid_sizes = unsafe {
        matches!(GxFifo::initialize_mem2(0), Err(GxFifoError::InvalidSize))
            && matches!(
                GxFifo::initialize_mem2(65537),
                Err(GxFifoError::InvalidSize)
            )
    };
    logger.record(
        "GX MEM2 FIFO rejects invalid sizes without allocation",
        invalid_sizes && system::get_arena2_lo() as usize == arena_before,
    );

    // SAFETY: This video probe owns GX for this run and keeps the FIFO alive
    // while GxCanvas configures the production GX backend.
    let fifo = match unsafe { GxFifo::initialize_default_mem2() } {
        Ok(fifo) => fifo,
        Err(error) => {
            logger.report_step("GX canvas FIFO initialization");
            logger.record("GX canvas FIFO initialization", false);
            logger.note_u32("GX canvas FIFO error code", error as u32);
            return;
        }
    };

    let address = GxFifo::address(&fifo) as usize;
    let size = GxFifo::size(&fifo);
    let expected_base = (arena_before + 31) & !31;
    logger.record(
        "GX default MEM2 FIFO address size and arena advancement",
        address == expected_base
            && size == 256 * 1024
            && system::get_arena2_lo() as usize == expected_base + size as usize,
    );

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
    probe_efb_access(logger);
    probe_efb_pe_registers(logger);
    probe_display_lists(logger);
    probe_viewport_commands(logger);
    probe_viewport_jitter_commands(logger);
    probe_scissor_commands(logger);
    probe_scissor_offset_commands(logger);
    probe_projection_commands(logger);
    probe_position_matrix_commands(logger);
    probe_indexed_position_matrix_commands(logger);
    probe_normal_matrix_commands(logger);
    probe_normal_matrix_3x3_commands(logger);
    probe_indexed_normal_matrix_commands(logger);
    probe_texture_matrix_commands(logger);
    probe_indexed_texture_matrix_commands(logger);
    probe_signed_position_commands(logger);
    probe_signed_normal_commands(logger);
    probe_signed_texture_commands(logger);
    probe_vertex_color_commands(logger);
    probe_vertex_indices_8(logger);
    probe_vertex_indices_16(logger);
    probe_float_vertex_commands(logger);
    probe_unsigned_position_commands(logger);
    probe_unsigned_texture_commands(logger);
    probe_tev_color_commands(logger);
    probe_tev_signed_color_commands(logger);
    probe_tev_k_color_commands(logger);
    probe_tev_k_signed_color_commands(logger);
    probe_tev_combiner_commands(logger);
    probe_tev_selector_commands(logger);
    probe_tev_indirect_commands(logger);
    probe_indirect_texture_commands(logger);
    probe_indirect_helper_commands(logger);
    probe_indirect_matrix_commands(logger);
    probe_tex_coord_scale_commands(logger);
    probe_alpha_compare_commands(logger);
    probe_fog_color_commands(logger);
    probe_fog_commands(logger);
    probe_clip_mode_commands(logger);
    probe_copy_clear_commands(logger);
    probe_copy_filter_commands(logger);
    probe_field_mask_commands(logger);
    probe_bounding_box_commands(logger);
    probe_z_texture_commands(logger);
    probe_z_mode_commands(logger);
    probe_pe_control_commands(logger);
    probe_vertex_cache_commands(logger);
    probe_texture_cache_commands(logger);
    probe_line_point_field_commands(logger);
    probe_vertex_descriptors(logger);
    probe_vertex_array_commands(logger);
    probe_vertex_format_commands(logger);
    probe_lighting_channel_commands(logger);
    probe_generation_mode_commands(logger);
    probe_texgen_commands(logger);
    probe_pixel_tev_commands(logger);
    probe_tev_order_scale(logger);
    probe_gx_completion_callbacks(logger);
    probe_solid_rect_render(logger, width, height);
    probe_textured_rect_render(logger, width, height);
    probe_texture_copy_render(logger, width, height);
}

fn probe_gx_completion_callbacks(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX completion and token callbacks");
    GX_DONE_COUNT.store(0, Ordering::Relaxed);
    GX_TOKEN_COUNT.store(0, Ordering::Relaxed);
    GX_LAST_TOKEN.store(0, Ordering::Relaxed);
    let old_done = gx::set_draw_done_callback(Some(probe_gx_done_callback));
    let old_token = gx::set_draw_sync_callback(Some(probe_gx_token_callback));
    gx::set_draw_sync(0x5a39);
    // SAFETY: GX is initialized and this probe owns its FIFO and callbacks.
    let completed = unsafe { gx::finish_frame_and_wait() };
    logger.record(
        "GX token and finish interrupts invoke callbacks",
        completed
            && GX_DONE_COUNT.load(Ordering::Acquire) == 1
            && GX_TOKEN_COUNT.load(Ordering::Acquire) == 1
            && GX_LAST_TOKEN.load(Ordering::Relaxed) == 0x5a39,
    );
    if completed {
        gx::set_draw_done();
        for _ in 0..10 {
            if GX_DONE_COUNT.load(Ordering::Acquire) >= 2 {
                break;
            }
            if !video::wait_vsync_timeout_us(RETRACE_WAIT_TIMEOUT_US) {
                break;
            }
        }
        let notified = GX_DONE_COUNT.load(Ordering::Acquire) == 2;
        if notified {
            gx::wait_draw_done();
        }
        logger.record("GX queued draw done signals before wait", notified);
        if notified {
            gx::draw_done();
            logger.record(
                "GX draw done waits for next finish callback",
                GX_DONE_COUNT.load(Ordering::Acquire) == 3,
            );
        } else {
            logger.record("GX draw done waits for next finish callback", false);
        }
    } else {
        logger.record("GX queued draw done signals before wait", false);
        logger.record("GX draw done waits for next finish callback", false);
    }
    let replaced_done = gx::set_draw_done_callback(old_done);
    let replaced_token = gx::set_draw_sync_callback(old_token);
    logger.record(
        "GX callback setters return replaced handlers",
        replaced_done.is_some_and(|f| {
            core::ptr::fn_addr_eq(f, probe_gx_done_callback as unsafe extern "C" fn())
        }) && replaced_token.is_some_and(|f| {
            core::ptr::fn_addr_eq(f, probe_gx_token_callback as unsafe extern "C" fn(u16))
        }),
    );
}

fn probe_vertex_descriptors(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    const N: usize = gx::GX_MAX_VTXDESC_LISTSIZE;
    const EMPTY: gx::GxVtxDesc = gx::GxVtxDesc {
        attr: gx::GX_VA_NULL,
        ty: 0,
    };
    let mut saved = [EMPTY; N];
    let mut actual = [gx::GxVtxDesc {
        attr: 0x55,
        ty: 0x55,
    }; N];
    // SAFETY: Both arrays have the required capacity, and GX is owned by the probe.
    unsafe { gx::get_vtx_descv(saved.as_mut_ptr()) };
    gx::clear_vtx_desc();
    unsafe { gx::get_vtx_descv(actual.as_mut_ptr()) };
    logger.record(
        "GX cleared vertex descriptor is fully empty",
        actual == [EMPTY; N],
    );

    let input = [
        gx::GxVtxDesc {
            attr: gx::GX_VA_TEX7,
            ty: gx::GX_INDEX16,
        },
        gx::GxVtxDesc {
            attr: gx::GX_VA_CLR0,
            ty: gx::GX_DIRECT,
        },
        gx::GxVtxDesc {
            attr: gx::GX_VA_POS,
            ty: gx::GX_INDEX8,
        },
        gx::GxVtxDesc {
            attr: gx::GX_VA_PTNMTXIDX,
            ty: gx::GX_DIRECT,
        },
        gx::GxVtxDesc {
            attr: gx::GX_VA_NBT,
            ty: gx::GX_INDEX16,
        },
        EMPTY,
        gx::GxVtxDesc {
            attr: gx::GX_VA_TEX0,
            ty: gx::GX_DIRECT,
        },
    ];
    // SAFETY: The input terminates at EMPTY and the output has full capacity.
    unsafe { gx::set_vtx_descv(input.as_ptr()) };
    unsafe { gx::get_vtx_descv(actual.as_mut_ptr()) };
    let expected = [input[3], input[2], input[4], input[1], input[0]];
    logger.record(
        "GX descriptor list orders attributes and stops at sentinel",
        actual[..expected.len()] == expected
            && actual[expected.len()..].iter().all(|x| *x == EMPTY),
    );

    gx::set_vtx_desc(gx::GX_VA_NRM, gx::GX_INDEX8);
    gx::set_vtx_desc(gx::GX_VA_CLR0, gx::GX_NONE);
    unsafe { gx::get_vtx_descv(actual.as_mut_ptr()) };
    let expected = [
        input[3],
        input[2],
        gx::GxVtxDesc {
            attr: gx::GX_VA_NRM,
            ty: gx::GX_INDEX8,
        },
        input[0],
    ];
    logger.record(
        "GX single descriptor switches normal and removes color",
        actual[..expected.len()] == expected
            && actual[expected.len()..].iter().all(|x| *x == EMPTY),
    );
    // SAFETY: Null lists are explicitly accepted; restore the saved list afterward.
    unsafe {
        gx::set_vtx_descv(core::ptr::null());
        gx::get_vtx_descv(core::ptr::null_mut());
    }
    gx::clear_vtx_desc();
    unsafe {
        gx::set_vtx_descv(saved.as_ptr());
        gx::get_vtx_descv(actual.as_mut_ptr());
    }
    logger.record("GX vertex descriptor restoration", actual == saved);
}

fn check_gx_command_bytes(expected: &[u8], emit: impl FnOnce()) -> bool {
    use ogc_gx as gx;

    #[repr(align(32))]
    struct CommandBytes([u8; 128]);
    let mut bytes = CommandBytes([0xa5; 128]);
    let token_before = gx::get_draw_sync();
    // SAFETY: GX is initialized and exclusively owned by this probe. Only the
    // first 96 aligned bytes are offered; the last 32 bytes form a guard.
    unsafe { gx::begin_disp_list(bytes.0.as_mut_ptr().cast(), 96) };
    emit();
    let size = gx::end_disp_list() as usize;
    // These APIs do not append GX_Flush. End pads only the emitted bytes.
    let expected_size = (expected.len() + 31) & !31;
    size == expected_size
        && size <= 96
        && bytes.0[..expected.len()] == *expected
        && bytes.0[expected.len()..size].iter().all(|byte| *byte == 0)
        && bytes.0[size..].iter().all(|byte| *byte == 0xa5)
        && gx::get_draw_sync() == token_before
}

fn probe_vertex_array_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX vertex array and primitive command bytes");
    // The address is encoded only into a captured display list; GX never reads it.
    let address = core::ptr::without_provenance_mut::<core::ffi::c_void>(0x8123_4560);
    let position = [0x08, 0xa0, 0x01, 0x23, 0x45, 0x60, 0x08, 0xb0, 0, 0, 0, 12];
    logger.record(
        "GX position array encodes physical address and stride",
        check_gx_command_bytes(&position, || {
            gx::set_array(u32::from(gx::GX_VA_POS), address, 12);
        }),
    );
    let normal = [0x08, 0xa1, 0x01, 0x23, 0x45, 0x60, 0x08, 0xb1, 0, 0, 0, 36];
    logger.record(
        "GX NBT array maps to normal array registers",
        check_gx_command_bytes(&normal, || {
            gx::set_array(u32::from(gx::GX_VA_NBT), address, 36);
        }),
    );
    logger.record(
        "GX invalid array attribute emits no command",
        check_gx_command_bytes(&[], || {
            gx::set_array(u32::from(gx::GX_VA_NULL), address, 12);
        }),
    );
    // SAFETY: The primitive header is captured, not executed. The display-list
    // setup flushes pending state before the captured begin call.
    logger.record(
        "GX primitive begin and end encode header",
        check_gx_command_bytes(&[0x95, 0x12, 0x34], || unsafe {
            gx::begin(gx::GX_TRIANGLES, 13, 0x1234);
            gx::end();
        }),
    );
}

fn probe_vertex_format_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX vertex format command bytes");
    let formats = [
        gx::GxVtxAttrFmt {
            vtxattr: u32::from(gx::GX_VA_TEX4),
            comptype: gx::GX_TEX_ST,
            compsize: gx::GX_S16,
            frac: 7,
        },
        gx::GxVtxAttrFmt {
            vtxattr: u32::from(gx::GX_VA_NULL),
            comptype: 0,
            compsize: 0,
            frac: 0,
        },
        gx::GxVtxAttrFmt {
            vtxattr: u32::from(gx::GX_VA_TEX1),
            comptype: gx::GX_TEX_ST,
            compsize: gx::GX_S16,
            frac: 9,
        },
    ];
    let expected = [
        0x08, 0x77, 0x40, 0, 0, 0x57, 0x08, 0x87, 0x38, 0, 0, 0, 0x08, 0x97, 0, 0, 0, 7, 0x97, 0, 0,
    ];
    // SAFETY: The list terminates at GX_VA_NULL. The primitive header is
    // captured in a display list and is not executed.
    logger.record(
        "GX vertex format flushes exact CP words and honors terminator",
        check_gx_command_bytes(&expected, || unsafe {
            gx::set_vtx_attr_fmt(7, u32::from(gx::GX_VA_POS), gx::GX_POS_XYZ, gx::GX_S16, 5);
            gx::set_vtx_attr_fmtv(7, formats.as_ptr());
            gx::begin(gx::GX_TRIANGLES, 7, 0);
            gx::end();
        }),
    );
}

fn probe_tev_order_scale(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX TEV order automatic texture scale");
    #[repr(align(32))]
    struct Image([u8; 8 * 4 * 4]);
    let mut image = Image([0; 8 * 4 * 4]);
    let mut texture = gx::GxTexObj::default();
    let projection = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // SAFETY: The probe owns GX and keeps the aligned texture image live.
    // Zero-vertex primitives flush state without sampling the image.
    let ready = unsafe {
        gx::configure_solid_2d(&projection);
        gx::set_tex_coord_scale_manually(3, 0, 1, 1);
        gx::set_tex_coord_cyl_wrap(3, 0, 0);
        gx::set_tex_coord_bias(3, 0, 0);
        gx::init_tex_obj(
            &mut texture,
            image.0.as_mut_ptr().cast(),
            gx::GxTexObjInit {
                wd: 8,
                ht: 4,
                fmt: gx::GX_TF_RGBA8,
                wrap_s: 0,
                wrap_t: 0,
                mipmap: 0,
            },
        );
        let loaded = gx::load_tex_obj(&mut texture, 0).is_ok();
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
        loaded
    };
    #[repr(align(32))]
    struct Trace([u8; 64]);
    let mut trace = Trace([0xa5; 64]);
    // SAFETY: GX is probe-owned; the aligned buffer remains live through capture.
    let trace_size = unsafe {
        gx::begin_disp_list(trace.0.as_mut_ptr().cast(), 64);
        gx::set_tev_order(0, 3, 0, gx::GX_COLOR0A0);
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
        gx::end_disp_list()
    };
    logger.note_u32("GX TEV order trace size", trace_size);
    for chunk in trace.0[..16].chunks_exact(4) {
        logger.note_u32(
            "GX TEV order trace bytes",
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        );
    }
    // C GX_SetTevOrder marks SU state dirty. Before the next primitive,
    // __GX_SetSUTexRegs uses map 0's 8x4 size for coordinate 3.
    logger.record(
        "GX TEV order updates automatic texture coordinate scale",
        ready
            && check_gx_command_bytes(
                &[
                    0x61, 0x28, 0, 0, 0x58, 0x61, 0x36, 0, 0, 7, 0x61, 0x37, 0, 0, 3, 0x90, 0, 0,
                ],
                || unsafe {
                    gx::set_tev_order(0, 3, 0, gx::GX_COLOR0A0);
                    gx::begin(gx::GX_TRIANGLES, 0, 0);
                    gx::end();
                },
            ),
    );
    // SAFETY: The second map uses the same live aligned image. Loading and
    // flushing it establishes map-specific dimensions and wrap fields.
    let wrapped_ready = unsafe {
        let mut wrapped = gx::GxTexObj::default();
        gx::init_tex_obj(
            &mut wrapped,
            image.0.as_mut_ptr().cast(),
            gx::GxTexObjInit {
                wd: 8,
                ht: 4,
                fmt: gx::GX_TF_RGBA8,
                wrap_s: 1,
                wrap_t: 1,
                mipmap: 0,
            },
        );
        let loaded = gx::load_tex_obj(&mut wrapped, 1).is_ok();
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
        loaded
    };
    logger.record(
        "GX TEV order selects map-specific scale and wrap fields",
        wrapped_ready
            && check_gx_command_bytes(
                &[
                    0x61, 0x28, 0, 0, 0x59, 0x61, 0x36, 1, 0, 7, 0x61, 0x37, 1, 0, 3, 0x90, 0, 0,
                ],
                || unsafe {
                    gx::set_tev_order(0, 3, 1, gx::GX_COLOR0A0);
                    gx::begin(gx::GX_TRIANGLES, 0, 0);
                    gx::end();
                },
            ),
    );
    // SAFETY: The object, region, and aligned image are live for this load.
    let preloaded_ready = unsafe {
        let mut object = gx::GxTexObj::default();
        let mut region = gx::GxTexRegion::default();
        gx::init_tex_obj(
            &mut object,
            image.0.as_mut_ptr().cast(),
            gx::GxTexObjInit {
                wd: 4,
                ht: 8,
                fmt: gx::GX_TF_RGBA8,
                wrap_s: 0,
                wrap_t: 1,
                mipmap: 0,
            },
        );
        gx::init_tex_cache_region(&mut region, 0, 0, 3, 0x80000, 3);
        let loaded = gx::load_tex_obj_preloaded(&mut object, &mut region, 2).is_ok();
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
        loaded
    };
    logger.record(
        "GX preloaded texture supplies map-specific scale",
        preloaded_ready
            && check_gx_command_bytes(
                &[
                    0x61, 0x28, 0, 0, 0x5a, 0x61, 0x36, 0, 0, 3, 0x61, 0x37, 1, 0, 7, 0x90, 0, 0,
                ],
                || unsafe {
                    gx::set_tev_order(0, 3, 2, gx::GX_COLOR0A0);
                    gx::begin(gx::GX_TRIANGLES, 0, 0);
                    gx::end();
                },
            ),
    );
    // SAFETY: The probe owns GX. Set manual scale before the capture and
    // disable it afterward so later cases exercise automatic mode.
    unsafe { gx::set_tex_coord_scale_manually(3, 1, 5, 6) };
    logger.record(
        "GX manual texture scale is not overwritten by TEV order",
        check_gx_command_bytes(&[0x61, 0x28, 0, 0, 0x58, 0x90, 0, 0], || unsafe {
            gx::set_tev_order(0, 3, 0, gx::GX_COLOR0A0);
            gx::begin(gx::GX_TRIANGLES, 0, 0);
            gx::end();
        }),
    );
    unsafe { gx::set_tex_coord_scale_manually(3, 0, 1, 1) };
    logger.record(
        "GX null TEV texture does not update automatic scale",
        check_gx_command_bytes(&[0x61, 0x28, 0, 3, 0x98, 0x90, 0, 0], || unsafe {
            gx::set_tev_order(0, 3, gx::GX_TEXMAP_NULL, gx::GX_COLORNULL);
            gx::begin(gx::GX_TRIANGLES, 0, 0);
            gx::end();
        }),
    );
    // SAFETY: Initialize a single indirect stage and clear all order fields.
    unsafe {
        gx::set_tev_order(0, 0, gx::GX_TEXMAP_NULL, gx::GX_COLORNULL);
        gx::set_tex_coord_scale_manually(2, 0, 1, 1);
        gx::set_tex_coord_cyl_wrap(2, 0, 0);
        gx::set_tex_coord_bias(2, 0, 0);
        for stage in 0..4 {
            gx::set_ind_tex_order(stage, 0, 0);
        }
        gx::set_num_ind_stages(1);
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
    }
    logger.record(
        "GX indirect texture order updates automatic scale",
        check_gx_command_bytes(
            &[
                0x61, 0x27, 0, 0, 0x10, 0x61, 0x34, 0, 0, 7, 0x61, 0x35, 0, 0, 3, 0x90, 0, 0,
            ],
            || unsafe {
                gx::set_ind_tex_order(0, 2, 0);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
            },
        ),
    );
}

fn probe_pixel_tev_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX pixel format and TEV default command bytes");
    let projection = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // SAFETY: The probe owns GX. Set known state before capture. The TEV
    // stage is inactive and later render checks configure GX again.
    unsafe {
        gx::set_z_comp_loc(1);
        gx::set_dst_alpha(1, 0x5a);
        gx::set_pixel_fmt(gx::GX_PF_Y8, gx::GX_ZC_LINEAR);
        gx::configure_solid_2d(&projection);
        gx::set_tev_swap_mode(5, 0, 0);
        gx::set_tev_color_op(5, 0, 0, 0, 0, 0);
        gx::set_tev_alpha_op(5, 0, 0, 0, 0, 0);
        gx::set_tev_color_in(5, 0, 0, 0, 0);
        gx::set_tev_alpha_in(5, 0, 0, 0, 0);
    }
    logger.record(
        "GX RGB565 format sets pixel control and deferred multisample bit",
        check_gx_command_bytes(
            &[0x61, 0x43, 0, 0, 0x52, 0x61, 0, 0, 2, 0x11, 0x90, 0, 0],
            || unsafe {
                gx::set_pixel_fmt(gx::GX_PF_RGB565_Z16, gx::GX_ZC_MID);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
            },
        ),
    );
    logger.record(
        "GX V8 format selects YUV field and invalid format emits nothing",
        check_gx_bp_words(&[0x4300_004c, 0x4200_095a], || unsafe {
            gx::set_pixel_fmt(gx::GX_PF_V8, gx::GX_ZC_NEAR);
            gx::set_pixel_fmt(8, 0);
        }),
    );
    logger.record(
        "GX modulate TEV default uses previous stage and clamp",
        check_gx_bp_words(
            &[0xca00_f80f, 0xcb00_f070, 0xca08_f80f, 0xcb08_f070],
            || unsafe {
                gx::set_tev_op(5, gx::GX_MODULATE);
            },
        ),
    );
    logger.record(
        "GX replace TEV default selects texture and clamps",
        check_gx_bp_words(
            &[0xca00_fff8, 0xcb00_ffc0, 0xca08_fff8, 0xcb08_ffc0],
            || unsafe {
                gx::set_tev_op(5, gx::GX_REPLACE);
            },
        ),
    );
}

fn probe_texgen_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX texture generator and current matrix command bytes");
    let projection = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // SAFETY: The probe owns GX. Set known low and high matrix indices and
    // flush pending state before capture. Later render probes configure GX.
    unsafe {
        gx::configure_solid_2d(&projection);
        for coord in 4..8 {
            gx::set_tex_coord_gen(coord, gx::GX_TG_MTX2X4, gx::GX_TG_TEX0, gx::GX_IDENTITY);
        }
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
    }
    let expected = [
        0x10, 0, 0, 0x10, 0x40, 0, 0, 0, 6, 0x10, 0, 0, 0x10, 0x50, 0, 0, 0, 0x3d, 0x10, 0, 0,
        0x10, 0x47, 0, 0, 1, 0x30, 0x10, 0, 0, 0x10, 0x57, 0, 0, 1, 5, 0x08, 0x30, 0, 0, 3, 7,
        0x10, 0, 0, 0x10, 0x18, 0, 0, 3, 7, 0x08, 0x40, 0, 0x57, 0xcf, 0x3c, 0x10, 0, 0, 0x10,
        0x19, 0, 0x57, 0xcf, 0x3c, 0x90, 0, 0,
    ];
    logger.record(
        "GX texture generators and matrix indices emit expected CP and XF words",
        check_gx_command_bytes(&expected, || unsafe {
            gx::set_tex_coord_gen(0, gx::GX_TG_MTX3X4, gx::GX_TG_POS, 12);
            gx::set_tex_coord_gen2(7, gx::GX_TG_SRTG, gx::GX_TG_COLOR1, 21, 1, 69);
            gx::set_tex_coord_gen2(8, gx::GX_TG_SRTG, gx::GX_TG_COLOR1, 0, 0, 64);
            gx::set_current_mtx(0x47);
            gx::begin(gx::GX_TRIANGLES, 0, 0);
            gx::end();
        }),
    );
}

fn probe_generation_mode_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX generation mode and texture offset command bytes");
    let projection = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // SAFETY: This probe owns GX. Establish a known generation word and
    // indirect map before capturing commands; later render probes reset GX.
    unsafe {
        gx::configure_solid_2d(&projection);
        gx::set_ind_tex_order(0, 0, 0);
        gx::set_ind_tex_order(1, 1, 1);
        gx::set_num_ind_stages(0);
        gx::begin(gx::GX_TRIANGLES, 0, 0);
        gx::end();
        gx::set_tex_coord_scale_manually(7, 0, 1, 1);
        gx::set_tex_coord_cyl_wrap(7, 0, 0);
        gx::set_tex_coord_bias(7, 0, 0);
        gx::enable_tex_offsets(7, 0, 0);
        gx::set_tex_coord_scale_manually(7, 1, 35, 1);
        gx::set_tex_coord_scale_manually(7, 0, 1, 1);
    }
    logger.record(
        "GX cull modes map to hardware values and reject invalid input",
        check_gx_command_bytes(
            &[
                0x61, 0, 0, 0x80, 0x11, 0x90, 0, 0, 0x61, 0, 0, 0x40, 0x11, 0x90, 0, 0, 0x90, 0, 0,
            ],
            || unsafe {
                gx::set_cull_mode(gx::GX_CULL_FRONT);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
                gx::set_cull_mode(gx::GX_CULL_BACK);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
                gx::set_cull_mode(4);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
            },
        ),
    );
    logger.record(
        "GX coplanar mode emits its mask and preserves generation state",
        check_gx_bp_words(
            &[0xfe08_0000, 0x0008_0011, 0xfe08_0000, 0x0000_0011],
            || unsafe {
                gx::set_co_planar(1);
                gx::set_co_planar(0);
            },
        ),
    );
    logger.record(
        "GX channel texture TEV and indirect counts flush expected state",
        check_gx_command_bytes(
            &[
                0x61, 0, 2, 0x0c, 0x23, 0x61, 0x0f, 0, 0, 3, 0x10, 0, 0, 0x10, 9, 0, 0, 0, 2, 0x10,
                0, 0, 0x10, 0x3f, 0, 0, 0, 3, 0x90, 0, 0,
            ],
            || unsafe {
                gx::set_num_chans(2);
                gx::set_num_tex_gens(3);
                gx::set_num_tev_stages(4);
                gx::set_num_ind_stages(2);
                gx::begin(gx::GX_TRIANGLES, 0, 0);
                gx::end();
            },
        ),
    );
    logger.record(
        "GX texture offsets preserve size and mask coordinate and enables",
        check_gx_bp_words(&[0x3e04_0022, 0x3e08_0022, 0x3e08_0022], || unsafe {
            gx::enable_tex_offsets(7, 1, 0);
            gx::enable_tex_offsets(15, 0, 1);
            gx::enable_tex_offsets(7, 2, 3);
        }),
    );
}

fn probe_lighting_channel_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX lighting channel command bytes");
    let ambient = gx::GxColor {
        r: 0x11,
        g: 0x22,
        b: 0x33,
        a: 0x44,
    };
    let alpha = gx::GxColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0x99,
    };
    let material = gx::GxColor {
        r: 0xaa,
        g: 0xbb,
        b: 0xcc,
        a: 0xdd,
    };
    let rgb = gx::GxColor {
        r: 0x12,
        g: 0x34,
        b: 0x56,
        a: 0x78,
    };
    let expected = [
        0x10, 0, 0, 0x10, 0x0a, 0x11, 0x22, 0x33, 0x99, 0x10, 0, 0, 0x10, 0x0d, 0x12, 0x34, 0x56,
        0xdd, 0x10, 0, 0, 0x10, 0x0e, 0, 0, 0x57, 0x57, 0x10, 0, 0, 0x10, 0x10, 0, 0, 0x57, 0x57,
        0x90, 0, 0,
    ];
    // SAFETY: The probe owns GX. The primitive header and deferred state
    // are captured in a display list rather than executed.
    logger.record(
        "GX channel colors and lighting control emit expected XF words",
        check_gx_command_bytes(&expected, || unsafe {
            gx::set_chan_amb_color(i32::from(gx::GX_COLOR0A0), ambient);
            gx::set_chan_amb_color(i32::from(gx::GX_ALPHA0), alpha);
            gx::set_chan_mat_color(i32::from(gx::GX_COLOR1A1), material);
            gx::set_chan_mat_color(i32::from(gx::GX_COLOR1), rgb);
            gx::set_chan_ctrl(
                i32::from(gx::GX_COLOR0A0),
                1,
                1,
                1,
                0xa5,
                gx::GX_DF_CLAMP,
                gx::GX_AF_SPOT,
            );
            gx::begin(gx::GX_TRIANGLES, 0, 0);
            gx::end();
        }),
    );
}

fn probe_viewport_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX viewport command bytes");
    // XF header: six registers starting at 0x101a. Fixed IEEE-754 words are
    // 160, -120, 16777215, 512, 482, 16777215, in C reference order.
    let expected = [
        0x10, 0, 5, 0x10, 0x1a, 0x43, 0x20, 0, 0, 0xc2, 0xf0, 0, 0, 0x4b, 0x7f, 0xff, 0xff, 0x44,
        0, 0, 0, 0x43, 0xf1, 0, 0, 0x4b, 0x7f, 0xff, 0xff,
    ];
    let matches = check_gx_command_bytes(&expected, || unsafe {
        ogc_gx::set_viewport(10.0, 20.0, 320.0, 240.0, 0.0, 1.0);
    });
    logger.record(
        "GX viewport emits exact nonzero-origin XF transform",
        matches,
    );
    // Reversed depth range and different geometry: 32, -16, -16777215,
    // 370, 364, 0. This also detects ignored near/far parameters.
    let expected = [
        0x10, 0, 5, 0x10, 0x1a, 0x42, 0, 0, 0, 0xc1, 0x80, 0, 0, 0xcb, 0x7f, 0xff, 0xff, 0x43,
        0xb9, 0, 0, 0x43, 0xb6, 0, 0, 0, 0, 0, 0,
    ];
    logger.record(
        "GX viewport encodes changed geometry and reversed depth",
        check_gx_command_bytes(&expected, || unsafe {
            ogc_gx::set_viewport(-4.0, 6.0, 64.0, 32.0, 1.0, 0.0);
        }),
    );
}

fn probe_viewport_jitter_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX viewport field jitter command bytes");
    for field in [0, 1] {
        let mut expected = [
            0x10, 0, 5, 0x10, 0x1a, 0x43, 0x20, 0, 0, 0xc2, 0xf0, 0, 0, 0x4b, 0x7f, 0xff, 0xff,
            0x44, 0, 0, 0, 0x43, 0xf1, 0, 0, 0x4b, 0x7f, 0xff, 0xff,
        ];
        if field == 0 {
            // Y origin is 481.5 (0x43f0c000), rather than 482 (0x43f10000).
            expected[22..25].copy_from_slice(&[0xf0, 0xc0, 0]);
        }
        logger.note_u32("GX viewport jitter field", field);
        logger.record(
            "GX viewport jitter emits exact field-dependent Y origin",
            check_gx_command_bytes(&expected, || unsafe {
                ogc_gx::set_viewport_jitter(10.0, 20.0, 320.0, 240.0, 0.0, 1.0, field);
            }),
        );
    }
}

fn probe_scissor_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX scissor command bytes");
    for (args, expected) in [
        (
            (17, 29, 64, 32),
            [0x61, 0x20, 0x16, 0x71, 0x73, 0x61, 0x21, 0x1a, 0x61, 0x92],
        ),
        (
            (0, 0, 0, 0),
            [0x61, 0x20, 0x15, 0x61, 0x56, 0x61, 0x21, 0x15, 0x51, 0x55],
        ),
        (
            (u32::MAX, u32::MAX, 1, 1),
            [0x61, 0x20, 0x15, 0x51, 0x55, 0x61, 0x21, 0x15, 0x51, 0x55],
        ),
    ] {
        logger.note_u32("GX scissor input width", args.2);
        logger.record(
            "GX scissor encodes bias inclusive bounds and unsigned wrap",
            check_gx_command_bytes(&expected, || unsafe {
                ogc_gx::set_scissor(args.0, args.1, args.2, args.3);
            }),
        );
    }
}

fn probe_scissor_offset_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX scissor offset command bytes");
    for ((x, y), expected) in [
        ((0, 0), [0x61, 0x59, 0x02, 0xac, 0xab]),
        ((4, -2), [0x61, 0x59, 0x02, 0xa8, 0xad]),
        ((-344, -346), [0x61, 0x59, 0x0f, 0xfb, 0xff]),
    ] {
        logger.record(
            "GX scissor offset encodes bias half units and negative wrap",
            check_gx_command_bytes(&expected, || unsafe {
                ogc_gx::set_scissor_box_offset(x, y);
            }),
        );
    }
}

fn probe_projection_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX projection matrix command bytes");
    // Distinct unused elements expose transposition and incorrect column reads.
    let matrix = [
        [2.0, 91.0, 3.0, 5.0],
        [92.0, 7.0, 11.0, 13.0],
        [93.0, 94.0, 17.0, 19.0],
        [95.0, 96.0, 97.0, 98.0],
    ];
    for projection in [gx::GX_PERSPECTIVE, gx::GX_ORTHOGRAPHIC] {
        // XF header: seven registers at 0x1020, followed by 2, 3, 7, 11,
        // 17, 19 and integer type 0. Orthographic selects 5 and 13 instead.
        let mut expected = [
            0x10, 0, 6, 0x10, 0x20, 0x40, 0, 0, 0, 0x40, 0x40, 0, 0, 0x40, 0xe0, 0, 0, 0x41, 0x30,
            0, 0, 0x41, 0x88, 0, 0, 0x41, 0x98, 0, 0, 0, 0, 0, 0,
        ];
        if projection == gx::GX_ORTHOGRAPHIC {
            expected[10] = 0xa0;
            expected[18] = 0x50;
            expected[32] = 1;
        }
        logger.note_u32("GX projection type", u32::from(projection));
        logger.record(
            "GX projection emits exact selected columns and integer type",
            check_gx_command_bytes(&expected, || unsafe {
                gx::load_projection_mtx(&matrix, projection);
            }),
        );
    }
}

// Fixed row-major inputs and independent IEEE-754 expectations. The fourth
// column must be omitted by normal-matrix loads; texture 2x4 omits row three.
const MATRIX_INPUT_3X4: [[f32; 4]; 3] = [
    [1.0, -2.0, 3.0, 4.0],
    [5.0, -6.0, 7.0, 8.0],
    [9.0, -10.0, 11.0, 12.0],
];
const MATRIX_WORDS_3X4: [u32; 12] = [
    0x3f80_0000,
    0xc000_0000,
    0x4040_0000,
    0x4080_0000,
    0x40a0_0000,
    0xc0c0_0000,
    0x40e0_0000,
    0x4100_0000,
    0x4110_0000,
    0xc120_0000,
    0x4130_0000,
    0x4140_0000,
];
const MATRIX_WORDS_3X3: [u32; 9] = [
    0x3f80_0000,
    0xc000_0000,
    0x4040_0000,
    0x40a0_0000,
    0xc0c0_0000,
    0x40e0_0000,
    0x4110_0000,
    0xc120_0000,
    0x4130_0000,
];

fn check_gx_matrix_command(header: [u8; 5], words: &[u32], emit: impl FnOnce()) -> bool {
    let mut expected = [0; 53];
    if words.len() > 12 {
        return false;
    }
    let size = 5 + words.len() * 4;
    expected[..5].copy_from_slice(&header);
    for (bytes, word) in expected[5..size].chunks_exact_mut(4).zip(words) {
        bytes.copy_from_slice(&word.to_be_bytes());
    }
    check_gx_command_bytes(&expected[..size], emit)
}

fn probe_position_matrix_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX immediate position matrix commands");
    // Twelve XF words, destination pnidx * 4: slots 3 and 9 give 0x0c/0x24.
    for (slot, header) in [(3, [0x10, 0, 0x0b, 0, 0x0c]), (9, [0x10, 0, 0x0b, 0, 0x24])] {
        logger.note_u32("GX position matrix slot", slot);
        logger.record(
            "GX position matrix emits exact row order and destination",
            check_gx_matrix_command(header, &MATRIX_WORDS_3X4, || unsafe {
                // SAFETY: Capture owns GX and the complete matrix stays live.
                ogc_gx::load_pos_mtx_imm(&MATRIX_INPUT_3X4, slot);
            }),
        );
    }
}

fn probe_indexed_position_matrix_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX indexed position matrix commands");
    for ((index, slot), expected) in [
        ((0x1234, 3), [0x20, 0x12, 0x34, 0xb0, 0x0c]),
        ((0xabcd, 9), [0x20, 0xab, 0xcd, 0xb0, 0x24]),
    ] {
        logger.record(
            "GX indexed position encodes source index length and slot",
            check_gx_command_bytes(&expected, || unsafe {
                // SAFETY: Capture does not submit this indexed memory read.
                ogc_gx::load_pos_mtx_idx(index, slot);
            }),
        );
    }
}

fn probe_normal_matrix_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX normal matrix from 3x4 commands");
    // Nine XF words at normal memory 0x400 + pnidx * 3.
    for (slot, header) in [(3, [0x10, 0, 8, 4, 9]), (9, [0x10, 0, 8, 4, 0x1b])] {
        logger.record(
            "GX normal matrix omits translation and selects normal memory",
            check_gx_matrix_command(header, &MATRIX_WORDS_3X3, || unsafe {
                // SAFETY: Capture owns GX and the complete matrix stays live.
                ogc_gx::load_nrm_mtx_imm(&MATRIX_INPUT_3X4, slot);
            }),
        );
    }
}

fn probe_normal_matrix_3x3_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX immediate 3x3 normal matrix commands");
    let matrix = [[1.0, -2.0, 3.0], [5.0, -6.0, 7.0], [9.0, -10.0, 11.0]];
    for (slot, header) in [(3, [0x10, 0, 8, 4, 9]), (9, [0x10, 0, 8, 4, 0x1b])] {
        logger.record(
            "GX 3x3 normal matrix emits all nine words in row order",
            check_gx_matrix_command(header, &MATRIX_WORDS_3X3, || unsafe {
                // SAFETY: Capture owns GX and the complete matrix stays live.
                ogc_gx::load_nrm_mtx_imm3x3(&matrix, slot);
            }),
        );
    }
}

fn probe_indexed_normal_matrix_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX indexed normal matrix commands");
    for ((index, slot), expected) in [
        ((0x1234, 3), [0x28, 0x12, 0x34, 0x84, 9]),
        ((0xabcd, 9), [0x28, 0xab, 0xcd, 0x84, 0x1b]),
    ] {
        logger.record(
            "GX indexed normal encodes source index length and address",
            check_gx_command_bytes(&expected, || unsafe {
                // SAFETY: Capture does not submit this indexed memory read.
                ogc_gx::load_nrm_mtx_idx3x3(index, slot);
            }),
        );
    }
}

fn probe_texture_matrix_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX immediate texture matrix commands");
    // Texture slot 30 maps to 0x78; post-transform slot 67 maps to 0x50c.
    // The fixed header distinguishes eight-word and twelve-word transfers.
    for (slot, kind, header, words) in [
        (30, gx::GX_MTX2X4, [0x10, 0, 7, 0, 0x78], 8),
        (30, gx::GX_MTX3X4, [0x10, 0, 0x0b, 0, 0x78], 12),
        (67, gx::GX_MTX2X4, [0x10, 0, 7, 5, 0x0c], 8),
        (67, gx::GX_MTX3X4, [0x10, 0, 0x0b, 5, 0x0c], 12),
    ] {
        logger.note_u32("GX texture matrix slot", slot);
        logger.note_u32("GX texture matrix word count", words as u32);
        logger.record(
            "GX texture matrix emits exact format rows and memory bank",
            check_gx_matrix_command(header, &MATRIX_WORDS_3X4[..words], || unsafe {
                // SAFETY: Capture owns GX and the complete matrix stays live.
                gx::load_tex_mtx_imm(&MATRIX_INPUT_3X4, slot, kind);
            }),
        );
    }
}

fn probe_indexed_texture_matrix_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX indexed texture matrix commands");
    for ((index, slot, kind), expected) in [
        ((0x1234, 30, gx::GX_MTX2X4), [0x30, 0x12, 0x34, 0x70, 0x78]),
        ((0x1234, 30, gx::GX_MTX3X4), [0x30, 0x12, 0x34, 0xb0, 0x78]),
        ((0xabcd, 67, gx::GX_MTX2X4), [0x30, 0xab, 0xcd, 0x75, 0x0c]),
        ((0xabcd, 67, gx::GX_MTX3X4), [0x30, 0xab, 0xcd, 0xb5, 0x0c]),
    ] {
        logger.record(
            "GX indexed texture encodes source format length and bank",
            check_gx_command_bytes(&expected, || unsafe {
                // SAFETY: Capture does not submit this indexed memory read.
                gx::load_tex_mtx_idx(index, slot, kind);
            }),
        );
    }
}

fn check_signed_vertex_capture(
    logger: &mut ProbeLogger,
    label: &str,
    expected: &[u8],
    emit_zero: impl FnOnce(),
    emit_signed: impl FnOnce(),
) {
    // If a writer bypasses capture, zero data reaches the GPU as NOP bytes.
    // Do not submit signed test patterns until the zero capture succeeds.
    let zeros = [0; 6];
    let captured_zero = check_gx_command_bytes(&zeros[..expected.len()], emit_zero);
    logger.record(label, captured_zero);
    if captured_zero {
        logger.record(label, check_gx_command_bytes(expected, emit_signed));
    } else {
        logger.report_step("Signed boundary case not run: zero capture failed");
    }
}

fn probe_signed_position_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX signed position capture and boundary bytes");
    // SAFETY: GX is owned by this probe. Each writer runs in display-list
    // capture; the zero case detects bypass before any signed pattern is used.
    check_signed_vertex_capture(
        logger,
        "GX position3s16 captures zeros and exact signed boundary bytes",
        &[0x80, 0, 0x7f, 0xff, 0xff, 0xff],
        || unsafe { gx::position3s16(0, 0, 0) },
        || unsafe { gx::position3s16(i16::MIN, i16::MAX, -1) },
    );
    check_signed_vertex_capture(
        logger,
        "GX position3s8 captures zeros and exact signed boundary bytes",
        &[0x80, 0x7f, 0xff],
        || unsafe { gx::position3s8(0, 0, 0) },
        || unsafe { gx::position3s8(i8::MIN, i8::MAX, -1) },
    );
    check_signed_vertex_capture(
        logger,
        "GX position2s16 captures zeros and exact signed boundary bytes",
        &[0x80, 0, 0x7f, 0xff],
        || unsafe { gx::position2s16(0, 0) },
        || unsafe { gx::position2s16(i16::MIN, i16::MAX) },
    );
    check_signed_vertex_capture(
        logger,
        "GX position2s8 captures zeros and exact signed boundary bytes",
        &[0x80, 0x7f],
        || unsafe { gx::position2s8(0, 0) },
        || unsafe { gx::position2s8(i8::MIN, i8::MAX) },
    );
}

fn probe_signed_normal_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX signed normal capture and boundary bytes");
    // SAFETY: The probe owns GX and checks zero capture before signed data.
    check_signed_vertex_capture(
        logger,
        "GX normal3s16 captures zeros and exact signed boundary bytes",
        &[0x7f, 0xff, 0xff, 0xff, 0x80, 0],
        || unsafe { gx::normal3s16(0, 0, 0) },
        || unsafe { gx::normal3s16(i16::MAX, -1, i16::MIN) },
    );
    check_signed_vertex_capture(
        logger,
        "GX normal3s8 captures zeros and exact signed boundary bytes",
        &[0x7f, 0xff, 0x80],
        || unsafe { gx::normal3s8(0, 0, 0) },
        || unsafe { gx::normal3s8(i8::MAX, -1, i8::MIN) },
    );
}

fn probe_signed_texture_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX signed texture coordinate capture and bytes");
    // SAFETY: The probe owns GX and checks zero capture before signed data.
    check_signed_vertex_capture(
        logger,
        "GX tex_coord2s16 captures zeros and exact signed boundary bytes",
        &[0x7f, 0xff, 0x80, 0],
        || unsafe { gx::tex_coord2s16(0, 0) },
        || unsafe { gx::tex_coord2s16(i16::MAX, i16::MIN) },
    );
    check_signed_vertex_capture(
        logger,
        "GX tex_coord2s8 captures zeros and exact signed boundary bytes",
        &[0x7f, 0x80],
        || unsafe { gx::tex_coord2s8(0, 0) },
        || unsafe { gx::tex_coord2s8(i8::MAX, i8::MIN) },
    );
    check_signed_vertex_capture(
        logger,
        "GX tex_coord1s16 captures zeros and exact signed boundary bytes",
        &[0xff, 0xff],
        || unsafe { gx::tex_coord1s16(0) },
        || unsafe { gx::tex_coord1s16(-1) },
    );
    check_signed_vertex_capture(
        logger,
        "GX tex_coord1s8 captures zeros and exact signed boundary bytes",
        &[0xff],
        || unsafe { gx::tex_coord1s8(0) },
        || unsafe { gx::tex_coord1s8(-1) },
    );
}

fn probe_vertex_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX vertex color capture bytes");
    // SAFETY: Each writer runs in the probe-owned display-list capture.
    for bytes in [[0x12, 0x34, 0x56, 0x78], [0xff, 0, 0x80, 1]] {
        logger.record(
            "GX color4u8 captures exact RGBA channel order",
            check_gx_command_bytes(&bytes, || unsafe {
                gx::color4u8(bytes[0], bytes[1], bytes[2], bytes[3]);
            }),
        );
    }
    for bytes in [[0x12, 0x34, 0x56], [0xff, 0, 0x80]] {
        logger.record(
            "GX color3u8 captures exactly three RGB bytes",
            check_gx_command_bytes(&bytes, || unsafe {
                gx::color3u8(bytes[0], bytes[1], bytes[2]);
            }),
        );
    }
    // In-range conversion multiplies by 255 and truncates toward zero.
    // Fixed expectations are 127/63/191 and 255/0/31.
    for (input, expected) in [
        ([0.5, 0.25, 0.75], [0x7f, 0x3f, 0xbf]),
        ([1.0, 0.0, 0.125], [0xff, 0, 0x1f]),
    ] {
        logger.record(
            "GX color3f32 captures fixed normalized conversion bytes",
            check_gx_command_bytes(&expected, || unsafe {
                gx::color3f32(input[0], input[1], input[2]);
            }),
        );
    }
    for (input, expected) in [
        (0x1234_5678, [0x12, 0x34, 0x56, 0x78]),
        (0xff00_8001, [0xff, 0, 0x80, 1]),
    ] {
        logger.record(
            "GX color1u32 captures exact big-endian packed color",
            check_gx_command_bytes(&expected, || unsafe { gx::color1u32(input) }),
        );
    }
    for (input, expected) in [(0x1234, [0x12, 0x34]), (0x80ff, [0x80, 0xff])] {
        logger.record(
            "GX color1u16 captures exactly two packed bytes",
            check_gx_command_bytes(&expected, || unsafe { gx::color1u16(input) }),
        );
    }
}

fn probe_vertex_indices_8(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX eight-bit vertex index capture");
    // SAFETY: These writers run only inside capture; no array read is submitted.
    let writers: [(&str, fn(u8)); 5] = [
        ("GX position1x8 exact index bytes", |index| unsafe {
            gx::position1x8(index)
        }),
        ("GX normal1x8 exact index bytes", |index| unsafe {
            gx::normal1x8(index)
        }),
        ("GX color1x8 exact index bytes", |index| unsafe {
            gx::color1x8(index)
        }),
        ("GX tex_coord1x8 exact index bytes", |index| unsafe {
            gx::tex_coord1x8(index)
        }),
        ("GX matrix_index1x8 exact index bytes", |index| unsafe {
            gx::matrix_index1x8(index)
        }),
    ];
    for (label, write) in writers {
        for index in [0x12, 0xff] {
            logger.record(label, check_gx_command_bytes(&[index], || write(index)));
        }
    }
}

fn probe_vertex_indices_16(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX sixteen-bit vertex index capture");
    // SAFETY: These writers run only inside capture; no array read is submitted.
    let writers: [(&str, fn(u16)); 4] = [
        ("GX position1x16 exact big-endian index", |index| unsafe {
            gx::position1x16(index)
        }),
        ("GX normal1x16 exact big-endian index", |index| unsafe {
            gx::normal1x16(index)
        }),
        ("GX color1x16 exact big-endian index", |index| unsafe {
            gx::color1x16(index)
        }),
        ("GX tex_coord1x16 exact big-endian index", |index| unsafe {
            gx::tex_coord1x16(index)
        }),
    ];
    for (label, write) in writers {
        for (index, expected) in [(0x1234, [0x12, 0x34]), (0xffff, [0xff, 0xff])] {
            logger.record(label, check_gx_command_bytes(&expected, || write(index)));
        }
    }
}

fn probe_float_vertex_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX floating-point vertex data bytes");
    // SAFETY: Every writer below runs only in the probe-owned capture.
    // Expectations are fixed IEEE-754 bytes, including the sign of zero.
    for (input, expected) in [
        (
            [1.5, -2.25, 0.125],
            [0x3f, 0xc0, 0, 0, 0xc0, 0x10, 0, 0, 0x3e, 0, 0, 0],
        ),
        (
            [-0.0, 0.5, -0.75],
            [0x80, 0, 0, 0, 0x3f, 0, 0, 0, 0xbf, 0x40, 0, 0],
        ),
    ] {
        logger.record(
            "GX position3f32 exact XYZ float bits and width",
            check_gx_command_bytes(&expected, || unsafe {
                gx::position3f32(input[0], input[1], input[2]);
            }),
        );
    }
    for (input, expected) in [
        ([1.5, -2.25], [0x3f, 0xc0, 0, 0, 0xc0, 0x10, 0, 0]),
        ([-0.0, 0.5], [0x80, 0, 0, 0, 0x3f, 0, 0, 0]),
    ] {
        logger.record(
            "GX position2f32 exact XY float bits and width",
            check_gx_command_bytes(&expected, || unsafe {
                gx::position2f32(input[0], input[1]);
            }),
        );
        logger.record(
            "GX tex_coord2f32 exact ST float bits and width",
            check_gx_command_bytes(&expected, || unsafe {
                gx::tex_coord2f32(input[0], input[1]);
            }),
        );
    }
    for (input, expected) in [
        (
            [-0.5, 0.25, -1.0],
            [0xbf, 0, 0, 0, 0x3e, 0x80, 0, 0, 0xbf, 0x80, 0, 0],
        ),
        (
            [0.0, 1.0, 0.5],
            [0, 0, 0, 0, 0x3f, 0x80, 0, 0, 0x3f, 0, 0, 0],
        ),
    ] {
        logger.record(
            "GX normal3f32 exact XYZ float bits and width",
            check_gx_command_bytes(&expected, || unsafe {
                gx::normal3f32(input[0], input[1], input[2]);
            }),
        );
    }
    for (input, expected) in [(1.5, [0x3f, 0xc0, 0, 0]), (-0.0, [0x80, 0, 0, 0])] {
        logger.record(
            "GX tex_coord1f32 exact single float bits and width",
            check_gx_command_bytes(&expected, || unsafe { gx::tex_coord1f32(input) }),
        );
    }
}

fn probe_unsigned_position_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX unsigned position data bytes");
    // SAFETY: Each writer runs only in the probe-owned display-list capture.
    for (input, expected) in [
        ([0x1234, 0x8000, 0xffff], [0x12, 0x34, 0x80, 0, 0xff, 0xff]),
        ([0, 1, 0xabcd], [0, 0, 0, 1, 0xab, 0xcd]),
    ] {
        logger.record(
            "GX position3u16 exact unsigned XYZ big-endian bytes",
            check_gx_command_bytes(&expected, || unsafe {
                gx::position3u16(input[0], input[1], input[2]);
            }),
        );
    }
    for input in [[0x12, 0x80, 0xff], [0, 1, 0xab]] {
        logger.record(
            "GX position3u8 exact unsigned XYZ bytes",
            check_gx_command_bytes(&input, || unsafe {
                gx::position3u8(input[0], input[1], input[2]);
            }),
        );
    }
    for (input, expected) in [
        ([0x1234, 0xffff], [0x12, 0x34, 0xff, 0xff]),
        ([0, 0x8000], [0, 0, 0x80, 0]),
    ] {
        logger.record(
            "GX position2u16 exact unsigned XY big-endian bytes",
            check_gx_command_bytes(&expected, || unsafe {
                gx::position2u16(input[0], input[1]);
            }),
        );
    }
    for input in [[0x12, 0xff], [0, 0x80]] {
        logger.record(
            "GX position2u8 exact unsigned XY bytes",
            check_gx_command_bytes(&input, || unsafe {
                gx::position2u8(input[0], input[1]);
            }),
        );
    }
}

fn probe_unsigned_texture_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX unsigned texture coordinate data bytes");
    // SAFETY: Each writer runs only in the probe-owned display-list capture.
    for (input, expected) in [
        ([0x1234, 0xffff], [0x12, 0x34, 0xff, 0xff]),
        ([0, 0x8000], [0, 0, 0x80, 0]),
    ] {
        logger.record(
            "GX tex_coord2u16 exact unsigned ST big-endian bytes",
            check_gx_command_bytes(&expected, || unsafe {
                gx::tex_coord2u16(input[0], input[1]);
            }),
        );
    }
    for input in [[0x12, 0xff], [0, 0x80]] {
        logger.record(
            "GX tex_coord2u8 exact unsigned ST bytes",
            check_gx_command_bytes(&input, || unsafe {
                gx::tex_coord2u8(input[0], input[1]);
            }),
        );
    }
    for (input, expected) in [(0x1234, [0x12, 0x34]), (0xffff, [0xff, 0xff])] {
        logger.record(
            "GX tex_coord1u16 exact unsigned single big-endian value",
            check_gx_command_bytes(&expected, || unsafe { gx::tex_coord1u16(input) }),
        );
    }
    for input in [0, 0xff] {
        logger.record(
            "GX tex_coord1u8 exact unsigned single byte",
            check_gx_command_bytes(&[input], || unsafe { gx::tex_coord1u8(input) }),
        );
    }
}

fn check_gx_bp_words(words: &[u32], emit: impl FnOnce()) -> bool {
    let mut expected = [0; 30];
    if words.len() > expected.len() / 5 {
        return false;
    }
    let size = words.len() * 5;
    for (bytes, word) in expected[..size].chunks_exact_mut(5).zip(words) {
        bytes[0] = 0x61;
        bytes[1..].copy_from_slice(&word.to_be_bytes());
    }
    check_gx_command_bytes(&expected[..size], emit)
}

fn probe_pe_control_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX pixel engine control command bytes");
    // SAFETY: GX is initialized and owned by this probe. Establish known
    // fields before the captured commands; later render probes configure GX.
    unsafe {
        gx::set_blend_mode(
            gx::GX_BM_NONE,
            gx::GX_BL_ONE,
            gx::GX_BL_ZERO,
            gx::GX_LO_CLEAR,
        );
        gx::set_color_update(1);
        gx::set_alpha_update(0);
        gx::set_dither(0);
        gx::set_dst_alpha(0, 0);
        gx::set_pixel_fmt(gx::GX_PF_RGB8_Z24, 0);
        gx::set_z_comp_loc(0);
    }
    logger.record(
        "GX alpha update and dither preserve neighboring fields",
        check_gx_bp_words(
            &[0x4100_0118, 0x4100_011c, 0x4100_010c, 0x4100_0108],
            || unsafe {
                gx::set_alpha_update(1);
                gx::set_dither(1);
                gx::set_alpha_update(0);
                gx::set_dither(0);
            },
        ),
    );
    logger.record(
        "GX destination alpha encodes enable and value",
        check_gx_bp_words(&[0x4200_01a5, 0x4200_005a], || unsafe {
            gx::set_dst_alpha(1, 0xa5);
            gx::set_dst_alpha(0, 0x5a);
        }),
    );
    logger.record(
        "GX depth comparison location toggles the expected bit",
        check_gx_bp_words(&[0x4300_0040, 0x4300_0000], || unsafe {
            gx::set_z_comp_loc(1);
            gx::set_z_comp_loc(0);
        }),
    );
}

fn probe_tev_indirect_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX indirect TEV command bytes");
    let params = gx::GxTevIndirect {
        indtexid: 3,
        format: 2,
        bias: 5,
        mtxid: 9,
        wrap_s: 3,
        wrap_t: 4,
        addprev: 1,
        utclod: 1,
        alpha: 2,
    };
    logger.record(
        "GX indirect TEV packs all fields and direct resets them",
        check_gx_bp_words(&[0x151c_735b, 0x1500_0000], || unsafe {
            gx::set_tev_indirect(5, params);
            gx::set_tev_direct(5);
        }),
    );
    let masked = gx::GxTevIndirect {
        indtexid: 0xff,
        format: 0xff,
        bias: 0xff,
        mtxid: 0xff,
        wrap_s: 0xff,
        wrap_t: 0xff,
        addprev: 0xff,
        utclod: 0xff,
        alpha: 0xff,
    };
    logger.record(
        "GX indirect TEV masks stage and input fields",
        check_gx_bp_words(&[0x1f1f_ffff], || unsafe {
            gx::set_tev_indirect(0xff, masked)
        }),
    );
}

fn probe_indirect_texture_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX indirect texture scale and order bytes");
    // SAFETY: GX is owned by this probe. Reset all fields shared by the
    // registers before capturing independently calculated words.
    unsafe {
        for stage in 0..4 {
            gx::set_ind_tex_coord_scale(stage, 0, 0);
            gx::set_ind_tex_order(stage, 0, 0);
        }
    }
    logger.record(
        "GX indirect scale packs four stages and preserves neighbors",
        check_gx_bp_words(
            &[0x2500_0021, 0x2500_4321, 0x2600_0065, 0x2600_8765],
            || unsafe {
                gx::set_ind_tex_coord_scale(0, 1, 2);
                gx::set_ind_tex_coord_scale(1, 3, 4);
                gx::set_ind_tex_coord_scale(2, 5, 6);
                gx::set_ind_tex_coord_scale(3, 7, 8);
                gx::set_ind_tex_coord_scale(4, 1, 1);
            },
        ),
    );
    logger.record(
        "GX indirect order packs four coordinate and map pairs",
        check_gx_bp_words(
            &[0x2700_0011, 0x2700_0d11, 0x2702_8d11, 0x27ee_8d11],
            || unsafe {
                gx::set_ind_tex_order(0, 2, 1);
                gx::set_ind_tex_order(1, 6, 4);
                gx::set_ind_tex_order(2, 5, 0);
                gx::set_ind_tex_order(3, 7, 3);
            },
        ),
    );
}

fn probe_indirect_helper_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX indirect TEV helper command bytes");
    logger.record(
        "GX indirect bump ST emits three stage commands",
        check_gx_bp_words(&[0x1406_ca32, 0x1516_d232, 0x1610_0002], || unsafe {
            gx::set_tev_ind_bump_st(4, 2, gx::GX_ITM_0);
        }),
    );
    logger.record(
        "GX indirect bump XYZ emits bias and matrix",
        check_gx_bp_words(&[0x1700_0671], || unsafe {
            gx::set_tev_ind_bump_xyz(7, 1, gx::GX_ITM_2);
        }),
    );
    logger.record(
        "GX indirect repeat emits wrap and previous offset",
        check_gx_bp_words(&[0x1816_c000], || unsafe { gx::set_tev_ind_repeat(8) }),
    );
    logger.record(
        "GX indirect warp emits selected bias and wrap",
        check_gx_bp_words(&[0x1906_c472, 0x1a00_0402], || unsafe {
            gx::set_tev_ind_warp(9, 2, 1, 1, gx::GX_ITM_1);
            gx::set_tev_ind_warp(10, 2, 0, 0, gx::GX_ITM_1);
        }),
    );
}

fn probe_indirect_matrix_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX indirect texture matrix and tile bytes");
    let matrix = [[0.5, -0.25, 0.0], [0.25, 0.5, -0.5]];
    logger.record(
        "GX indirect matrix packs signed coefficients and exponent",
        check_gx_bp_words(&[0x0908_0200, 0x0a10_0700, 0x0b70_0000], || unsafe {
            gx::set_ind_tex_matrix(gx::GX_ITM_1, &matrix, -1);
        }),
    );
    logger.record(
        "GX indirect matrix rejects null input",
        check_gx_command_bytes(&[], || unsafe {
            gx::set_ind_tex_matrix(gx::GX_ITM_1, core::ptr::null(), 0);
        }),
    );
    let tile = gx::GxTevIndirectTile {
        indtexid: 2,
        tilesize_x: 32,
        tilesize_y: 64,
        tilespacing_x: 32,
        tilespacing_y: 64,
        indtexfmt: 1,
        indtexmtx: gx::GX_ITM_0,
        bias_sel: 3,
        alpha_sel: 1,
    };
    logger.record(
        "GX indirect tile emits spacing matrix and wrap fields",
        check_gx_bp_words(
            &[0x06c0_0020, 0x0782_0000, 0x0840_0000, 0x150b_82b6],
            || unsafe {
                gx::set_tev_ind_tile(5, tile);
            },
        ),
    );
}

fn probe_tev_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX TEV color register command bytes");
    for (id, color, words) in [
        (
            0,
            gx::GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0x78,
            },
            [0xe007_8012, 0xe103_4056, 0xe103_4056, 0xe103_4056],
        ),
        (
            3,
            gx::GxColor {
                r: 0xff,
                g: 0,
                b: 0x80,
                a: 1,
            },
            [0xe600_10ff, 0xe700_0080, 0xe700_0080, 0xe700_0080],
        ),
    ] {
        logger.record(
            "GX TEV color encodes RA BG slots and three BG writes",
            check_gx_bp_words(&words, || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                gx::set_tev_color(id, color);
            }),
        );
    }
}

fn probe_tev_combiner_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX TEV combiner command bytes");
    // SAFETY: GX is initialized and the probe owns its state. Stage 5 is
    // inactive; initialize both words so every expected bit is independent.
    unsafe {
        gx::set_tev_swap_mode(5, 0, 0);
        gx::set_tev_color_op(5, 0, 0, 0, 0, 0);
        gx::set_tev_alpha_op(5, 0, 0, 0, 0, 0);
        gx::set_tev_color_in(5, 0, 0, 0, 0);
        gx::set_tev_alpha_in(5, 0, 0, 0, 0);
    }
    // BP register addresses CA and CB belong to stage 5. Values below are
    // independently packed from the C reference's bit positions.
    logger.record(
        "GX TEV input and arithmetic operation preserve other fields",
        check_gx_bp_words(
            &[0xca00_1234, 0xcb00_29c0, 0xcaad_1234, 0xcb47_29c0],
            || unsafe {
                gx::set_tev_color_in(0x15, 1, 2, 3, 4);
                gx::set_tev_alpha_in(5, 1, 2, 3, 4);
                gx::set_tev_color_op(5, 1, 1, 2, 1, 2);
                gx::set_tev_alpha_op(5, 9, 2, 3, 0, 1);
            },
        ),
    );
}

fn probe_tev_selector_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;
    logger.report_step("GX TEV constant selectors and swap command bytes");
    // SAFETY: The probe owns GX. These inactive stages and table are set to
    // known fields before capture so the expected BP words are fixed.
    unsafe {
        gx::set_tev_k_color_sel(4, 0);
        gx::set_tev_k_color_sel(5, 0);
        gx::set_tev_k_alpha_sel(4, 0);
        gx::set_tev_k_alpha_sel(5, 0);
        gx::set_tev_k_color_sel(6, 0);
        gx::set_tev_k_color_sel(7, 0);
        gx::set_tev_k_alpha_sel(6, 0);
        gx::set_tev_k_alpha_sel(7, 0);
        gx::set_tev_swap_mode_table(1, 0, 0, 0, 0);
        gx::set_tev_alpha_op(5, 0, 0, 0, 0, 0);
        gx::set_tev_alpha_in(5, 0, 0, 0, 0);
        gx::set_tev_swap_mode(5, 0, 0);
    }
    // The C reference places the table in F8/F9, followed by the even and
    // odd stage selectors in F8. Every word includes the unchanged fields.
    logger.record(
        "GX TEV swap table and even odd selectors encode exact fields",
        check_gx_bp_words(
            &[
                0xf800_0009,
                0xf900_000f,
                0xf800_0129,
                0xf800_0b29,
                0xf806_cb29,
                0xf86e_cb29,
            ],
            || unsafe {
                gx::set_tev_swap_mode_table(1, 1, 2, 3, 3);
                gx::set_tev_k_color_sel(4, 0x12);
                gx::set_tev_k_alpha_sel(4, 5);
                gx::set_tev_k_color_sel(5, 0x1b);
                gx::set_tev_k_alpha_sel(5, 0x0d);
            },
        ),
    );
    logger.record(
        "GX TEV swap mode encodes raster and texture selections",
        check_gx_bp_words(&[0xcb00_000e], || unsafe {
            gx::set_tev_swap_mode(5, 2, 3);
        }),
    );
}

fn probe_tex_coord_scale_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX texture coordinate scale and flag command bytes");
    // SAFETY: The probe owns GX. Coordinate 7 is inactive, and these calls
    // establish a known manual flag, size, wrap, and bias before capture.
    unsafe {
        gx::set_tex_coord_scale_manually(7, 0, 1, 1);
        gx::set_tex_coord_cyl_wrap(7, 0, 0);
        gx::set_tex_coord_bias(7, 0, 0);
        gx::set_tex_coord_scale_manually(7, 1, 1, 1);
        gx::set_tex_coord_scale_manually(7, 0, 1, 1);
    }
    // Registers 3e and 3f are coordinate 7 S and T. The C reference puts
    // size minus one in bits 0..15, bias in bit 16, and wrap in bit 17.
    logger.record(
        "GX manual texture scale emits sizes and deferred flags",
        check_gx_bp_words(
            &[
                0x3e02_007f,
                0x3f01_001f,
                0x3e00_007f,
                0x3f03_001f,
                0x3e01_007f,
                0x3f02_001f,
            ],
            || unsafe {
                gx::set_tex_coord_cyl_wrap(7, 1, 0);
                gx::set_tex_coord_bias(7, 0, 1);
                gx::set_tex_coord_scale_manually(7, 1, 128, 32);
                gx::set_tex_coord_cyl_wrap(7, 0, 1);
                gx::set_tex_coord_bias(7, 1, 0);
                gx::set_tex_coord_scale_manually(7, 0, 12, 13);
                gx::set_tex_coord_cyl_wrap(7, 0, 0);
            },
        ),
    );
    logger.record(
        "GX manual texture scale wraps zero size to sixteen bits",
        check_gx_bp_words(&[0x3e00_ffff, 0x3f00_0000], || unsafe {
            gx::set_tex_coord_scale_manually(7, 1, 0, 1);
        }),
    );
}

fn probe_tev_signed_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX signed TEV color register bytes");
    for (id, color, words) in [
        (
            0,
            gx::GxColorS10 {
                r: -1024,
                g: 1023,
                b: -1,
                a: -17,
            },
            [0xe07e_f400, 0xe13f_f7ff, 0xe13f_f7ff, 0xe13f_f7ff],
        ),
        (
            3,
            gx::GxColorS10 {
                r: 1023,
                g: -1024,
                b: -17,
                a: -1,
            },
            [0xe67f_f3ff, 0xe740_07ef, 0xe740_07ef, 0xe740_07ef],
        ),
    ] {
        logger.record(
            "GX signed TEV color preserves exact eleven-bit fields",
            check_gx_bp_words(&words, || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                gx::set_tev_color_s10(id, color);
            }),
        );
    }
}

fn probe_tev_k_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX constant TEV color register bytes");
    for (id, color, words) in [
        (
            0,
            gx::GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0x78,
            },
            [0xe087_8012, 0xe183_4056, 0xe183_4056, 0xe183_4056],
        ),
        (
            3,
            gx::GxColor {
                r: 0xff,
                g: 0,
                b: 0x80,
                a: 1,
            },
            [0xe680_10ff, 0xe780_0080, 0xe780_0080, 0xe780_0080],
        ),
    ] {
        logger.record(
            "GX constant TEV color sets bank bit and exact channel fields",
            check_gx_bp_words(&words, || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                gx::set_tev_k_color(id, color);
            }),
        );
    }
}

fn probe_tev_k_signed_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX signed constant TEV color register bytes");
    for (id, color, words) in [
        (
            0,
            gx::GxColorS10 {
                r: -1024,
                g: 1023,
                b: -1,
                a: -17,
            },
            [0xe0fe_f400, 0xe1bf_f7ff, 0xe1bf_f7ff, 0xe1bf_f7ff],
        ),
        (
            3,
            gx::GxColorS10 {
                r: 1023,
                g: -1024,
                b: -17,
                a: -1,
            },
            [0xe6ff_f3ff, 0xe7c0_07ef, 0xe7c0_07ef, 0xe7c0_07ef],
        ),
    ] {
        logger.record(
            "GX signed constant TEV color encodes bank and signed fields",
            check_gx_bp_words(&words, || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                gx::set_tev_k_color_s10(id, color);
            }),
        );
    }
}

fn probe_alpha_compare_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX alpha comparison command bytes");
    for ((comp0, ref0, operation, comp1, ref1), word) in [
        ((1, 0x12, 2, 6, 0xab), 0xf3b1_ab12),
        ((7, 0xff, 3, 0, 0), 0xf3c7_00ff),
    ] {
        logger.record(
            "GX alpha comparison encodes both references and operations",
            check_gx_bp_words(&[word], || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                ogc_gx::set_alpha_compare(comp0, ref0, operation, comp1, ref1);
            }),
        );
    }
}

fn probe_fog_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX fog and range adjustment command bytes");
    let color = gx::GxColor {
        r: 0x12,
        g: 0x34,
        b: 0x56,
        a: 0x78,
    };
    // SAFETY: The probe owns GX. These commands are captured in display
    // lists and are not executed; the table remains live during capture.
    logger.record(
        "GX perspective fog emits expected coefficients and color",
        check_gx_bp_words(
            &[
                0xee03_e800,
                0xef80_001e,
                0xf000_0002,
                0xf143_f000,
                0xf212_3456,
            ],
            || unsafe { gx::set_fog(gx::GX_FOG_PERSP_LIN, 1.0, 3.0, 1.0, 2.0, color) },
        ),
    );
    logger.record(
        "GX orthographic fog emits expected coefficients and color",
        check_gx_bp_words(
            &[
                0xee03_f000,
                0xef00_0000,
                0xf000_0000,
                0xf153_f000,
                0xf212_3456,
            ],
            || unsafe { gx::set_fog(gx::GX_FOG_ORTHO_LIN, 2.0, 4.0, 1.0, 2.0, color) },
        ),
    );
    logger.record(
        "GX degenerate perspective fog emits zero coefficients",
        check_gx_bp_words(
            &[
                0xee00_0000,
                0xef40_000f,
                0xf000_0001,
                0xf140_0000,
                0xf212_3456,
            ],
            || unsafe { gx::set_fog(gx::GX_FOG_PERSP_LIN, 1.0, 1.0, 1.0, 2.0, color) },
        ),
    );
    let table = gx::GxFogAdjTbl {
        r: [
            0x123, 0xabc, 0xfff, 0x1000, 0x2345, 0x5678, 0, 1, 0xfffe, 0xffff,
        ],
    };
    logger.record(
        "GX fog range adjustment packs table and wraps center",
        check_gx_bp_words(
            &[
                0xe9ab_c123,
                0xea00_0fff,
                0xeb67_8345,
                0xec00_1000,
                0xedff_fffe,
                0xe800_04da,
            ],
            || unsafe { gx::set_fog_range_adj(1, 900, &table) },
        ),
    );
    logger.record(
        "GX disabled fog range adjustment does not read a table",
        check_gx_bp_words(&[0xe800_0156], || unsafe {
            gx::set_fog_range_adj(0, 0, core::ptr::null());
        }),
    );
}

fn probe_fog_color_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX fog color command bytes");
    for (color, word) in [
        (
            gx::GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0,
            },
            0xf212_3456,
        ),
        (
            gx::GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0xff,
            },
            0xf212_3456,
        ),
        (
            gx::GxColor {
                r: 0xff,
                g: 0,
                b: 0x80,
                a: 1,
            },
            0xf2ff_0080,
        ),
    ] {
        logger.record(
            "GX fog color encodes RGB and ignores alpha",
            check_gx_bp_words(&[word], || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                gx::set_fog_color(color);
            }),
        );
    }
}

fn probe_clip_mode_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX clip mode command bytes");
    for mode in [0, 1] {
        // One XF register at address 0x1005; the final word is the mode.
        let expected = [0x10, 0, 0, 0x10, 5, 0, 0, 0, mode];
        logger.record(
            "GX clip mode emits exact XF enable and disable values",
            check_gx_command_bytes(&expected, || unsafe {
                // SAFETY: This probe owns GX and captures the commands.
                ogc_gx::set_clip_mode(mode);
            }),
        );
    }
}

fn probe_copy_clear_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX copy clear color and depth command bytes");
    for (rgba, depth, expected) in [
        (
            [0x12, 0x34, 0x56, 0x78],
            0xab12_3456,
            [0x4f00_7812, 0x5000_3456, 0x5112_3456],
        ),
        (
            [0xff, 0, 0x80, 1],
            0xffff_ffff,
            [0x4f00_01ff, 0x5000_0080, 0x51ff_ffff],
        ),
    ] {
        logger.record(
            "GX copy clear packs AR GB and masked 24-bit depth",
            check_gx_bp_words(&expected, || unsafe {
                // SAFETY: The probe owns initialized GX and captures commands.
                gx::set_copy_clear(
                    gx::GxColor {
                        r: rgba[0],
                        g: rgba[1],
                        b: rgba[2],
                        a: rgba[3],
                    },
                    depth,
                );
            }),
        );
    }
}

fn probe_copy_filter_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX copy filter sample and coefficient bytes");
    // Upper bits must be masked to four-bit sample and six-bit coefficient fields.
    let samples = [
        [0x10, 0x21],
        [0x32, 0x43],
        [0x54, 0x65],
        [0x76, 0x87],
        [0x98, 0xa9],
        [0xba, 0xcb],
        [0xdc, 0xed],
        [0xfe, 0x0f],
        [0x10, 0x21],
        [0x32, 0x43],
        [0x54, 0x65],
        [0x76, 0x87],
    ];
    let coefficients = [0x41, 0x82, 0xc3, 4, 0x45, 0x86, 0xc7];
    for (aa, vf, expected) in [
        (
            0,
            0,
            [
                0x0166_6666,
                0x0266_6666,
                0x0366_6666,
                0x0466_6666,
                0x5359_5000,
                0x5400_0015,
            ],
        ),
        (
            1,
            0,
            [
                0x0154_3210,
                0x02ba_9876,
                0x0310_fedc,
                0x0476_5432,
                0x5359_5000,
                0x5400_0015,
            ],
        ),
        (
            0,
            1,
            [
                0x0166_6666,
                0x0266_6666,
                0x0366_6666,
                0x0466_6666,
                0x5310_3081,
                0x5400_7185,
            ],
        ),
        (
            1,
            1,
            [
                0x0154_3210,
                0x02ba_9876,
                0x0310_fedc,
                0x0476_5432,
                0x5310_3081,
                0x5400_7185,
            ],
        ),
    ] {
        logger.note_u32("GX filter AA enabled", aa);
        logger.note_u32("GX vertical filter enabled", vf);
        logger.record(
            "GX copy filter emits six exact default or masked table words",
            check_gx_bp_words(&expected, || unsafe {
                // SAFETY: Enabled tables are complete and live. Disabled tables
                // are null to check that their pointers are not consumed.
                gx::set_copy_filter(
                    aa as u8,
                    if aa == 0 {
                        core::ptr::null()
                    } else {
                        samples.as_ptr()
                    },
                    vf as u8,
                    if vf == 0 {
                        core::ptr::null()
                    } else {
                        coefficients.as_ptr()
                    },
                );
            }),
        );
    }
    for (aa, vf) in [(1, 0), (0, 1)] {
        logger.record(
            "GX copy filter rejects enabled null table without commands",
            check_gx_command_bytes(&[], || unsafe {
                // SAFETY: Rust explicitly rejects a null enabled table before
                // reading either pointer or emitting any BP command.
                gx::set_copy_filter(aa, core::ptr::null(), vf, core::ptr::null());
            }),
        );
    }
}

fn probe_field_mask_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX even and odd field mask command bytes");
    for ((even, odd), expected) in [
        ((0, 0), 0x4400_0000),
        ((1, 0), 0x4400_0002),
        ((0, 1), 0x4400_0001),
        ((0xff, 0xff), 0x4400_0003),
    ] {
        logger.record(
            "GX field mask encodes independent even odd and masked bits",
            check_gx_bp_words(&[expected], || unsafe {
                // SAFETY: The probe owns initialized GX and captures commands.
                ogc_gx::set_field_mask(even, odd);
            }),
        );
    }
}

fn probe_bounding_box_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX bounding box reset command bytes");
    for _ in 0..2 {
        logger.record(
            "GX bounding box reset emits both fixed reset words each time",
            check_gx_bp_words(&[0x5500_03ff, 0x5600_03ff], || unsafe {
                // SAFETY: The probe owns initialized GX and captures commands.
                ogc_gx::clear_bounding_box();
            }),
        );
    }
}

fn probe_z_texture_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX Z texture bias format and operation command bytes");
    for ((op, format, bias), expected) in [
        (
            (gx::GX_ZT_DISABLE, gx::GX_TF_Z8, 0),
            [0xf400_0000, 0xf500_0000],
        ),
        (
            (gx::GX_ZT_ADD, gx::GX_TF_Z16, 0xab12_3456),
            [0xf412_3456, 0xf500_0005],
        ),
        (
            (gx::GX_ZT_REPLACE, gx::GX_TF_Z24X8, 0xffff_ffff),
            [0xf4ff_ffff, 0xf500_000a],
        ),
    ] {
        logger.record(
            "GX Z texture emits exact masked bias format and operation",
            check_gx_bp_words(&expected, || unsafe {
                // SAFETY: The probe owns initialized GX and captures commands.
                gx::set_z_texture(op, format, bias);
            }),
        );
    }
}

fn probe_z_mode_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX depth comparison mode command bytes");
    for ((enable, function, update), expected) in [
        ((0, 0, 0), 0x4000_0000),
        ((1, 7, 1), 0x4000_001f),
        ((1, 3, 1), 0x4000_0017),
    ] {
        logger.record(
            "GX depth mode encodes enable function and update",
            check_gx_bp_words(&[expected], || unsafe {
                // SAFETY: The probe owns initialized GX and captures commands.
                ogc_gx::set_z_mode(enable, function, update);
            }),
        );
    }
}

fn probe_vertex_cache_commands(logger: &mut ProbeLogger) {
    logger.report_step("GX vertex cache invalidation command");
    for _ in 0..2 {
        logger.record(
            "GX vertex cache invalidation emits opcode each time",
            check_gx_command_bytes(&[0x48], || ogc_gx::inv_vtx_cache()),
        );
    }
}

fn probe_texture_cache_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX texture cache invalidation commands");
    #[repr(align(32))]
    struct CacheTrace([u8; 64]);
    let mut trace = CacheTrace([0xa5; 64]);
    unsafe {
        gx::begin_disp_list(trace.0.as_mut_ptr().cast(), 64);
        gx::invalidate_tex_all();
    }
    let trace_size = gx::end_disp_list();
    logger.note_u32("GX cache trace size", trace_size);
    if trace_size >= 20 {
        for (index, command) in trace.0[..20].chunks_exact(5).enumerate() {
            logger.note_hex32_index(
                "GX cache trace word",
                index as i32,
                u32::from_be_bytes([command[1], command[2], command[3], command[4]]),
            );
            logger.note_u32("GX cache trace opcode", u32::from(command[0]));
        }
    }
    // Initialization sets the indirect mask to zero: BP word 0x0f000000.
    // The C reference flushes that mask before and after the two cache words.
    let expected = [0x0f00_0000, 0x6600_1000, 0x6600_1100, 0x0f00_0000];
    for _ in 0..2 {
        logger.record(
            "GX invalidate_tex_all emits both cache words and flushes",
            check_gx_bp_words(&expected, || unsafe { gx::invalidate_tex_all() }),
        );
        logger.record(
            "GX invalidate_texture_cache emits both cache words and flushes",
            check_gx_bp_words(&expected, || unsafe { gx::invalidate_texture_cache() }),
        );
    }
    // SAFETY: The initialized FIFO is exclusively owned by this probe.
    unsafe { gx::invalidate_tex_all() };
    gx::inv_vtx_cache();
    gx::set_draw_sync(0x7531);
    let completed = unsafe { gx::finish_frame_and_wait() };
    logger.record(
        "GX executes cache commands and subsequent token",
        completed && gx::get_draw_sync() == 0x7531,
    );
}

fn probe_line_point_field_commands(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX line point and field shared state");
    // SAFETY: The probe owns GX. Establish a known baseline and wait for it.
    let ready = unsafe {
        gx::set_line_width(6, 0);
        gx::set_point_size(6, 0);
        gx::set_field_mode(0, 0);
        gx::finish_frame_and_wait()
    };
    logger.record("GX line point field baseline completes", ready);
    if !ready {
        return;
    }
    // Keep the shadow changes between captures for this shared-register test.
    gx::set_misc(u32::from(gx::GX_MT_DL_SAVE_CTX), 0);
    logger.record(
        "GX line width preserves point fields",
        check_gx_bp_words(&[0x2203_060c], || unsafe { gx::set_line_width(12, 3) }),
    );
    logger.record(
        "GX point size preserves line fields",
        check_gx_bp_words(&[0x222b_220c], || unsafe { gx::set_point_size(34, 5) }),
    );
    // GX initialization sets the indirect texture mask to zero. The C
    // reference flushes that current mask on both sides of register 0x68.
    logger.record(
        "GX field mode preserves dimensions and flushes current mask",
        check_gx_bp_words(
            &[0x226b_220c, 0x0f00_0000, 0x6800_0001, 0x0f00_0000],
            || unsafe {
                gx::set_field_mode(1, 1);
            },
        ),
    );
    logger.record(
        "GX field mode clears its bits and retains line point state",
        check_gx_bp_words(
            &[0x222b_220c, 0x0f00_0000, 0x6800_0000, 0x0f00_0000],
            || unsafe {
                gx::set_field_mode(0, 0);
            },
        ),
    );
    // Establish a distinct current indirect mask through the production GX
    // state path. This case detects an implementation that always writes zero.
    // SAFETY: The probe owns GX and flushes the changed state before capture.
    let mask_ready = unsafe {
        gx::set_ind_tex_order(gx::GX_INDTEXSTAGE0, 0, 3);
        gx::set_num_ind_stages(1);
        gx::flush();
        gx::finish_frame_and_wait()
    };
    logger.record("GX nonzero indirect mask setup completes", mask_ready);
    if mask_ready {
        logger.record(
            "GX field mode preserves a nonzero indirect mask",
            check_gx_bp_words(
                &[0x226b_220c, 0x0f00_0008, 0x6800_0001, 0x0f00_0008],
                || unsafe {
                    gx::set_field_mode(1, 1);
                },
            ),
        );
    }
    // SAFETY: Restore the original no-indirect-stage setup before tracing.
    let mask_restored = unsafe {
        gx::set_ind_tex_order(gx::GX_INDTEXSTAGE0, 0, 0);
        gx::set_num_ind_stages(0);
        gx::flush();
        gx::finish_frame_and_wait()
    };
    logger.record("GX indirect mask restoration completes", mask_restored);
    #[repr(align(32))]
    struct FieldTrace([u8; 64]);
    let mut trace = FieldTrace([0xa5; 64]);
    // SAFETY: GX is owned by the probe and the capture buffer remains live.
    unsafe {
        gx::begin_disp_list(trace.0.as_mut_ptr().cast(), 64);
        gx::set_field_mode(1, 1);
    }
    let size = gx::end_disp_list();
    logger.note_u32("GX field trace size", size);
    if size >= 20 {
        for (index, command) in trace.0[..20].chunks_exact(5).enumerate() {
            logger.note_hex32_index(
                "GX field trace word",
                index as i32,
                u32::from_be_bytes([command[1], command[2], command[3], command[4]]),
            );
            logger.note_u32("GX field trace opcode", u32::from(command[0]));
        }
    }
    gx::set_misc(u32::from(gx::GX_MT_DL_SAVE_CTX), 1);
    // SAFETY: Restore the baseline GX settings before leaving this probe.
    let restored = unsafe {
        gx::set_line_width(6, 0);
        gx::set_point_size(6, 0);
        gx::set_field_mode(0, 0);
        gx::finish_frame_and_wait()
    };
    logger.record("GX line point field restoration completes", restored);
}

fn probe_solid_rect_render(logger: &mut ProbeLogger, width: u16, height: u16) {
    use ogc_gx as gx;

    logger.report_step("GX solid rectangle rendered pixel checks");
    let w = f32::from(width);
    let h = f32::from(height);
    let projection = [
        [2.0 / w, 0.0, 0.0, -1.0],
        [0.0, -2.0 / h, 0.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // SAFETY: GX and the FIFO are initialized and owned by this probe. The
    // display mode supplies the active EFB dimensions and the matrix is live.
    let ready = unsafe {
        gx::set_pixel_fmt(gx::GX_PF_RGB8_Z24, gx::GX_ZC_LINEAR);
        gx::configure_solid_2d(&projection);
        gx::set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
        gx::set_scissor(0, 0, u32::from(width), u32::from(height));
        gx::set_z_mode(0, gx::GX_ALWAYS, 0);
        gx::set_blend_mode(
            gx::GX_BM_NONE,
            gx::GX_BL_ONE,
            gx::GX_BL_ZERO,
            gx::GX_LO_CLEAR,
        );
        gx::set_color_update(1);
        gx::finish_frame_and_wait()
    };
    logger.record("GX solid rectangle render setup completes", ready);
    if !ready {
        return;
    }

    let black = gx::GxColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0xff,
    };
    let red = gx::GxColor {
        r: 0xff,
        g: 0,
        b: 0,
        a: 0xff,
    };
    let green = gx::GxColor {
        r: 0,
        g: 0xff,
        b: 0,
        a: 0xff,
    };
    let blue = gx::GxColor {
        r: 0,
        g: 0,
        b: 0xff,
        a: 0xff,
    };
    let points = [(40, 40), (60, 40), (110, 20)];
    gx::poke_alpha_read(gx::GX_READ_FF);
    for (x, y) in points {
        gx::poke_argb(x, y, black);
    }
    let read = |x, y| {
        let mut color = gx::GxColor::default();
        // SAFETY: The output is writable and the EFB coordinates are valid.
        unsafe { gx::peek_argb(x, y, &mut color) };
        color
    };
    let colors_match = |a, b, c| read(40, 40) == a && read(60, 40) == b && read(110, 20) == c;
    // Prove that the deliberately changed combiner produces black before
    // asking configure_solid_2d to restore the solid-color pipeline.
    gx::poke_argb(40, 40, green);
    // SAFETY: The probe owns the initialized GX state and waits before reads.
    let scrambled = unsafe {
        gx::set_tev_color_in(
            0,
            gx::GX_CC_ZERO,
            gx::GX_CC_ZERO,
            gx::GX_CC_ZERO,
            gx::GX_CC_ZERO,
        );
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, red).is_ok() && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX altered combiner renders black before reconfiguration",
        scrambled && colors_match(black, black, black),
    );
    if !scrambled {
        return;
    }
    // SAFETY: Reconfigure the owned pipeline, then select the no-blend case.
    let reconfigured = unsafe {
        gx::configure_solid_2d(&projection);
        gx::set_blend_mode(
            gx::GX_BM_NONE,
            gx::GX_BL_ONE,
            gx::GX_BL_ZERO,
            gx::GX_LO_CLEAR,
        );
        gx::finish_frame_and_wait()
    };
    logger.record("GX solid pipeline reconfiguration completes", reconfigured);
    if !reconfigured {
        return;
    }
    // SAFETY: The configured GX state remains owned by this probe. Each
    // finish waits until the GPU completes before pixel reads and reuse.
    let first = unsafe {
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, red).is_ok() && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX solid rectangle writes inside pixels and preserves outside",
        first && colors_match(red, red, black),
    );
    if !first {
        return;
    }

    let blocked = unsafe {
        gx::set_color_update(0);
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, blue).is_ok()
            && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX disabled color update preserves rendered pixels",
        blocked && colors_match(red, red, black),
    );
    if !blocked {
        return;
    }

    let unblended = unsafe {
        gx::set_color_update(1);
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, gx::GxColor { a: 0, ..green }).is_ok()
            && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX disabled blending writes transparent source color",
        unblended && colors_match(green, green, black),
    );
    if !unblended {
        return;
    }

    let transparent = unsafe {
        gx::set_blend_mode(
            gx::GX_BM_BLEND,
            gx::GX_BL_SRCALPHA,
            gx::GX_BL_INVSRCALPHA,
            gx::GX_LO_CLEAR,
        );
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, gx::GxColor { a: 0, ..red }).is_ok()
            && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX zero-alpha blend preserves destination pixels",
        transparent && colors_match(green, green, black),
    );
    if !transparent {
        return;
    }

    let reset_red = unsafe {
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, red).is_ok() && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX opaque blend replaces destination pixels",
        reset_red && colors_match(red, red, black),
    );
    if !reset_red {
        return;
    }

    let clipped = unsafe {
        gx::set_scissor(32, 32, 16, 24);
        gx::draw_solid_rect(32.0, 32.0, 48.0, 24.0, 0.0, green).is_ok()
            && gx::finish_frame_and_wait()
    };
    logger.record(
        "GX scissor clips opaque blended rectangle",
        clipped && colors_match(green, red, black),
    );

    // SAFETY: Restore full-screen scissor and the initialized depth setting.
    let restored = unsafe {
        gx::set_scissor(0, 0, u32::from(width), u32::from(height));
        gx::set_z_mode(1, gx::GX_LEQUAL, 1);
        gx::finish_frame_and_wait()
    };
    logger.record("GX rectangle test state restoration completes", restored);
}

#[repr(align(32))]
struct ProbeRgbaTile([u8; 64]);

fn probe_texture_copy_render(logger: &mut ProbeLogger, width: u16, height: u16) {
    use ogc_gx as gx;

    logger.report_step("GX EFB texture copy rendered pixel checks");
    let mut image = ProbeRgbaTile([0; 64]);
    let colors = [
        gx::GxColor {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        },
        gx::GxColor {
            r: 0,
            g: 255,
            b: 0,
            a: 255,
        },
        gx::GxColor {
            r: 0,
            g: 0,
            b: 255,
            a: 255,
        },
        gx::GxColor {
            r: 255,
            g: 255,
            b: 0,
            a: 255,
        },
    ];
    for y in 0..4u16 {
        for x in 0..4u16 {
            gx::poke_argb(100 + x, 100 + y, colors[usize::from((y / 2) * 2 + x / 2)]);
        }
    }
    // SAFETY: The aligned image has one 4x4 RGBA8 tile and remains live until
    // GX completes the copy and the subsequent texture draw.
    let copied = unsafe {
        ogc_cache::dc_flush_range(image.0.as_ptr().cast(), 64);
        gx::set_tex_copy_src(100, 100, 4, 4);
        gx::set_tex_copy_dst(4, 4, u32::from(gx::GX_TF_RGBA8), 0);
        gx::copy_tex(image.0.as_mut_ptr().cast(), 0);
        gx::finish_frame_and_wait()
    };
    logger.record("GX EFB texture copy completes", copied);
    if !copied {
        return;
    }
    // SAFETY: The GPU copy completed and the entire aligned cache line range
    // belongs to this image. Invalidate before reading the written bytes.
    unsafe { ogc_cache::dc_invalidate_range(image.0.as_ptr().cast(), 64) };
    let mut expected = ProbeRgbaTile([0; 64]);
    fill_probe_rgba_tile(
        &mut expected,
        [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]],
    );
    logger.record(
        "GX EFB copy writes expected tiled RGBA8 bytes",
        image.0 == expected.0,
    );

    let mut texture = gx::GxTexObj::default();
    let w = f32::from(width);
    let h = f32::from(height);
    let projection = [
        [2.0 / w, 0.0, 0.0, -1.0],
        [0.0, -2.0 / h, 0.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    gx::poke_argb(
        110,
        20,
        gx::GxColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
    );
    // SAFETY: The copied image remains live and GX is owned by the probe.
    let drawn = unsafe {
        gx::init_tex_obj(
            &mut texture,
            image.0.as_mut_ptr().cast(),
            gx::GxTexObjInit {
                wd: 4,
                ht: 4,
                fmt: gx::GX_TF_RGBA8,
                wrap_s: 0,
                wrap_t: 0,
                mipmap: 0,
            },
        );
        gx::init_tex_obj_lod(
            &mut texture,
            gx::GxTexObjLod {
                minfilt: gx::GX_NEAR,
                magfilt: gx::GX_NEAR,
                ..Default::default()
            },
        );
        gx::configure_textured_2d(&projection);
        gx::set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
        gx::set_scissor(0, 0, u32::from(width), u32::from(height));
        gx::set_z_mode(0, gx::GX_ALWAYS, 0);
        gx::invalidate_tex_all();
        gx::draw_textured_rect(&mut texture, 32.0, 32.0, 32.0, 32.0, 0.0).is_ok()
            && gx::finish_frame_and_wait()
    };
    let pixels = drawn
        && check_textured_samples(
            logger,
            [0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffff00, 0xff000000],
        );
    logger.record("GX copied texture renders expected quadrants", pixels);
    let restored = unsafe {
        gx::set_z_mode(1, gx::GX_LEQUAL, 1);
        gx::finish_frame_and_wait()
    };
    logger.record("GX texture copy state restoration completes", restored);
}

fn fill_probe_rgba_tile(tile: &mut ProbeRgbaTile, quadrants: [[u8; 3]; 4]) {
    // GX RGBA8 4x4 tiles store 16 AR pairs, then 16 GB pairs.
    for y in 0..4 {
        for x in 0..4 {
            let pixel = y * 4 + x;
            let [r, g, b] = quadrants[(y / 2) * 2 + x / 2];
            tile.0[pixel * 2] = 0xff;
            tile.0[pixel * 2 + 1] = r;
            tile.0[32 + pixel * 2] = g;
            tile.0[32 + pixel * 2 + 1] = b;
        }
    }
}

fn check_textured_samples(logger: &mut ProbeLogger, expected: [u32; 5]) -> bool {
    let points = [(40, 40), (56, 40), (40, 56), (56, 56), (110, 20)];
    let mut matches = true;
    for (index, ((x, y), expected)) in points.into_iter().zip(expected).enumerate() {
        let mut color = ogc_gx::GxColor::default();
        // SAFETY: Each EFB coordinate is valid and color is writable.
        unsafe { ogc_gx::peek_argb(x, y, &mut color) };
        let actual = u32::from_be_bytes([color.a, color.r, color.g, color.b]);
        logger.note_hex32_index("GX textured sample ARGB", index as i32, actual);
        matches &= actual == expected;
    }
    matches
}

fn probe_textured_rect_render(logger: &mut ProbeLogger, width: u16, height: u16) {
    use ogc_gx as gx;

    logger.report_step("GX textured rectangle rendered pixel checks");
    let w = f32::from(width);
    let h = f32::from(height);
    let projection = [
        [2.0 / w, 0.0, 0.0, -1.0],
        [0.0, -2.0 / h, 0.0, 1.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut first = ProbeRgbaTile([0; 64]);
    let mut second = ProbeRgbaTile([0; 64]);
    fill_probe_rgba_tile(
        &mut first,
        [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]],
    );
    fill_probe_rgba_tile(
        &mut second,
        [
            [255, 0, 255],
            [0, 255, 255],
            [255, 255, 255],
            [64, 128, 192],
        ],
    );
    let mut texture = gx::GxTexObj::default();
    // SAFETY: The probe owns GX and both aligned images stay live through
    // each completion. Flush their cached bytes before GPU access.
    let ready = unsafe {
        ogc_cache::dc_flush_range(first.0.as_ptr().cast(), 64);
        ogc_cache::dc_flush_range(second.0.as_ptr().cast(), 64);
        gx::init_tex_obj(
            &mut texture,
            first.0.as_mut_ptr().cast(),
            gx::GxTexObjInit {
                wd: 4,
                ht: 4,
                fmt: gx::GX_TF_RGBA8,
                wrap_s: 0,
                wrap_t: 0,
                mipmap: 0,
            },
        );
        gx::init_tex_obj_lod(
            &mut texture,
            gx::GxTexObjLod {
                minfilt: gx::GX_NEAR,
                magfilt: gx::GX_NEAR,
                ..Default::default()
            },
        );
        gx::configure_textured_2d(&projection);
        gx::set_viewport(0.0, 0.0, w, h, 0.0, 1.0);
        gx::set_scissor(0, 0, u32::from(width), u32::from(height));
        gx::set_z_mode(0, gx::GX_ALWAYS, 0);
        gx::set_blend_mode(
            gx::GX_BM_NONE,
            gx::GX_BL_ONE,
            gx::GX_BL_ZERO,
            gx::GX_LO_CLEAR,
        );
        gx::set_color_update(1);
        gx::invalidate_tex_all();
        gx::finish_frame_and_wait()
    };
    logger.record("GX textured rectangle setup completes", ready);
    if !ready {
        return;
    }
    let black = gx::GxColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0xff,
    };
    gx::poke_alpha_read(gx::GX_READ_FF);
    for (x, y) in [(40, 40), (56, 40), (40, 56), (56, 56), (110, 20)] {
        gx::poke_argb(x, y, black);
    }

    // SAFETY: Null and invalid map inputs are rejected before use; the valid
    // texture object remains writable and its aligned image remains live.
    let invalid = unsafe {
        matches!(
            gx::load_tex_obj(core::ptr::null_mut(), 0),
            Err(gx::GxTextureError::InvalidObject)
        ) && matches!(
            gx::load_tex_obj(&mut texture, 8),
            Err(gx::GxTextureError::UnsupportedObject)
        ) && matches!(
            gx::draw_textured_rect(&mut texture, 32.0, 32.0, 0.0, 32.0, 0.0),
            Err(gx::GxTextureError::InvalidGeometry)
        ) && matches!(
            gx::draw_textured_rect(core::ptr::null_mut(), 32.0, 32.0, 32.0, 32.0, 0.0),
            Err(gx::GxTextureError::InvalidObject)
        )
    };
    logger.record(
        "GX textured drawing rejects invalid object map and geometry",
        invalid,
    );

    // SAFETY: The object and flushed first image remain live through completion.
    let loaded = unsafe { gx::load_tex_obj(&mut texture, 0) }.is_ok();
    logger.record("GX texture object loads into map zero", loaded);
    if !loaded {
        return;
    }
    let drawn = unsafe {
        gx::draw_textured_rect(&mut texture, 32.0, 32.0, 32.0, 32.0, 0.0).is_ok()
            && gx::finish_frame_and_wait()
    };
    let first_expected = [
        0xffff_0000,
        0xff00_ff00,
        0xff00_00ff,
        0xffff_ff00,
        0xff00_0000,
    ];
    let first_pixels = drawn && check_textured_samples(logger, first_expected);
    logger.record(
        "GX textured rectangle draws first tile quadrants",
        first_pixels,
    );
    if !drawn {
        return;
    }

    // SAFETY: The first draw completed; the second aligned and flushed image
    // stays live through the next load, draw, and GPU completion.
    let replaced = unsafe {
        gx::init_tex_obj_data(&mut texture, second.0.as_mut_ptr().cast());
        gx::invalidate_tex_all();
        gx::load_tex_obj(&mut texture, 0).is_ok()
            && gx::draw_textured_rect(&mut texture, 32.0, 32.0, 32.0, 32.0, 0.0).is_ok()
            && gx::finish_frame_and_wait()
    };
    let second_expected = [
        0xffff_00ff,
        0xff00_ffff,
        0xffff_ffff,
        0xff40_80c0,
        0xff00_0000,
    ];
    let second_pixels = replaced && check_textured_samples(logger, second_expected);
    logger.record(
        "GX texture replacement draws new tile quadrants",
        second_pixels,
    );

    // SAFETY: Restore full-mode scissor and the initialized depth setting.
    let restored = unsafe {
        gx::set_scissor(0, 0, u32::from(width), u32::from(height));
        gx::set_z_mode(1, gx::GX_LEQUAL, 1);
        gx::finish_frame_and_wait()
    };
    logger.record(
        "GX textured rectangle state restoration completes",
        restored,
    );
}

fn probe_display_lists(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    #[repr(align(32))]
    struct ListBytes([u8; 96]);

    logger.report_step("GX display-list capture and GPU replay");
    gx::set_draw_sync(0x1357);
    // SAFETY: The probe owns the initialized GX FIFO.
    let ready = unsafe { gx::finish_frame_and_wait() };
    logger.record(
        "GX direct draw token reaches pixel engine",
        ready && gx::get_draw_sync() == 0x1357,
    );
    if !ready {
        return;
    }

    let mut previous_token = 0x1357;
    for token in [0x2468u16, 0xace1] {
        let mut bytes = ListBytes([0xa5; 96]);
        // SAFETY: The aligned buffer has 64 writable bytes and stays live
        // through capture, cache flush, replay, and GPU completion.
        unsafe { gx::begin_disp_list(bytes.0.as_mut_ptr().cast(), 64) };
        gx::tex_mode_sync();
        gx::set_draw_sync(token);
        let size = gx::end_disp_list();
        let [high, low] = token.to_be_bytes();
        // BP opcode 0x61, texture sync register 0x63, then token registers
        // 0x48 and 0x47. GX_Flush adds 32 zero bytes; padding ends at 64.
        let expected = [
            0x61, 0x63, 0, 0, 0, 0x61, 0x48, 0, high, low, 0x61, 0x47, 0, high, low,
        ];
        logger.record(
            "GX display list has exact BP bytes padding and guard",
            size == 64
                && bytes.0[..15] == expected
                && bytes.0[15..64] == [0; 49]
                && bytes.0[64..] == [0xa5; 32],
        );
        logger.record(
            "GX capture does not execute token commands",
            gx::get_draw_sync() == previous_token,
        );
        if size != 64 {
            return;
        }
        // SAFETY: The captured list is complete, aligned, flushed, and live.
        // Finish keeps the buffer live until the GPU has consumed the list.
        let completed = unsafe {
            ogc_cache::dc_flush_range(bytes.0.as_ptr().cast(), size);
            gx::call_disp_list(bytes.0.as_mut_ptr().cast(), size);
            gx::finish_frame_and_wait()
        };
        logger.record(
            "GX replay executes captured token on pixel engine",
            completed && gx::get_draw_sync() == token,
        );
        if !completed {
            return;
        }
        previous_token = token;
    }

    let mut bytes = ListBytes([0xa5; 96]);
    // SAFETY: Only the first 32 bytes are offered for this overflow check.
    unsafe { gx::begin_disp_list(bytes.0.as_mut_ptr().cast(), 32) };
    gx::set_draw_sync(0xdead);
    let overflow_size = gx::end_disp_list();
    logger.record(
        "GX display-list overflow preserves guard and GPU token",
        overflow_size == 0 && bytes.0[32..] == [0xa5; 64] && gx::get_draw_sync() == previous_token,
    );
    logger.record(
        "GX inactive display-list end returns zero",
        gx::end_disp_list() == 0,
    );
    // Check FIFO restoration after the overflow; an ordinary token command
    // must still reach the pixel engine rather than the capture buffer.
    gx::set_draw_sync(0x3579);
    let completed = unsafe { gx::finish_frame_and_wait() };
    logger.record(
        "GX FIFO executes commands after capture overflow",
        completed && gx::get_draw_sync() == 0x3579,
    );
}

fn probe_efb_access(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX EFB color and depth access");
    // SAFETY: The probe owns the initialized GX FIFO. Complete its format
    // command before CPU access to the EFB. No rendering runs during this test.
    let ready = unsafe {
        gx::set_pixel_fmt(gx::GX_PF_RGB8_Z24, gx::GX_ZC_LINEAR);
        gx::finish_frame_and_wait()
    };
    logger.record("GX EFB format command completes", ready);
    if !ready {
        return;
    }

    gx::poke_alpha_read(gx::GX_READ_FF);
    // Distinct X and Y coordinates detect address-field errors. RGB8 has no
    // stored alpha; GX_READ_FF must return 255 for each color read.
    let points = [(8, 8), (9, 8), (8, 9)];
    let colors = [
        gx::GxColor {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0xff,
        },
        gx::GxColor {
            r: 0xab,
            g: 0xcd,
            b: 0xef,
            a: 0xff,
        },
        gx::GxColor {
            r: 0x65,
            g: 0x43,
            b: 0x21,
            a: 0xff,
        },
    ];
    let depths = [0x12_3456, 0xab_cdef, 0x65_4321];
    for ((x, y), color) in points.into_iter().zip(colors) {
        gx::poke_argb(x, y, color);
    }
    for ((x, y), depth) in points.into_iter().zip(depths) {
        gx::poke_z(x, y, depth);
    }

    let read_pixel = |x, y| {
        let mut color = gx::GxColor::default();
        let mut depth = 0xffff_ffff;
        // SAFETY: Each output points to live writable storage.
        unsafe {
            gx::peek_argb(x, y, &mut color);
            gx::peek_z(x, y, &mut depth);
        }
        (color, depth)
    };
    let mut colors_match = true;
    let mut depths_match = true;
    for (index, (x, y)) in points.into_iter().enumerate() {
        let (color, depth) = read_pixel(x, y);
        colors_match &= color == colors[index];
        depths_match &= depth == depths[index];
        logger.note_hex32(
            "GX EFB observed ARGB",
            u32::from_be_bytes([color.a, color.r, color.g, color.b]),
        );
        logger.note_hex32("GX EFB observed depth", depth);
    }
    logger.record(
        "GX EFB RGB8 colors preserve X and Y coordinates",
        colors_match,
    );
    logger.record(
        "GX EFB Z24 depths preserve X and Y coordinates",
        depths_match,
    );

    let replacement = gx::GxColor {
        r: 0xfe,
        g: 0x80,
        b: 0x07,
        a: 0xff,
    };
    gx::poke_argb(8, 8, replacement);
    gx::poke_z(8, 8, 0x00_fedc);
    let mut replacement_matches = true;
    for (index, (x, y)) in points.into_iter().enumerate() {
        let (color, depth) = read_pixel(x, y);
        let expected_color = if index == 0 {
            replacement
        } else {
            colors[index]
        };
        let expected_depth = if index == 0 { 0x00_fedc } else { depths[index] };
        replacement_matches &= color == expected_color && depth == expected_depth;
    }
    logger.record(
        "GX EFB replacement preserves adjacent color and depth",
        replacement_matches,
    );

    gx::poke_alpha_read(gx::GX_READ_00);
    let mut color = gx::GxColor::default();
    // SAFETY: The output is writable and remains live for this call.
    unsafe { gx::peek_argb(8, 8, &mut color) };
    logger.record(
        "GX EFB alpha read mode changes only returned alpha",
        color
            == gx::GxColor {
                a: 0,
                ..replacement
            },
    );
    gx::poke_alpha_read(gx::GX_READ_FF);
}

fn pe_test_register(index: usize) -> *mut u16 {
    // The Wii pixel-engine MMIO block has 16-bit registers at 0xcc001000.
    (0xcc00_1000usize + index * 2) as *mut u16
}

fn pe_test_read(index: usize) -> u16 {
    // SAFETY: Callers use only the four pixel-engine configuration registers.
    unsafe { core::ptr::read_volatile(pe_test_register(index)) }
}

fn record_pe_register(logger: &mut ProbeLogger, label: &str, index: usize, expected: u16) {
    let actual = pe_test_read(index);
    logger.note_hex32("GX PE register actual", u32::from(actual));
    logger.note_hex32("GX PE register expected", u32::from(expected));
    logger.record(label, actual == expected);
}

fn probe_efb_pe_registers(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    logger.report_step("GX pixel engine configuration register readback");
    // Save the exact register state so the existing EFB and later checks keep
    // their original settings. The probe exclusively owns GX during this run.
    let saved = [
        pe_test_read(0),
        pe_test_read(1),
        pe_test_read(2),
        pe_test_read(3),
    ];

    gx::poke_alpha_mode(3, 0xa5);
    record_pe_register(
        logger,
        "GX alpha mode stores function and threshold",
        3,
        0x03a5,
    );
    gx::poke_alpha_mode(0xff, 0x12);
    record_pe_register(logger, "GX alpha mode replaces both bytes", 3, 0xff12);

    gx::poke_dst_alpha(2, 0x48);
    record_pe_register(
        logger,
        "GX destination alpha masks disabled enable bit",
        2,
        0x0048,
    );
    gx::poke_dst_alpha(0xff, 0xa5);
    record_pe_register(
        logger,
        "GX destination alpha stores enabled value",
        2,
        0x01a5,
    );

    for enabled in [0, 1] {
        let before = pe_test_read(1);
        gx::poke_color_update(enabled);
        let expected = (before & !0x0008) | if enabled == 0 { 0 } else { 0x0008 };
        record_pe_register(logger, "GX color update changes only its bit", 1, expected);
    }
    for enabled in [0, 1] {
        let before = pe_test_read(1);
        gx::poke_alpha_update(enabled);
        let expected = (before & !0x0010) | if enabled == 0 { 0 } else { 0x0010 };
        record_pe_register(logger, "GX alpha update changes only its bit", 1, expected);
    }
    for enabled in [0, 1] {
        let before = pe_test_read(1);
        gx::poke_dither(enabled);
        let expected = (before & !0x0004) | if enabled == 0 { 0 } else { 0x0004 };
        record_pe_register(logger, "GX dither changes only its bit", 1, expected);
    }

    // Clear the three preserved update bits and all blend fields. This makes
    // the following complete expected register words independent of startup.
    gx::poke_alpha_update(0);
    gx::poke_color_update(0);
    gx::poke_dither(0);
    gx::poke_blend_mode(gx::GX_BM_NONE, 0, 0, 0);
    record_pe_register(logger, "GX blend mode baseline is zero", 1, 0);
    gx::poke_blend_mode(gx::GX_BM_BLEND, 4, 5, 6);
    record_pe_register(
        logger,
        "GX blend mode encodes factors and logic field",
        1,
        0x64a1,
    );
    gx::poke_blend_mode(gx::GX_BM_LOGIC, 1, 7, 3);
    record_pe_register(
        logger,
        "GX logic mode replaces prior blend fields",
        1,
        0x31e2,
    );
    gx::poke_blend_mode(gx::GX_BM_SUBTRACT, 7, 0, 15);
    record_pe_register(
        logger,
        "GX subtract mode sets subtract and blend bits",
        1,
        0xff01,
    );

    // The C reference has a final assignment that leaves only update bit 4.
    gx::poke_z_mode(0, 0, 0);
    record_pe_register(logger, "GX poke Z mode clears update", 0, 0);
    gx::poke_z_mode(1, 7, 1);
    record_pe_register(
        logger,
        "GX poke Z mode follows C update-bit behavior",
        0,
        0x0010,
    );
    gx::poke_z_mode(1, 3, 0);
    record_pe_register(logger, "GX poke Z mode clears prior update bit", 0, 0);

    // SAFETY: These are the same writable configuration registers saved above;
    // no GPU work is submitted between the snapshot and restoration.
    for (index, value) in saved.into_iter().enumerate() {
        unsafe { core::ptr::write_volatile(pe_test_register(index), value) };
    }
    logger.record(
        "GX pixel engine configuration registers restore",
        (0..saved.len()).all(|index| pe_test_read(index) == saved[index]),
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
