//! Layer III joint stereo processing from `III_stereo()` in `layer3.c`.

use crate::{
    fixed,
    frame::{FLAG_I_STEREO, FLAG_MS_STEREO},
};

const I_STEREO: u8 = 0x01;
const MS_STEREO: u8 = 0x02;
const IS_TABLE: [i32; 7] = [
    0x0000_0000,
    0x0361_962f,
    0x05db_3d74,
    0x0800_0000,
    0x0a24_c28c,
    0x0c9e_69d1,
    0x1000_0000,
];
const IS_LSF_TABLE: [[i32; 15]; 2] = [
    [
        0x0d74_4fcd,
        0x0b50_4f33,
        0x0983_7f05,
        0x0800_0000,
        0x06ba_27e6,
        0x05a8_279a,
        0x04c1_bf83,
        0x0400_0000,
        0x035d_13f3,
        0x02d4_13cd,
        0x0260_dfc1,
        0x0200_0000,
        0x01ae_89fa,
        0x016a_09e6,
        0x0130_6fe1,
    ],
    [
        0x0b50_4f33,
        0x0800_0000,
        0x05a8_279a,
        0x0400_0000,
        0x02d4_13cd,
        0x0200_0000,
        0x016a_09e6,
        0x0100_0000,
        0x00b5_04f3,
        0x0080_0000,
        0x005a_827a,
        0x0040_0000,
        0x002d_413d,
        0x0020_0000,
        0x0016_a09e,
    ],
];
const INVSQRT2: i32 = 0x0b50_4f33;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StereoError {
    BadStereo,
    InvalidBandWidths,
    MissingIllegalPositionFlags,
}

/// Apply intensity and middle/side stereo to one Layer III granule.
///
/// The two coefficient arrays contain 576 frequency lines each. `sfbwidth`
/// must be the long, short, or mixed width table selected for the left
/// channel. For LSF intensity stereo, `illegal_positions` contains the
/// scalefactor decoder's illegal-position flags for the right channel.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)]
pub fn process_joint_stereo(
    xr: &mut [[i32; 576]; 2],
    mode_extension: u8,
    header_flags: &mut u32,
    lsf: bool,
    right_scalefac_compress: u16,
    left_block_type: u8,
    right_block_type: u8,
    left_mixed: bool,
    right_mixed: bool,
    right_scalefactors: &[u8; 39],
    illegal_positions: Option<&[u8; 39]>,
    sfbwidth: &[u8; 39],
) -> Result<(), StereoError> {
    if left_block_type != right_block_type || left_mixed != right_mixed {
        return Err(StereoError::BadStereo);
    }

    let mut modes = [mode_extension; 39];
    if mode_extension & I_STEREO != 0 {
        *header_flags |= FLAG_I_STEREO;
        disable_intensity_in_nonzero_right_bands(
            &mut modes,
            mode_extension,
            right_block_type == 2,
            right_mixed,
            &xr[1],
            sfbwidth,
        )?;
        apply_intensity(
            xr,
            &mut modes,
            lsf,
            right_scalefac_compress,
            right_scalefactors,
            illegal_positions,
            sfbwidth,
        )?;
    }

    if mode_extension & MS_STEREO != 0 {
        *header_flags |= FLAG_MS_STEREO;
        for (band, &width) in sfbwidth.iter().enumerate() {
            if modes[band] != MS_STEREO {
                continue;
            }
            let start = band_start(sfbwidth, band)?;
            let end = start + usize::from(width);
            if end > 576 {
                return Err(StereoError::InvalidBandWidths);
            }
            for line in start..end {
                let mid = xr[0][line];
                let side = xr[1][line];
                xr[0][line] = fixed::mul_configured(mid.wrapping_add(side), INVSQRT2);
                xr[1][line] = fixed::mul_configured(mid.wrapping_sub(side), INVSQRT2);
            }
        }
    }
    Ok(())
}

fn band_start(widths: &[u8; 39], band: usize) -> Result<usize, StereoError> {
    widths[..band]
        .iter()
        .try_fold(0_usize, |sum, &width| sum.checked_add(usize::from(width)))
        .filter(|&start| start <= 576)
        .ok_or(StereoError::InvalidBandWidths)
}

