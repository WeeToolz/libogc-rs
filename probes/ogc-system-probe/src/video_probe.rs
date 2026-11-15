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
