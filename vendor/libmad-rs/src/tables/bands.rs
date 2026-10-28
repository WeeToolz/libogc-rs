//! Layer III scalefactor band widths from `sfbwidth_table` in `layer3.c`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BandLayout {
    Long,
    Short,
    Mixed,
}

/// Fixed storage for one band-width row. `len` excludes zero padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BandWidths {
    values: [u8; 39],
    len: u8,
}

impl BandWidths {
    pub fn as_slice(&self) -> &[u8] {
        // Each table row has a compile-time checked length no greater than 39.
        &self.values[..self.len as usize]
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn total_lines(&self) -> usize {
        self.as_slice()
            .iter()
            .map(|&width| usize::from(width))
            .sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BandSet {
    pub long: BandWidths,
    pub short: BandWidths,
    pub mixed: BandWidths,
}

impl BandSet {
    pub const fn get(&self, layout: BandLayout) -> &BandWidths {
        match layout {
            BandLayout::Long => &self.long,
            BandLayout::Short => &self.short,
            BandLayout::Mixed => &self.mixed,
        }
    }
}

const fn widths<const N: usize>(input: [u8; N]) -> BandWidths {
    assert!(N <= 39);
    let mut values = [0; 39];
    let mut index = 0;
    while index < N {
        values[index] = input[index];
        index += 1;
    }
    BandWidths {
        values,
        len: N as u8,
    }
}

const L_48000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 10, 12, 16, 18, 22, 28, 34, 40, 46, 54, 54, 192,
]);
const L_44100: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 8, 8, 10, 12, 16, 20, 24, 28, 34, 42, 50, 54, 76, 158,
]);
const L_32000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 8, 10, 12, 16, 20, 24, 30, 38, 46, 56, 68, 84, 102, 26,
]);
const S_48000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 6, 6, 6, 10, 10, 10, 12, 12, 12, 14, 14, 14, 16,
    16, 16, 20, 20, 20, 26, 26, 26, 66, 66, 66,
]);
const S_44100: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18,
    18, 18, 22, 22, 22, 30, 30, 30, 56, 56, 56,
]);
const S_32000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 8, 8, 12, 12, 12, 16, 16, 16, 20, 20, 20, 26,
    26, 26, 34, 34, 34, 42, 42, 42, 12, 12, 12,
]);
const M_48000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 4, 4, 4, 6, 6, 6, 6, 6, 6, 10, 10, 10, 12, 12, 12, 14, 14, 14, 16, 16,
    16, 20, 20, 20, 26, 26, 26, 66, 66, 66,
]);
const M_44100: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 4, 4, 4, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18, 18,
    18, 22, 22, 22, 30, 30, 30, 56, 56, 56,
]);
const M_32000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 6, 6, 4, 4, 4, 6, 6, 6, 8, 8, 8, 12, 12, 12, 16, 16, 16, 20, 20, 20, 26, 26,
    26, 34, 34, 34, 42, 42, 42, 12, 12, 12,
]);

const L_24000: BandWidths = widths([
    6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 18, 22, 26, 32, 38, 46, 54, 62, 70, 76, 36,
]);
const L_22050: BandWidths = widths([
    6, 6, 6, 6, 6, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 38, 46, 52, 60, 68, 58, 54,
]);
const S_24000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18, 18, 18,
    24, 24, 24, 32, 32, 32, 44, 44, 44, 12, 12, 12,
]);
const S_22050: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 6, 6, 6, 8, 8, 8, 10, 10, 10, 14, 14, 14, 18, 18, 18, 26,
    26, 26, 32, 32, 32, 42, 42, 42, 18, 18, 18,
]);
const S_16000: BandWidths = widths([
    4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18, 18, 18,
    24, 24, 24, 30, 30, 30, 40, 40, 40, 18, 18, 18,
]);
const M_24000: BandWidths = widths([
    6, 6, 6, 6, 6, 6, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18, 18, 18, 24, 24, 24,
    32, 32, 32, 44, 44, 44, 12, 12, 12,
]);
const M_22050: BandWidths = widths([
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 8, 8, 8, 10, 10, 10, 14, 14, 14, 18, 18, 18, 26, 26, 26,
    32, 32, 32, 42, 42, 42, 18, 18, 18,
]);
const M_16000: BandWidths = widths([
    6, 6, 6, 6, 6, 6, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14, 18, 18, 18, 24, 24, 24,
    30, 30, 30, 40, 40, 40, 18, 18, 18,
]);
const L_8000: BandWidths = widths([
    12, 12, 12, 12, 12, 12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 76, 90, 2, 2, 2, 2, 2,
]);
const S_8000: BandWidths = widths([
    8, 8, 8, 8, 8, 8, 8, 8, 8, 12, 12, 12, 16, 16, 16, 20, 20, 20, 24, 24, 24, 28, 28, 28, 36, 36,
    36, 2, 2, 2, 2, 2, 2, 2, 2, 2, 26, 26, 26,
]);
const M_8000: BandWidths = widths([
    12, 12, 12, 4, 4, 4, 8, 8, 8, 12, 12, 12, 16, 16, 16, 20, 20, 20, 24, 24, 24, 28, 28, 28, 36,
    36, 36, 2, 2, 2, 2, 2, 2, 2, 2, 2, 26, 26, 26,
]);

