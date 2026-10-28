//! Conversion helpers for Wii Y0 Cb Y1 Cr framebuffers.

pub(crate) fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let y = i32::from(y) - 16;
    let cb = i32::from(cb) - 128;
    let cr = i32::from(cr) - 128;
    [
        ((298 * y + 409 * cr + 128) >> 8).clamp(0, 255) as u8,
        ((298 * y - 100 * cb - 208 * cr + 128) >> 8).clamp(0, 255) as u8,
        ((298 * y + 516 * cb + 128) >> 8).clamp(0, 255) as u8,
    ]
}

pub(crate) fn rgb_to_ycbcr(rgb: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(i32::from);
    [
        (16 + ((66 * r + 129 * g + 25 * b + 128) >> 8)).clamp(16, 235) as u8,
        (128 + ((-38 * r - 74 * g + 112 * b + 128) >> 8)).clamp(16, 240) as u8,
        (128 + ((112 * r - 94 * g - 18 * b + 128) >> 8)).clamp(16, 240) as u8,
    ]
}

pub(crate) fn pack_ycbcr_pair(
    original: [u8; 4],
    rgb: [[u8; 3]; 2],
    update_luma: [bool; 2],
) -> [u8; 4] {
    let first = rgb_to_ycbcr(rgb[0]);
    let second = rgb_to_ycbcr(rgb[1]);
    [
        if update_luma[0] {
            first[0]
        } else {
            original[0]
        },
        ((u16::from(first[1]) + u16::from(second[1])) / 2) as u8,
        if update_luma[1] {
            second[0]
        } else {
            original[2]
        },
        ((u16::from(first[2]) + u16::from(second[2])) / 2) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::{pack_ycbcr_pair, rgb_to_ycbcr, ycbcr_to_rgb};

    #[test]
    fn rgb_conversion_matches_the_wii_xfb_range() {
        assert_eq!(rgb_to_ycbcr([0, 0, 0]), [16, 128, 128]);
        assert_eq!(rgb_to_ycbcr([255, 255, 255]), [235, 128, 128]);
        assert_eq!(rgb_to_ycbcr([255, 0, 0]), [82, 90, 240]);
        assert_eq!(rgb_to_ycbcr([0, 255, 0]), [144, 54, 34]);
        assert_eq!(rgb_to_ycbcr([0, 0, 255]), [41, 240, 110]);
    }

    #[test]
    fn primary_colors_round_trip_with_small_rounding_error() {
        for color in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
        ] {
            let [y, cb, cr] = rgb_to_ycbcr(color);
            let decoded = ycbcr_to_rgb(y, cb, cr);
            for (actual, expected) in decoded.into_iter().zip(color) {
                assert!(actual.abs_diff(expected) <= 3, "{color:?} -> {decoded:?}");
            }
        }
    }

    #[test]
    fn preserves_luma_for_an_untouched_pixel_in_a_pair() {
        let original = [89, 122, 173, 137];
        let rgb = [
            ycbcr_to_rgb(original[0], original[1], original[3]),
            ycbcr_to_rgb(original[2], original[1], original[3]),
        ];

        let packed = pack_ycbcr_pair(original, rgb, [true, false]);

        assert_eq!(packed[2], original[2]);
    }
}
