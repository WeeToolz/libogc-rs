//! Source XFB patterns. Words are Y0 U Y1 V, not EasyCap's U Y0 V Y1.
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const NAMES: [&str; 8] = [
    "GEOMETRY",
    "LUMA LEVELS",
    "75 PERCENT BARS",
    "LUMA DETAIL",
    "CHROMA EDGES",
    "ROW PARITY AND MOTION",
    "FLAT FIELDS",
    "FRAME AND FIELD ID",
];
pub const LEVELS: [u8; 16] = [
    0, 8, 16, 20, 24, 32, 48, 64, 96, 128, 160, 192, 219, 235, 240, 255,
];
// Explicit BT.601 limited-range 75% RGB bars: white yellow cyan green magenta red blue black.
pub const BARS: [(u8, u8, u8); 8] = [
    (180, 128, 128),
    (162, 44, 142),
    (131, 156, 44),
    (112, 72, 58),
    (84, 184, 198),
    (65, 100, 212),
    (35, 212, 114),
    (16, 128, 128),
];
pub const fn pack(y0: u8, u: u8, y1: u8, v: u8) -> u32 {
    u32::from_be_bytes([y0, u, y1, v])
}
pub const fn grey(y: u8) -> u32 {
    pack(y, 128, y, 128)
}

fn luma_detail(x: usize, y: usize) -> u8 {
    let band = ((y - 64) / 72).min(4);
    let width = [1, 2, 4, 8, 16][band];
    let high = if x < 320 {
        (x / width).is_multiple_of(2)
    } else {
        (y / width).is_multiple_of(2)
    };
    if high { 235 } else { 16 }
}

pub fn pair(page: usize, x: usize, y: usize, tick: u32) -> u32 {
    match page {
        0 => {
            if !(4..WIDTH - 4).contains(&x) || !(4..HEIGHT - 4).contains(&y) {
                return grey(235);
            }
            if [16, 32, 64, WIDTH - 66, WIDTH - 34, WIDTH - 18].contains(&x)
                || [16, 32, 64, HEIGHT - 65, HEIGHT - 33, HEIGHT - 17].contains(&y)
            {
                return grey(180);
            }
            if x == WIDTH / 2 || y == HEIGHT / 2 {
                return pack(180, 90, 180, 240);
            }
            if x.is_multiple_of(40) || y.is_multiple_of(40) {
                grey(96)
            } else {
                grey(16)
            }
        }
        1 => {
            if y < 260 {
                grey(LEVELS[(x / 40).min(15)])
            } else if y < 370 {
                grey((16 + x * 219 / (WIDTH - 1)) as u8)
            } else {
                grey((x * 255 / (WIDTH - 1)) as u8)
            }
        }
        2 => {
            let (yy, u, v) = BARS[x / 80];
            pack(yy, u, yy, v)
        }
        3 => {
            if (64..424).contains(&y) {
                pack(luma_detail(x, y), 128, luma_detail(x + 1, y), 128)
            } else {
                grey(16)
            }
        }
        4 => {
            let width = [2, 4, 8, 16][((y.saturating_sub(64)) / 90).min(3)];
            let high = (x / width).is_multiple_of(2);
            let c = if high { 192 } else { 64 };
            if x < 320 {
                pack(128, c, 128, 128)
            } else {
                pack(128, 128, 128, c)
            }
        }
        5 => {
            if (64..240).contains(&y) {
                grey(if y.is_multiple_of(2) { 235 } else { 16 })
            } else if (240..400).contains(&y) {
                let bar = (tick as usize * 4) % WIDTH;
                grey(if (x + WIDTH - bar) % WIDTH < 32 {
                    235
                } else {
                    32
                })
            } else {
                grey(16)
            }
        }
        6 => grey([16, 64, 128, 235][usize::from(x >= 320) + 2 * usize::from(y >= 240)]),
        crate::identification::PAGE => crate::identification::pair(x, y, tick, 0),
        _ => grey(16),
    }
}

