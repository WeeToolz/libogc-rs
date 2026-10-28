//! Layer III per-channel spectral transform from the transform section of
//! `III_decode()` in `layer3.c`.

use crate::{
    fixed::Fixed,
    layer::{
        imdct::{imdct_long, imdct_short},
        layer3::{ChannelSideInfo, FLAG_MIXED_BLOCK},
        overlap::{frequency_invert, overlap_add, overlap_zero},
        spectral::{SpectralError, alias_reduce, reorder_short},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformError {
    InvalidBlockType,
    Spectral(SpectralError),
}

impl From<SpectralError> for TransformError {
    fn from(value: SpectralError) -> Self {
        Self::Spectral(value)
    }
}

/// Reorder, alias-reduce, transform, overlap, and frequency-invert one Layer
/// III channel granule. `spectral` is mutable because the C path reorders and
/// alias-reduces it in place. `overlap` is indexed by subband; `samples` uses
/// C's `[time][subband]` layout.
pub fn transform_channel(
    spectral: &mut [Fixed; 576],
    channel: &ChannelSideInfo,
    band_widths: &[u8; 39],
    overlap: &mut [[Fixed; 18]; 32],
    samples: &mut [[Fixed; 32]; 18],
) -> Result<(), TransformError> {
    if channel.block_type > 3 {
        return Err(TransformError::InvalidBlockType);
    }
    let mixed = channel.flags & FLAG_MIXED_BLOCK != 0;

    if channel.block_type == 2 {
        reorder_short(spectral, channel, band_widths)?;
        // libmad applies alias reduction to the lower long blocks of mixed
        // short granules unless OPT_STRICT is enabled.
        if mixed {
            alias_reduce(spectral, 36)?;
        }
    } else {
        alias_reduce(spectral, 576)?;
    }

    let mut line = 0;
    if channel.block_type != 2 || mixed {
        let long_type = if mixed { 0 } else { channel.block_type };
        for (subband, saved_overlap) in overlap.iter_mut().enumerate().take(2) {
            let mut output = [0; 36];
            let input: &[Fixed; 18] = spectral[line..line + 18].try_into().expect("18-line block");
            imdct_long(input, &mut output, u32::from(long_type));
            overlap_add(&output, saved_overlap, samples, subband);
            line += 18;
        }
    } else {
        for (subband, saved_overlap) in overlap.iter_mut().enumerate().take(2) {
            let mut output = [0; 36];
            let input: &[Fixed; 18] = spectral[line..line + 18].try_into().expect("18-line block");
            imdct_short(input, &mut output);
            overlap_add(&output, saved_overlap, samples, subband);
            line += 18;
        }
    }
    frequency_invert(samples, 1);

    let mut last_line = 576;
    while last_line > 36 && spectral[last_line - 1] == 0 {
        last_line -= 1;
    }
    let subband_limit = 32 - (576 - last_line) / 18;

    for (subband, saved_overlap) in overlap.iter_mut().enumerate().take(subband_limit).skip(2) {
        let mut output = [0; 36];
        let input: &[Fixed; 18] = spectral[line..line + 18].try_into().expect("18-line block");
        if channel.block_type == 2 {
            imdct_short(input, &mut output);
        } else {
            imdct_long(input, &mut output, u32::from(channel.block_type));
        }
        overlap_add(&output, saved_overlap, samples, subband);
        if subband & 1 != 0 {
            frequency_invert(samples, subband);
        }
        line += 18;
    }

    for (subband, saved_overlap) in overlap.iter_mut().enumerate().skip(subband_limit) {
        overlap_zero(saved_overlap, samples, subband);
        if subband & 1 != 0 {
            frequency_invert(samples, subband);
        }
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
    fn zero_long_granule_drains_all_saved_overlap() {
        let mut spectral = [0; 576];
        let channel = ChannelSideInfo::default();
        let mut overlap = [[0; 18]; 32];
        let mut samples = [[0; 32]; 18];
        for (sb, band) in overlap.iter_mut().enumerate() {
            band.fill(sb as i32 + 1);
        }

        transform_channel(
            &mut spectral,
            &channel,
            &SHORT_WIDTHS,
            &mut overlap,
            &mut samples,
        )
        .unwrap();
        for (i, row) in samples.iter().enumerate() {
            assert_eq!(row[0], 1);
            assert_eq!(row[1], if i & 1 == 0 { 2 } else { -2 });
            for (sb, &actual) in row.iter().enumerate().skip(2) {
                let value = sb as i32 + 1;
                let expected = if sb & 1 != 0 && i & 1 != 0 {
                    -value
                } else {
                    value
                };
                assert_eq!(actual, expected);
            }
        }
        assert!(overlap.iter().flatten().all(|&value| value == 0));
    }

    #[test]
    fn long_impulse_produces_signal_in_first_two_bands() {
        let mut spectral = [0; 576];
        spectral[0] = 1 << 27;
        let channel = ChannelSideInfo::default();
        let mut overlap = [[0; 18]; 32];
        let mut samples = [[0; 32]; 18];
        transform_channel(
            &mut spectral,
            &channel,
            &SHORT_WIDTHS,
            &mut overlap,
            &mut samples,
        )
        .unwrap();
        assert!(samples.iter().any(|row| row[0] != 0));
        assert!(
            samples
                .iter()
                .all(|row| row[1..].iter().all(|&value| value == 0))
        );
    }

    #[test]
    fn short_mixed_and_nonmixed_paths_accept_valid_band_widths() {
        let mut short = [0; 576];
        let mut short_channel = ChannelSideInfo {
            block_type: 2,
            ..ChannelSideInfo::default()
        };
        let mut overlap = [[0; 18]; 32];
        let mut samples = [[0; 32]; 18];
        transform_channel(
            &mut short,
            &short_channel,
            &SHORT_WIDTHS,
            &mut overlap,
            &mut samples,
        )
        .unwrap();

        short_channel.flags |= FLAG_MIXED_BLOCK;
        let mixed_widths = [
            4, 4, 4, 4, 4, 4, 6, 6, 4, 4, 4, 6, 6, 6, 6, 6, 6, 10, 10, 10, 12, 12, 12, 14, 14, 14,
            16, 16, 16, 20, 20, 20, 26, 26, 26, 66, 66, 66, 0,
        ];
        transform_channel(
            &mut short,
            &short_channel,
            &mixed_widths,
            &mut overlap,
            &mut samples,
        )
        .unwrap();
    }

    #[test]
    fn invalid_block_type_is_reported() {
        let mut spectral = [0; 576];
        let channel = ChannelSideInfo {
            block_type: 4,
            ..ChannelSideInfo::default()
        };
        let mut overlap = [[0; 18]; 32];
        let mut samples = [[0; 32]; 18];
        assert_eq!(
            transform_channel(
                &mut spectral,
                &channel,
                &SHORT_WIDTHS,
                &mut overlap,
                &mut samples
            ),
            Err(TransformError::InvalidBlockType)
        );
    }
}