#[allow(clippy::needless_range_loop)]
fn disable_intensity_in_nonzero_right_bands(
    modes: &mut [u8; 39],
    mode_extension: u8,
    short: bool,
    mixed: bool,
    right: &[i32; 576],
    widths: &[u8; 39],
) -> Result<(), StereoError> {
    let mut line = 0_usize;
    let mut band = 0_usize;
    let mut lower = 0_usize;
    let mut start = 0_usize;
    let mut max = 0_usize;
    let mut bounds = [0_usize; 3];

    if short {
        if mixed {
            while line < 36 {
                let width = usize::from(*widths.get(band).ok_or(StereoError::InvalidBandWidths)?);
                let end = line
                    .checked_add(width)
                    .ok_or(StereoError::InvalidBandWidths)?;
                if width == 0 || end > 576 {
                    return Err(StereoError::InvalidBandWidths);
                }
                if right[line..end].iter().any(|&coefficient| coefficient != 0) {
                    lower = band + 1;
                }
                line = end;
                band += 1;
            }
            start = band;
        }

        let mut window = 0;
        while line < 576 {
            let width = usize::from(*widths.get(band).ok_or(StereoError::InvalidBandWidths)?);
            let end = line
                .checked_add(width)
                .ok_or(StereoError::InvalidBandWidths)?;
            if width == 0 || end > 576 {
                return Err(StereoError::InvalidBandWidths);
            }
            if right[line..end].iter().any(|&coefficient| coefficient != 0) {
                max = band + 1;
                bounds[window] = band + 1;
            }
            line = end;
            band += 1;
            window = (window + 1) % 3;
        }
        if max != 0 {
            lower = start;
        }
        for mode in modes.iter_mut().take(lower) {
            *mode = mode_extension & !I_STEREO;
        }
        let mut window = 0;
        for band in start..max {
            if band < bounds[window] {
                modes[band] = mode_extension & !I_STEREO;
            }
            window = (window + 1) % 3;
        }
    } else {
        let mut bound = 0;
        while line < 576 {
            let width = usize::from(*widths.get(band).ok_or(StereoError::InvalidBandWidths)?);
            let end = line
                .checked_add(width)
                .ok_or(StereoError::InvalidBandWidths)?;
            if width == 0 || end > 576 {
                return Err(StereoError::InvalidBandWidths);
            }
            if right[line..end].iter().any(|&coefficient| coefficient != 0) {
                bound = band + 1;
            }
            line = end;
            band += 1;
        }
        for mode in modes.iter_mut().take(bound) {
            *mode = mode_extension & !I_STEREO;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)]
fn apply_intensity(
    xr: &mut [[i32; 576]; 2],
    modes: &mut [u8; 39],
    lsf: bool,
    scalefac_compress: u16,
    scalefactors: &[u8; 39],
    illegal_positions: Option<&[u8; 39]>,
    widths: &[u8; 39],
) -> Result<(), StereoError> {
    if lsf && illegal_positions.is_none() {
        return Err(StereoError::MissingIllegalPositionFlags);
    }
    let lsf_scale = usize::from(scalefac_compress & 1);
    let mut line = 0_usize;
    for (band, &width) in widths.iter().enumerate() {
        if line == 576 {
            break;
        }
        let width = usize::from(width);
        let end = line
            .checked_add(width)
            .ok_or(StereoError::InvalidBandWidths)?;
        if width == 0 || end > 576 {
            return Err(StereoError::InvalidBandWidths);
        }
        line = end;
        if modes[band] & I_STEREO == 0 {
            continue;
        }
        let position = usize::from(scalefactors[band]);
        if lsf {
            if illegal_positions.is_some_and(|positions| positions[band] != 0) {
                modes[band] &= !I_STEREO;
                continue;
            }
            if position == 0 {
                for sample in line - width..line {
                    xr[1][sample] = xr[0][sample];
                }
                continue;
            }
            let Some(&coefficient) = IS_LSF_TABLE[lsf_scale].get((position - 1) / 2) else {
                modes[band] &= !I_STEREO;
                continue;
            };
            for sample in line - width..line {
                let left = xr[0][sample];
                let opposite = fixed::mul_configured(left, coefficient);
                if position & 1 != 0 {
                    xr[0][sample] = opposite;
                    xr[1][sample] = left;
                } else {
                    xr[1][sample] = opposite;
                }
            }
        } else {
            if position >= 7 {
                modes[band] &= !I_STEREO;
                continue;
            }
            for sample in line - width..line {
                let left = xr[0][sample];
                xr[0][sample] = fixed::mul_configured(left, IS_TABLE[position]);
                xr[1][sample] = fixed::mul_configured(left, IS_TABLE[6 - position]);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long_widths() -> [u8; 39] {
        let mut widths = [0; 39];
        widths[..22].copy_from_slice(&[
            4, 4, 4, 4, 4, 4, 6, 6, 8, 8, 10, 12, 16, 20, 24, 28, 34, 42, 50, 54, 76, 158,
        ]);
        widths
    }

    #[test]
    fn middle_side_converts_mid_and_side_to_left_and_right() {
        let mut xr = [[0; 576]; 2];
        xr[0][0] = fixed::ONE;
        xr[1][0] = fixed::ONE / 2;
        let mut flags = 0;
        process_joint_stereo(
            &mut xr,
            MS_STEREO,
            &mut flags,
            false,
            0,
            0,
            0,
            false,
            false,
            &[0; 39],
            None,
            &long_widths(),
        )
        .unwrap();
        assert_eq!(flags & FLAG_MS_STEREO, FLAG_MS_STEREO);
        assert_eq!(
            xr[0][0],
            fixed::mul_configured(3 * fixed::ONE / 2, INVSQRT2)
        );
        assert_eq!(xr[1][0], fixed::mul_configured(fixed::ONE / 2, INVSQRT2));
    }

    #[test]
    fn mpeg1_intensity_stereo_uses_right_scalefactor_position() {
        let mut xr = [[0; 576]; 2];
        xr[0][0] = fixed::ONE;
        let mut positions = [0; 39];
        positions[0] = 2;
        let mut flags = 0;
        process_joint_stereo(
            &mut xr,
            I_STEREO,
            &mut flags,
            false,
            0,
            0,
            0,
            false,
            false,
            &positions,
            None,
            &long_widths(),
        )
        .unwrap();
        assert_eq!(flags & FLAG_I_STEREO, FLAG_I_STEREO);
        // FPM_DEFAULT computes ((x + 2048) >> 12) * ((y + 32768) >> 16).
        // With x = 1.0, its rounded coefficient values are 1499 and 2597.
        assert_eq!(xr[0][0], 98_238_464);
        assert_eq!(xr[1][0], 170_196_992);
    }

    #[test]
    fn combined_stereo_keeps_all_bands_through_last_nonzero_right_band_in_ms() {
        let mut xr = [[0; 576]; 2];
        xr[0][0] = fixed::ONE;
        xr[1][0] = fixed::ONE / 2;
        xr[0][4] = fixed::ONE / 2;
        xr[1][4] = fixed::ONE / 4;
        let positions = [1; 39];
        let mut flags = 0;

        process_joint_stereo(
            &mut xr,
            I_STEREO | MS_STEREO,
            &mut flags,
            false,
            0,
            0,
            0,
            false,
            false,
            &positions,
            None,
            &long_widths(),
        )
        .unwrap();

        assert_eq!(
            flags & (FLAG_I_STEREO | FLAG_MS_STEREO),
            FLAG_I_STEREO | FLAG_MS_STEREO
        );
        assert_eq!(
            xr[0][0],
            fixed::mul_configured(fixed::ONE + fixed::ONE / 2, INVSQRT2)
        );
        assert_eq!(
            xr[1][0],
            fixed::mul_configured(fixed::ONE - fixed::ONE / 2, INVSQRT2)
        );
        assert_eq!(
            xr[0][4],
            fixed::mul_configured(fixed::ONE / 2 + fixed::ONE / 4, INVSQRT2)
        );
        assert_eq!(
            xr[1][4],
            fixed::mul_configured(fixed::ONE / 2 - fixed::ONE / 4, INVSQRT2)
        );
        assert_ne!(xr[0][4], fixed::mul_configured(fixed::ONE / 2, IS_TABLE[1]));
    }

    #[test]
    fn lsf_intensity_stereo_honors_illegal_position_and_scale() {
        let mut xr = [[0; 576]; 2];
        xr[0][0] = fixed::ONE;
        let mut positions = [0; 39];
        positions[0] = 1;
        let mut illegal = [0; 39];
        let mut flags = 0;
        process_joint_stereo(
            &mut xr,
            I_STEREO,
            &mut flags,
            true,
            1,
            0,
            0,
            false,
            false,
            &positions,
            Some(&illegal),
            &long_widths(),
        )
        .unwrap();
        assert_eq!(
            xr[0][0],
            fixed::mul_configured(fixed::ONE, IS_LSF_TABLE[1][0])
        );
        assert_eq!(xr[1][0], fixed::ONE);

        illegal[0] = 1;
        xr = [[0; 576]; 2];
        xr[0][0] = fixed::ONE;
        process_joint_stereo(
            &mut xr,
            I_STEREO,
            &mut flags,
            true,
            1,
            0,
            0,
            false,
            false,
            &positions,
            Some(&illegal),
            &long_widths(),
        )
        .unwrap();
        assert_eq!(xr[0][0], fixed::ONE);
        assert_eq!(xr[1][0], 0);
    }
}