/// C table order: 48 kHz, 44.1 kHz, 32 kHz, 24 kHz, 22.05 kHz,
/// 16 kHz, 12 kHz, 11.025 kHz, and 8 kHz.
pub static SAMPLE_RATE_ROWS: [BandSet; 9] = [
    BandSet {
        long: L_48000,
        short: S_48000,
        mixed: M_48000,
    },
    BandSet {
        long: L_44100,
        short: S_44100,
        mixed: M_44100,
    },
    BandSet {
        long: L_32000,
        short: S_32000,
        mixed: M_32000,
    },
    BandSet {
        long: L_24000,
        short: S_24000,
        mixed: M_24000,
    },
    BandSet {
        long: L_22050,
        short: S_22050,
        mixed: M_22050,
    },
    BandSet {
        long: L_22050,
        short: S_16000,
        mixed: M_16000,
    },
    BandSet {
        long: L_22050,
        short: S_16000,
        mixed: M_16000,
    },
    BandSet {
        long: L_22050,
        short: S_16000,
        mixed: M_16000,
    },
    BandSet {
        long: L_8000,
        short: S_8000,
        mixed: M_8000,
    },
];

/// Look up a layout by C's samplerate index. Return `None` for an invalid index.
pub fn get(samplerate_index: usize, layout: BandLayout) -> Option<&'static BandWidths> {
    if samplerate_index >= SAMPLE_RATE_ROWS.len() {
        None
    } else {
        Some(SAMPLE_RATE_ROWS[samplerate_index].get(layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sfbwidth_layouts_cover_576_lines() {
        for row in &SAMPLE_RATE_ROWS {
            for layout in [BandLayout::Long, BandLayout::Short, BandLayout::Mixed] {
                let widths = row.get(layout);
                assert_eq!(widths.total_lines(), 576, "{layout:?}: {widths:?}");
            }
        }
    }

    #[test]
    fn hz_44100_rows_match_mpeg1_reference_data() {
        assert_eq!(
            get(1, BandLayout::Long).unwrap().as_slice()[..9],
            [4, 4, 4, 4, 4, 4, 6, 6, 8]
        );
        assert_eq!(
            get(1, BandLayout::Short).unwrap().as_slice()[..15],
            [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6]
        );
        assert_eq!(get(1, BandLayout::Mixed).unwrap().len(), 38);
        assert_eq!(
            get(1, BandLayout::Mixed).unwrap().as_slice()[..8],
            [4, 4, 4, 4, 4, 4, 6, 6]
        );
    }

    #[test]
    fn lsf_alias_rows_follow_the_c_samplerate_mapping() {
        assert_eq!(get(5, BandLayout::Long), get(4, BandLayout::Long));
        assert_eq!(get(6, BandLayout::Short), get(5, BandLayout::Short));
        assert_eq!(get(7, BandLayout::Mixed), get(5, BandLayout::Mixed));
        assert_eq!(
            get(8, BandLayout::Long).unwrap().as_slice()[..3],
            [12, 12, 12]
        );
        assert_eq!(get(8, BandLayout::Mixed).unwrap().len(), 39);
        assert_eq!(get(9, BandLayout::Long), None);
    }
}
