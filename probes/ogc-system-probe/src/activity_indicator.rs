//! A real VI heartbeat and seconds since the probe last recorded progress.
use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};
use ogc_video as video;

static LAST_PROGRESS: AtomicU32 = AtomicU32::new(0);
static ENABLED: AtomicU32 = AtomicU32::new(0);
static mut FRAME: *mut u32 = core::ptr::null_mut();
static mut STRIDE: usize = 0;
static mut WIDTH: usize = 0;
static mut HEIGHT: usize = 0;
static mut PREVIOUS: video::ViRetraceCallback = None;

fn seconds() -> u32 {
    (ogc_system::time() / 60_750_000) as u32
}
pub(crate) fn progress() {
    LAST_PROGRESS.store(seconds(), Ordering::Release);
}

/// The caller owns this complete, uncached YUY2 framebuffer for the run.
pub(crate) unsafe fn start(frame: *mut c_void, bytes: usize, width: usize, height: usize) -> bool {
    if frame.is_null() || width < 160 || height < 64 || bytes % height != 0 {
        return false;
    }
    let stride = bytes / height;
    if stride < width * 2 || stride % 4 != 0 {
        return false;
    }
    let irq = ogc_irq::irq_disable();
    if ENABLED.load(Ordering::Acquire) != 0 {
        ogc_irq::irq_restore(irq);
        return true;
    }
    unsafe {
        FRAME = frame.cast();
        STRIDE = stride / 4;
        WIDTH = width;
        HEIGHT = height;
        progress();
        PREVIOUS = video::set_post_retrace_callback(Some(retrace));
    }
    ENABLED.store(1, Ordering::Release);
    ogc_irq::irq_restore(irq);
    true
}

/// Select another live XFB with the same dimensions and stride as `start`.
#[cfg(feature = "agent-report")]
pub(crate) unsafe fn set_framebuffer(frame: *mut c_void) {
    let irq = ogc_irq::irq_disable();
    unsafe { FRAME = frame.cast() };
    ogc_irq::irq_restore(irq);
}

const GLYPHS: [[u8; 5]; 20] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
    [7, 4, 7, 1, 7], // S
    [7, 2, 2, 2, 2], // T
    [7, 4, 6, 4, 7], // E
    [6, 5, 6, 4, 4], // P
    [0, 0, 0, 0, 0], // space
    [7, 4, 6, 4, 4], // F
    [2, 2, 2, 2, 7], // I
    [6, 5, 5, 5, 6], // D
    [4, 4, 4, 4, 7], // L
    [0, 6, 4, 4, 4], // s
];
unsafe fn glyph(x: usize, y: usize, index: usize) {
    for row in 0..10 {
        for column in 0..3 {
            let lit = GLYPHS[index][row / 2] & (1 << (2 - column)) != 0;
            let color = if lit { 0xeb80_eb80 } else { 0x1080_1080 };
            unsafe {
                FRAME
                    .add((y + row) * STRIDE + x / 2 + column)
                    .write_volatile(color);
            }
        }
    }
}

unsafe extern "C" fn retrace(count: u32) {
    unsafe {
        if let Some(previous) = PREVIOUS {
            previous(count);
        }
        if ENABLED.load(Ordering::Acquire) == 0 {
            return;
        }
        let x = (WIDTH - 144) & !1;
        let y = HEIGHT - 26;
        glyph(x, y - 14, 15);
        glyph(x + 8, y - 14, 14);
        let mut divisor = 1_000_000_000u32;
        for digit in 0..10 {
            glyph(x + 16 + digit * 8, y - 14, (count / divisor % 10) as usize);
            divisor = (divisor / 10).max(1);
        }
        let elapsed = seconds()
            .wrapping_sub(LAST_PROGRESS.load(Ordering::Acquire))
            .min(999);
        let text = [
            16,
            17,
            18,
            14,
            19,
            (elapsed / 100) as usize,
            (elapsed / 10 % 10) as usize,
            (elapsed % 10) as usize,
            10,
        ];
        for (index, code) in text.into_iter().enumerate() {
            glyph(x + index * 8, y + 3, code);
        }
        let phase = (count / 6 % 8) as usize;
        const DOTS: [(usize, usize); 8] = [
            (6, 0),
            (10, 2),
            (12, 6),
            (10, 10),
            (6, 12),
            (2, 10),
            (0, 6),
            (2, 2),
        ];
        for row in 0..16 {
            for column in 0..8 {
                FRAME
                    .add((y + row) * STRIDE + (x + 84) / 2 + column)
                    .write_volatile(0x1080_1080);
            }
        }
        for (index, (dx, dy)) in DOTS.into_iter().enumerate() {
            let color = if index == phase {
                0xeb80_eb80
            } else {
                0x5080_5080
            };
            for row in 0..2 {
                FRAME
                    .add((y + dy + row) * STRIDE + (x + 84 + dx) / 2)
                    .write_volatile(color);
            }
        }
    }
}