fn text(frame: &mut [u32], x: usize, y: usize, bytes: &[u8]) {
    for (i, ch) in bytes.iter().enumerate() {
        for (row, bits) in ogc_ui::glyph_rows(*ch as char).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..2 {
                    for dx in 0..2 {
                        let xx = x + i * 12 + col * 2 + dx;
                        let yy = y + row * 2 + dy;
                        if xx >= WIDTH || yy >= HEIGHT {
                            continue;
                        }
                        let p = &mut frame[yy * (WIDTH / 2) + xx / 2];
                        *p = if xx.is_multiple_of(2) {
                            (*p & 0x00ff_ffff) | 0xeb00_0000
                        } else {
                            (*p & 0xffff_00ff) | 0x0000_eb00
                        };
                    }
                }
            }
        }
    }
}

pub fn render(frame: &mut [u32], page: usize, tick: u32, retrace: u32, auto: bool, hold: bool) {
    assert_eq!(frame.len(), WIDTH * HEIGHT / 2);
    assert!(page < NAMES.len());
    for y in 0..HEIGHT {
        for x in (0..WIDTH).step_by(2) {
            frame[y * (WIDTH / 2) + x / 2] = if page == crate::identification::PAGE {
                crate::identification::pair(x, y, tick, retrace)
            } else {
                pair(page, x, y, tick)
            };
        }
    }
    // Labels stay outside the central measurement region.
    for y in 34..60 {
        frame[y * 320 + 40..y * 320 + 280].fill(grey(16));
    }
    for y in 424..448 {
        frame[y * 320 + 32..y * 320 + 288].fill(grey(16));
    }
    text(frame, 80, 40, NAMES[page].as_bytes());
    text(frame, 64, 428, b"A NEXT B BACK START AUTO X HOLD");
    let state: &[u8] = if hold {
        b"HOLD".as_slice()
    } else if auto {
        b"AUTO"
    } else {
        b"MANUAL"
    };
    for y in 450..468 {
        frame[y * 320 + 40..y * 320 + 280].fill(grey(16));
    }
    text(frame, 80, 452, state);
    let mut count = [b'0'; 10];
    let mut n = tick;
    for digit in count.iter_mut().rev() {
        *digit = b'0' + (n % 10) as u8;
        n /= 10;
    }
    text(frame, 264, 452, &count);
    if page == crate::identification::PAGE {
        text(frame, 64, 72, b"EVEN SOURCE ROWS");
        text(frame, 64, 232, b"ODD SOURCE ROWS");
        text(frame, 64, 392, b"ID / INVERTED ID / VI SNAPSHOT");
        text(frame, 80, 12, b"NTSC 480I - SOURCE ID");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_bytes_and_independent_levels_are_exact() {
        assert_eq!(grey(16).to_be_bytes(), [16, 128, 16, 128]);
        assert_eq!(pair(1, 80, 100, 0).to_be_bytes(), [16, 128, 16, 128]);
        assert_eq!(pair(1, 520, 100, 0).to_be_bytes(), [235, 128, 235, 128]);
        assert_eq!(pair(2, 400, 100, 0).to_be_bytes(), [65, 100, 65, 212]);
    }
    #[test]
    fn single_pixel_detail_and_moving_bar_are_not_flat() {
        assert_eq!(pair(3, 100, 100, 0).to_be_bytes(), [235, 128, 16, 128]);
        assert_eq!(pair(5, 0, 300, 0).to_be_bytes(), [235, 128, 235, 128]);
        assert_eq!(pair(5, 0, 300, 8).to_be_bytes(), [32, 128, 32, 128]);
        assert_eq!(pair(5, 100, 100, 0).to_be_bytes(), [235, 128, 235, 128]);
        assert_eq!(pair(5, 100, 101, 0).to_be_bytes(), [16, 128, 16, 128]);
    }
    #[test]
    fn render_all_pages_preserves_measurement_regions() {
        let mut frame = std::vec![0; WIDTH*HEIGHT/2];
        for page in 0..NAMES.len() {
            render(&mut frame, page, 0, 0, true, false);
            assert_eq!(frame[100 * 320 + 50], pair(page, 100, 100, 0));
            assert_eq!(frame[300 * 320 + 200], pair(page, 400, 300, 0));
        }
    }
}
