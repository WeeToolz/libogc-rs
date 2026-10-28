//! Layer III scalefactor decoding from `III_scalefactors()` in `layer3.c`.

use crate::bit::{BitError, BitPtr};

const MPEG1_SLEN: [(u8, u8); 16] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (3, 0),
    (1, 1),
    (1, 2),
    (1, 3),
    (2, 1),
    (2, 2),
    (2, 3),
    (3, 1),
    (3, 2),
    (3, 3),
    (4, 2),
    (4, 3),
];

const LSF_NSFB: [[[u8; 4]; 3]; 6] = [
    [[6, 5, 5, 5], [9, 9, 9, 9], [6, 9, 9, 9]],
    [[6, 5, 7, 3], [9, 9, 12, 6], [6, 9, 12, 6]],
    [[11, 10, 0, 0], [18, 18, 0, 0], [15, 18, 0, 0]],
    [[7, 7, 7, 0], [12, 12, 12, 0], [6, 15, 12, 0]],
    [[6, 6, 6, 3], [12, 9, 9, 6], [6, 12, 9, 6]],
    [[8, 8, 5, 0], [15, 12, 9, 0], [6, 18, 9, 0]],
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalefactorError {
    /// The part2 budget does not contain the next scalefactor.
    BadScfsi,
    /// The backing bitstream ended before the requested scalefactor.
    Truncated,
    /// MPEG-1 scalefac_compress is outside its four-bit table.
    InvalidCompress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedScalefactors {
    pub values: [u8; 39],
    /// Intensity positions produced by the LSF right channel branch.
    pub intensity: Option<[u8; 39]>,
    /// LSF scalefac_compress values 500 and above set preflag in C.
    pub preflag: bool,
    pub part2_length: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Mpeg1ScalefactorParams<'a> {
    pub scalefac_compress: u8,
    pub block_type: u8,
    pub mixed_block: bool,
    pub scfsi: u8,
    pub granule_zero: Option<&'a [u8; 39]>,
}

fn read(
    ptr: &mut BitPtr,
    data: &[u8],
    bits_left: &mut usize,
    width: u8,
) -> Result<u8, ScalefactorError> {
    if *bits_left < usize::from(width) {
        return Err(ScalefactorError::BadScfsi);
    }
    let value = ptr
        .read(data, u32::from(width))
        .map_err(|error| match error {
            BitError::OutOfBounds | BitError::TooManyBits => ScalefactorError::Truncated,
        })? as u8;
    *bits_left -= usize::from(width);
    Ok(value)
}

fn part2_length(start: BitPtr, end: BitPtr) -> usize {
    BitPtr::bit_length(start, end).max(0) as usize
}

/// Decode MPEG-1 scalefactors for one channel and granule.
///
/// `scfsi` uses the four MPEG-1 reuse bits. Reused values come from
/// `granule_zero`; short blocks read the short-block layout and ignore SCFSI.
pub fn decode_mpeg1(
    ptr: &mut BitPtr,
    data: &[u8],
    params: Mpeg1ScalefactorParams<'_>,
    bits_left: usize,
) -> Result<DecodedScalefactors, ScalefactorError> {
    let Mpeg1ScalefactorParams {
        scalefac_compress,
        block_type,
        mixed_block,
        scfsi,
        granule_zero,
    } = params;
    let (slen1, slen2) = *MPEG1_SLEN
        .get(usize::from(scalefac_compress))
        .ok_or(ScalefactorError::InvalidCompress)?;
    let start = *ptr;
    let mut remaining = bits_left;
    let mut values = [0; 39];

    if block_type == 2 {
        let first_count = if mixed_block { 17 } else { 18 };
        for value in values.iter_mut().take(first_count) {
            *value = read(ptr, data, &mut remaining, slen1)?;
        }
        for value in values.iter_mut().skip(first_count).take(18) {
            *value = read(ptr, data, &mut remaining, slen2)?;
        }
        // C fills the remaining scalefactor slots with zero.
    } else {
        let groups = [
            (0, 6, 0x8, slen1),
            (6, 11, 0x4, slen1),
            (11, 16, 0x2, slen2),
            (16, 21, 0x1, slen2),
        ];
        for (begin, end, reuse_flag, width) in groups {
            if scfsi & reuse_flag != 0 {
                let previous = granule_zero.ok_or(ScalefactorError::BadScfsi)?;
                values[begin..end].copy_from_slice(&previous[begin..end]);
            } else {
                for value in &mut values[begin..end] {
                    *value = read(ptr, data, &mut remaining, width)?;
                }
            }
        }
        // C explicitly sets long scalefactor 21 to zero.
    }

    Ok(DecodedScalefactors {
        values,
        intensity: None,
        preflag: false,
        part2_length: part2_length(start, *ptr),
    })
}

/// Decode MPEG-2/2.5 scalefactors for one channel and granule.
///
/// Set `intensity_stereo_right` for the right channel when intensity stereo
/// is active. In that case this returns the intensity positions that C writes
/// into the corresponding left channel's scalefactors.
pub fn decode_lsf(
    ptr: &mut BitPtr,
    data: &[u8],
    scalefac_compress: u16,
    block_type: u8,
    mixed_block: bool,
    intensity_stereo_right: bool,
    bits_left: usize,
) -> Result<DecodedScalefactors, ScalefactorError> {
    if scalefac_compress > 511 {
        return Err(ScalefactorError::InvalidCompress);
    }
    let start = *ptr;
    let mut remaining = bits_left;
    let index = if block_type == 2 {
        if mixed_block { 2 } else { 1 }
    } else {
        0
    };
    let mut values = [0; 39];
    let mut intensity = intensity_stereo_right.then_some([0; 39]);
    let mut preflag = false;

    let (table, slen) = if intensity_stereo_right {
        let compress = scalefac_compress >> 1;
        if compress < 180 {
            (
                3,
                [
                    (compress / 36) as u8,
                    ((compress % 36) / 6) as u8,
                    (compress % 6) as u8,
                    0,
                ],
            )
        } else if compress < 244 {
            let compress = compress - 180;
            (
                4,
                [
                    ((compress % 64) >> 4) as u8,
                    ((compress % 16) >> 2) as u8,
                    (compress % 4) as u8,
                    0,
                ],
            )
        } else {
            let compress = compress - 244;
            (5, [(compress / 3) as u8, (compress % 3) as u8, 0, 0])
        }
    } else if scalefac_compress < 400 {
        (
            0,
            [
                ((scalefac_compress >> 4) / 5) as u8,
                ((scalefac_compress >> 4) % 5) as u8,
                ((scalefac_compress % 16) >> 2) as u8,
                (scalefac_compress % 4) as u8,
            ],
        )
    } else if scalefac_compress < 500 {
        let compress = scalefac_compress - 400;
        (
            1,
            [
                ((compress >> 2) / 5) as u8,
                ((compress >> 2) % 5) as u8,
                (compress % 4) as u8,
                0,
            ],
        )
    } else {
        let compress = scalefac_compress - 500;
        preflag = true;
        (2, [(compress / 3) as u8, (compress % 3) as u8, 0, 0])
    };

    let counts = LSF_NSFB[table][index];
    let mut n = 0;
    for part in 0..4 {
        let max_value = ((1_u16 << slen[part]) - 1) as u8;
        for _ in 0..counts[part] {
            let value = read(ptr, data, &mut remaining, slen[part])?;
            values[n] = value;
            if let Some(intensity_values) = intensity.as_mut() {
                intensity_values[n] = u8::from(value == max_value);
            }
            n += 1;
        }
    }

    Ok(DecodedScalefactors {
        values,
        intensity,
        preflag,
        part2_length: part2_length(start, *ptr),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpeg1_long_block_reads_both_slen_groups() {
        let mut ptr = BitPtr::new(0);
        let decoded = decode_mpeg1(
            &mut ptr,
            &[0xff; 10],
            Mpeg1ScalefactorParams {
                scalefac_compress: 15,
                block_type: 0,
                mixed_block: false,
                scfsi: 0,
                granule_zero: None,
            },
            74,
        )
        .unwrap();
        assert!(decoded.values[..11].iter().all(|value| *value == 15));
        assert!(decoded.values[11..21].iter().all(|value| *value == 7));
        assert_eq!(decoded.values[21], 0);
        assert_eq!(decoded.part2_length, 74);
        assert_eq!(ptr.byte, 9);
        assert_eq!(ptr.left, 6);
    }

    #[test]
    fn mpeg1_scfsi_copies_the_selected_granule_zero_groups() {
        let mut previous = [0; 39];
        for (index, value) in previous.iter_mut().enumerate() {
            *value = index as u8;
        }
        let mut ptr = BitPtr::new(0);
        let decoded = decode_mpeg1(
            &mut ptr,
            &[],
            Mpeg1ScalefactorParams {
                scalefac_compress: 0,
                block_type: 0,
                mixed_block: false,
                scfsi: 0x0f,
                granule_zero: Some(&previous),
            },
            0,
        )
        .unwrap();
        assert_eq!(&decoded.values[..21], &previous[..21]);
        assert_eq!(decoded.part2_length, 0);
        assert_eq!(ptr, BitPtr::new(0));
    }

    #[test]
    fn mpeg1_budget_error_keeps_cursor_at_last_complete_value() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(
            decode_mpeg1(
                &mut ptr,
                &[0xff; 4],
                Mpeg1ScalefactorParams {
                    scalefac_compress: 15,
                    block_type: 0,
                    mixed_block: false,
                    scfsi: 0,
                    granule_zero: None,
                },
                10
            ),
            Err(ScalefactorError::BadScfsi)
        );
        assert_eq!(ptr.byte, 1);
        assert_eq!(ptr.left, 8);
    }

    #[test]
    fn lsf_long_block_and_intensity_values_match_zero_width_case() {
        let mut ptr = BitPtr::new(0);
        let decoded = decode_lsf(&mut ptr, &[], 0, 0, false, false, 0).unwrap();
        assert_eq!(decoded.values[21..], [0; 18]);
        assert_eq!(decoded.part2_length, 0);

        let intensity = decode_lsf(&mut ptr, &[], 0, 0, false, true, 0).unwrap();
        assert!(intensity.values[..21].iter().all(|value| *value == 0));
        assert!(
            intensity.intensity.unwrap()[..21]
                .iter()
                .all(|value| *value == 1)
        );
    }

    #[test]
    fn lsf_sets_preflag_for_compress_values_at_least_500() {
        let decoded = decode_lsf(&mut BitPtr::new(0), &[], 500, 0, false, false, 0).unwrap();
        assert!(decoded.preflag);
    }

    #[test]
    fn lsf_budget_and_buffer_errors_preserve_read_cursor() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(
            decode_lsf(&mut ptr, &[0xff], 16, 0, false, false, 2),
            Err(ScalefactorError::BadScfsi)
        );
        assert_eq!(ptr.byte, 0);
        assert_eq!(ptr.left, 6);

        let mut ptr = BitPtr::new(0);
        assert_eq!(
            decode_lsf(&mut ptr, &[], 16, 0, false, false, 5),
            Err(ScalefactorError::Truncated)
        );
        assert_eq!(ptr, BitPtr::new(0));
    }
}
