use alloc::alloc::{Layout, alloc_zeroed, dealloc};
use core::ffi::c_void;
use core::ptr::NonNull;

use ogc_system::{self as system, SYS_FONTSIZE_SJIS, SysFontHeader};
use ogc_system_sys as raw_system;

use crate::probe_support::ProbeLogger;

const FONT_BYTES: usize = SYS_FONTSIZE_SJIS as usize;
const TEXEL_BYTES: usize = 32 * 1024;
const FONT_ALIGNMENT: usize = 32;
const TEXEL_STRIDE: i32 = 64;

struct AlignedBuffer {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl AlignedBuffer {
    fn new(size: usize, align: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size, align).ok()?;
        let ptr = unsafe { alloc_zeroed(layout) };
        Some(Self {
            ptr: NonNull::new(ptr)?,
            layout,
        })
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("FONT PROBE");

    probe_raw_font_api_guards(logger);

    let encoding = system::get_font_encoding();
    logger.note_u32("font encoding", encoding);
    logger.record("font encoding sane", encoding <= 1);

    let Some(mut font) = AlignedBuffer::new(FONT_BYTES, FONT_ALIGNMENT) else {
        logger.record("font buffer allocated", false);
        return;
    };
    logger.record("font buffer allocated", true);

    // SAFETY: `font` has the documented size/alignment and remains live through
    // every font query below.
    let init_rc_a = unsafe { system::init_font(font.as_mut_ptr().cast::<SysFontHeader>()) };
    logger.note_i32("font init rc a", init_rc_a as i32);
    // SAFETY: same live, aligned font allocation as above.
    let init_rc_b = unsafe { system::init_font(font.as_mut_ptr().cast::<SysFontHeader>()) };
    logger.note_i32("font init rc b", init_rc_b as i32);
    logger.record(
        "font init reports a stable loaded or unavailable state",
        init_rc_a == init_rc_b && init_rc_b <= 1,
    );

    let loaded = init_rc_b == 1;
    let extended = if encoding == 1 { 0x8140 } else { 0x00e9 };

    probe_texture(logger, "ascii", 'A' as i32, loaded, true);
    probe_texture(logger, "extended", extended, loaded, false);
    probe_texture(logger, "fallback", -1, loaded, false);
    probe_texel(logger, "ascii", 'A' as i32, loaded);
}

fn probe_raw_font_api_guards(logger: &mut ProbeLogger) {
    // SAFETY: A null font pointer is a documented rejected input.
    let init = unsafe { raw_system::sys_init_font(core::ptr::null_mut()) };
    let init_snapshot = raw_system::snapshot_font();
    logger.record(
        "Raw system font init rejects null storage",
        init == 0 && init_snapshot.init_called && init_snapshot.init_ptr == 0,
    );

    let mut image = core::ptr::NonNull::<u8>::dangling()
        .as_ptr()
        .cast::<c_void>();
    let mut xpos = 99;
    let mut ypos = 99;
    let mut texture_width = 99;
    // SAFETY: All output pointers point to live, uniquely borrowed local values.
    unsafe {
        raw_system::sys_get_font_texture(
            'A' as i32,
            &mut image,
            &mut xpos,
            &mut ypos,
            &mut texture_width,
        )
    };
    let texture_snapshot = raw_system::snapshot_font();
    logger.record(
        "Raw system font texture clears outputs without font data",
        image.is_null()
            && xpos == 0
            && ypos == 0
            && texture_width == 0
            && texture_snapshot.texture_called
            && texture_snapshot.texture_c == 'A' as u32,
    );

    let mut texel_width = 99;
    // SAFETY: The null image is rejected before the destination is read or written.
    unsafe {
        raw_system::sys_get_font_texel(
            'A' as i32,
            core::ptr::null_mut(),
            0,
            TEXEL_STRIDE,
            &mut texel_width,
        )
    };
    let texel_snapshot = raw_system::snapshot_font();
    logger.record(
        "Raw system font texel rejects null image",
        texel_width == 0 && texel_snapshot.texel_called && texel_snapshot.texel_c == 'A' as u32,
    );
}

fn probe_texture(
    logger: &mut ProbeLogger,
    label: &str,
    code: i32,
    loaded: bool,
    expect_width_positive: bool,
) {
    let mut image: *mut c_void = core::ptr::null_mut();
    let mut xpos = 0;
    let mut ypos = 0;
    let mut width = 0;
    // SAFETY: every output reference is valid and uniquely borrowed for the call.
    unsafe { system::get_font_texture(code, &mut image, &mut xpos, &mut ypos, &mut width) };
    logger.note_i32(&alloc::format!("font {} width", label), width);
    logger.note_i32(&alloc::format!("font {} xpos", label), xpos);
    logger.note_i32(&alloc::format!("font {} ypos", label), ypos);
    logger.record(
        &alloc::format!("font {} texture", label),
        if loaded {
            !image.is_null() && xpos >= 0 && ypos >= 0 && (!expect_width_positive || width > 0)
        } else {
            image.is_null() && width == 0 && xpos == 0 && ypos == 0
        },
    );
}

fn probe_texel(logger: &mut ProbeLogger, label: &str, code: i32, loaded: bool) {
    let Some(mut texel) = AlignedBuffer::new(TEXEL_BYTES, FONT_ALIGNMENT) else {
        logger.record(
            &alloc::format!("font {} texel buffer allocated", label),
            false,
        );
        return;
    };

    let mut width = 0;
    // SAFETY: `texel` is a live aligned output allocation and `width` is a valid
    // unique output reference.
    unsafe {
        system::get_font_texel(
            code,
            texel.as_mut_ptr().cast::<c_void>(),
            0,
            TEXEL_STRIDE,
            &mut width,
        )
    };
    logger.note_i32(&alloc::format!("font {} texel width", label), width);
    logger.record(
        &alloc::format!("font {} texel", label),
        if loaded {
            width > 0 && texel.as_slice().iter().any(|&byte| byte != 0)
        } else {
            width == 0 && texel.as_slice().iter().all(|&byte| byte == 0)
        },
    );
}
