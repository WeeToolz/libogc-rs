//! Binary source markers. Row parity is spatial, not a claim about field order.
use crate::patterns::grey;

pub const PAGE: usize = 7;
pub const BIT_X: usize = 64;
pub const BIT_WIDTH: usize = 16;
pub const BAND_HEIGHT: usize = 32;
// Per parity: presentation ID, inverted presentation ID, sampled VI retrace.
pub const BANDS: [[usize; 3]; 2] = [[104, 144, 184], [264, 304, 344]];

pub fn pair(x: usize, y: usize, presentation: u32, retrace: u32) -> u32 {
    for (parity, bands) in BANDS.iter().enumerate() {
        for (kind, &top) in bands.iter().enumerate() {
            if !(top..top + BAND_HEIGHT).contains(&y) {
                continue;
            }
            if y % 2 != parity {
                return grey(128);
            }
            if (32..48).contains(&x) || (592..608).contains(&x) {
                return grey(235);
            }
            if (48..BIT_X).contains(&x) || (576..592).contains(&x) {
                return grey(16);
            }
            if (BIT_X..BIT_X + 32 * BIT_WIDTH).contains(&x) {
                let word = match kind {
                    0 => presentation,
                    1 => !presentation,
                    _ => retrace,
                };
                let bit = 31 - (x - BIT_X) / BIT_WIDTH;
                return grey(if word & (1 << bit) != 0 { 235 } else { 16 });
            }
        }
    }
    grey(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_words_are_msb_first_with_an_inverted_copy() {
        // 0x80000001: only the first and last cells are white.
        assert_eq!(pair(64, 104, 0x8000_0001, 0), 0xeb80_eb80);
        assert_eq!(pair(80, 104, 0x8000_0001, 0), 0x1080_1080);
        assert_eq!(pair(560, 104, 0x8000_0001, 0), 0xeb80_eb80);
        assert_eq!(pair(64, 144, 0x8000_0001, 0), 0x1080_1080);
        assert_eq!(pair(80, 144, 0x8000_0001, 0), 0xeb80_eb80);
    }

    #[test]
    fn source_parities_have_distinct_bands_and_neutral_other_rows() {
        assert_eq!(pair(64, 104, u32::MAX, 0), 0xeb80_eb80);
        assert_eq!(pair(64, 105, u32::MAX, 0), 0x8080_8080);
        assert_eq!(pair(64, 264, u32::MAX, 0), 0x8080_8080);
        assert_eq!(pair(64, 265, u32::MAX, 0), 0xeb80_eb80);
        assert_eq!(pair(32, 105, 0, 0), 0x8080_8080);
        assert_eq!(pair(32, 265, 0, 0), 0xeb80_eb80);
    }

    #[test]
    fn retrace_is_independent_of_presentation_and_handles_wrap() {
        assert_eq!(pair(64, 184, 0, 0x8000_0000), 0xeb80_eb80);
        assert_eq!(pair(560, 184, u32::MAX, 0x8000_0000), 0x1080_1080);
        assert_eq!(pair(560, 345, 0, 1), 0xeb80_eb80);
        assert_eq!(pair(560, 345, 0, 0), 0x1080_1080);
    }
}
