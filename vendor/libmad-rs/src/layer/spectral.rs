//! Layer III spectral-line reorder and alias reduction from `layer3.c`.

use crate::{
    fixed::{self, Fixed},
    layer::layer3::{ChannelSideInfo, FLAG_MIXED_BLOCK},
};

const CS: [Fixed; 8] = [
    0x0db84a81, 0x0e1b9d7f, 0x0f31adcf, 0x0fbba815, 0x0feda417, 0x0ffc8fc8, 0x0fff964c, 0x0ffff8d3,
];

const CA: [Fixed; 8] = [
    -0x083b5fe7,
    -0x078c36d2,
    -0x05039814,
    -0x02e91dd1,
    -0x0183603a,
    -0x00a7cb87,
    -0x003a2847,
    -0x000f27b4,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpectralError {
    NotShortBlock,
    InvalidBandWidths,
    InvalidLineCount,
}

/// Reorder short-block frequency lines into subband and window order.
///
/// `band_widths` is the per-window scalefactor-band width list used by the C
/// decoder. For a mixed block it starts with the long-block widths up to
/// spectral line 36, followed by the short-block widths.
pub fn reorder_short(
    samples: &mut [Fixed; 576],
    channel: &ChannelSideInfo,
    band_widths: &[u8; 39],
) -> Result<(), SpectralError> {
    if channel.block_type != 2 {
        return Err(SpectralError::NotShortBlock);
    }

    let mixed = channel.flags & FLAG_MIXED_BLOCK != 0;
    let start_subband = if mixed { 2 } else { 0 };
    let mut width_index = 0;
    if mixed {
        let mut long_lines = 0;
        while long_lines < 36 {
            let Some(&width) = band_widths.get(width_index) else {
                return Err(SpectralError::InvalidBandWidths);
            };
            if width == 0 {
                return Err(SpectralError::InvalidBandWidths);
            }
            long_lines += usize::from(width);
            width_index += 1;
        }
    }

    let reorder_len = 576 - 18 * start_subband;
    let mut temp = [0; 576];
    let mut written = [false; 576];
    let mut subband = [start_subband; 3];
    let mut in_subband = [0; 3];
    let mut window = 0;
    let Some(&first_width) = band_widths.get(width_index) else {
        return Err(SpectralError::InvalidBandWidths);
    };
    if first_width == 0 {
        return Err(SpectralError::InvalidBandWidths);
    }
    let mut width_left = usize::from(first_width);
    width_index += 1;

    for &sample in samples.iter().skip(18 * start_subband) {
        if width_left == 0 {
            let Some(&width) = band_widths.get(width_index) else {
                return Err(SpectralError::InvalidBandWidths);
            };
            if width == 0 {
                return Err(SpectralError::InvalidBandWidths);
            }
            width_left = usize::from(width);
            width_index += 1;
            window = (window + 1) % 3;
        }
        width_left -= 1;

        let offset = (subband[window] - start_subband) * 18 + window * 6 + in_subband[window];
        if offset >= temp.len() || written[offset] {
            return Err(SpectralError::InvalidBandWidths);
        }
        temp[offset] = sample;
        written[offset] = true;
        in_subband[window] += 1;
        if in_subband[window] == 6 {
            in_subband[window] = 0;
            subband[window] += 1;
        }
    }

    if written[..reorder_len].iter().any(|slot| !slot) {
        return Err(SpectralError::InvalidBandWidths);
    }
    samples[18 * start_subband..].copy_from_slice(&temp[..reorder_len]);
    Ok(())
}

/// Reduce aliasing at the subband boundaries in the first `lines` values.
///
/// The C decoder calls this with 36 lines for mixed short blocks and 576
/// lines for long blocks. Other counts retain C's boundary-loop behavior.
pub fn alias_reduce(samples: &mut [Fixed; 576], lines: usize) -> Result<(), SpectralError> {
    if lines > samples.len() {
        return Err(SpectralError::InvalidLineCount);
    }

    let mut boundary = 18;
    while boundary < lines {
        for i in 0..8 {
            let upper_index = boundary - 1 - i;
            let lower_index = boundary + i;
            let upper = samples[upper_index];
            let lower = samples[lower_index];
            // FPM_DEFAULT rounds each product separately, then adds in the
            // fixed-point type. Preserve the target's wrapping negation too.
            samples[upper_index] = fixed::mul_configured(upper, CS[i])
                .wrapping_add(fixed::mul_configured(lower.wrapping_neg(), CA[i]));
            samples[lower_index] = fixed::mul_configured(lower, CS[i])
                .wrapping_add(fixed::mul_configured(upper, CA[i]));
        }
        boundary += 18;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT_WIDTHS: [u8; 39] = [
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 6, 6, 6, 8, 8, 8, 10, 10, 10, 12, 12, 12, 14, 14, 14,
        18, 18, 18, 22, 22, 22, 30, 30, 30, 56, 56, 56,
    ];

    #[test]
    fn short_reorder_places_window_lines_in_subband_order() {
        let mut samples = [0; 576];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = index as i32;
        }
        let channel = ChannelSideInfo {
            block_type: 2,
            ..ChannelSideInfo::default()
        };
        reorder_short(&mut samples, &channel, &SHORT_WIDTHS).unwrap();
        assert_eq!(&samples[..4], &[0, 1, 2, 3]);
        assert_eq!(&samples[6..10], &[4, 5, 6, 7]);
        assert_eq!(&samples[12..16], &[8, 9, 10, 11]);
        assert_eq!(&samples[4..6], &[12, 13]);
        assert_eq!(&samples[10..12], &[16, 17]);
    }

    #[test]
    fn mixed_reorder_preserves_first_two_subbands() {
        let mut samples = [0; 576];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = index as i32;
        }
        let mixed_widths = [
            4, 4, 4, 4, 4, 4, 6, 6, 4, 4, 4, 6, 6, 6, 6, 6, 6, 10, 10, 10, 12, 12, 12, 14, 14, 14,
            16, 16, 16, 20, 20, 20, 26, 26, 26, 66, 66, 66, 0,
        ];
        let channel = ChannelSideInfo {
            block_type: 2,
            flags: FLAG_MIXED_BLOCK,
            ..ChannelSideInfo::default()
        };
        reorder_short(&mut samples, &channel, &mixed_widths).unwrap();
        assert_eq!(
            &samples[..36],
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
            ]
        );
    }

    #[test]
    fn alias_reduction_uses_c_coefficients_and_boundary_equations() {
        let mut samples = [0; 576];
        samples[17] = fixed::ONE;
        alias_reduce(&mut samples, 36).unwrap();
        // C FPM_DEFAULT rounds the operands separately: ONE becomes 65536,
        // CS[0] becomes 3512, and CA[0] becomes -2107.
        assert_eq!(samples[17], 230_162_432);
        assert_eq!(samples[18], -138_084_352);
        assert!(samples[0..9].iter().all(|sample| *sample == 0));
        assert!(samples[26..36].iter().all(|sample| *sample == 0));
    }

    #[test]
    fn alias_reduction_rejects_a_line_count_outside_the_spectral_buffer() {
        assert_eq!(
            alias_reduce(&mut [0; 576], 577),
            Err(SpectralError::InvalidLineCount)
        );
    }

    #[test]
    fn alias_reduction_wraps_negation_of_minimum_fixed_value() {
        let mut samples = [0; 576];
        samples[18] = i32::MIN;

        alias_reduce(&mut samples, 36).unwrap();

        // On the target, negating i32::MIN wraps to i32::MIN. The two FPM
        // products are then added in the fixed-point type.
        assert_eq!(samples[17], 1_104_674_816);
        assert_eq!(samples[18], -1_841_299_456);
    }
}
