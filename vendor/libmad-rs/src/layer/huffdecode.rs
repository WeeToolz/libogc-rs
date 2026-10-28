//! Layer III big-values and count1 decoding from `III_huffdecode()`.

use crate::{
    bit::BitPtr,
    fixed::Fixed,
    layer::{
        huffman::{self, HuffmanError},
        layer3::{ChannelSideInfo, FLAG_COUNT1TABLE_SELECT},
        requantize::{self, RequantError},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HuffDecodeError {
    BadPart3Length,
    BadBigValues,
    BadHuffmanTable,
    BadHuffmanData,
    InvalidBandWidths,
    Truncated,
}

/// Decode one Layer III channel's Huffman data and requantize it.
///
/// `ptr` points to the start of part3 data. On success or a decoding error
/// after its size is validated, it advances to the end of `part3_length`, as
/// `III_huffdecode()` advances the caller's cursor before its lookahead reads.
/// The coefficient result is zero-filled beyond the decoded big-values and
/// count1 symbols.
pub fn decode_channel(
    ptr: &mut BitPtr,
    data: &[u8],
    channel: &ChannelSideInfo,
    scalefactors: &[u8; 39],
    band_widths: &[u8],
    part3_length: u32,
) -> Result<[Fixed; 576], HuffDecodeError> {
    if part3_length > u32::from(channel.part2_3_length) {
        return Err(HuffDecodeError::BadPart3Length);
    }
    if channel.big_values > 288 {
        return Err(HuffDecodeError::BadBigValues);
    }
    let start = *ptr;
    if start.available_bits(data.len()) < part3_length as usize {
        return Err(HuffDecodeError::Truncated);
    }
    let mut part_end = start;
    part_end
        .skip(part3_length as usize, data)
        .map_err(|_| HuffDecodeError::Truncated)?;
    *ptr = part_end;

    let exponents =
        requantize::exponents(channel, scalefactors, band_widths).map_err(map_requant_error)?;
    let bands = BandMap::new(band_widths)?;
    let mut output = [0_i32; 576];
    let mut cursor = start;
    let mut position = 0_usize;
    let mut band = 0_usize;
    let mut region = 0_usize;
    let mut regions_left = usize::from(channel.region0_count) + 1;

    for _ in 0..usize::from(channel.big_values) {
        // III_huffdecode() stops big-values decoding when part3 is exhausted.
        // Remaining spectral lines are already zero-initialized.
        if bit_distance(start, cursor)? >= part3_length as usize {
            break;
        }
        advance_band(
            position,
            &bands,
            &mut band,
            &mut region,
            &mut regions_left,
            channel,
        );
        let table = *channel
            .table_select
            .get(region)
            .ok_or(HuffDecodeError::BadHuffmanTable)?;
        let mut next = cursor;
        let (x, y) = huffman::decode_pair(&mut next, data, table).map_err(map_huffman_error)?;
        if bit_distance(start, next)? > part3_length as usize {
            return Err(HuffDecodeError::BadHuffmanData);
        }
        if position + 1 >= output.len() {
            return Err(HuffDecodeError::BadBigValues);
        }
        let exponent = exponents[band.min(38)];
        output[position] = signed_requantize(x, exponent)?;
        output[position + 1] = signed_requantize(y, exponent)?;
        position += 2;
        cursor = next;
    }

    let table = usize::from(channel.flags & FLAG_COUNT1TABLE_SELECT);
    loop {
        let consumed = bit_distance(start, cursor)?;
        if consumed >= part3_length as usize || position > 572 {
            break;
        }
        let mut next = cursor;
        let values = match huffman::decode_quad(&mut next, data, table) {
            Ok(values) => values,
            // A truncated count1 lookahead is stuffing at the end of part3.
            // C exits the count1 loop and keeps the remaining lines at zero.
            Err(HuffmanError::Truncated) => break,
            Err(error) => return Err(map_huffman_error(error)),
        };
        let next_consumed = bit_distance(start, next)?;
        // C decodes count1 with guarded lookahead, then drops the last quad
        // when the code or its signs cross the part3 boundary.
        if next_consumed > part3_length as usize {
            break;
        }

        update_band_for_position(position, &bands, &mut band)?;
        let first_exp = exponents[band.min(38)];
        for i in 0..2 {
            output[position + i] = signed_requantize(i32::from(values[i]), first_exp)?;
        }
        position += 2;
        update_band_for_position(position, &bands, &mut band)?;
        let second_exp = exponents[band.min(38)];
        for i in 0..2 {
            output[position + i] = signed_requantize(i32::from(values[i + 2]), second_exp)?;
        }
        position += 2;
        cursor = next;
    }
    Ok(output)
}

fn signed_requantize(value: i32, exponent: i32) -> Result<Fixed, HuffDecodeError> {
    if value == 0 {
        return Ok(0);
    }
    let magnitude =
        requantize::requantize(value.unsigned_abs(), exponent).map_err(map_requant_error)?;
    Ok(if value < 0 {
        magnitude.wrapping_neg()
    } else {
        magnitude
    })
}

fn map_huffman_error(error: HuffmanError) -> HuffDecodeError {
    match error {
        HuffmanError::InvalidTable => HuffDecodeError::BadHuffmanTable,
        HuffmanError::InvalidTableData | HuffmanError::Truncated => HuffDecodeError::BadHuffmanData,
    }
}

fn map_requant_error(error: RequantError) -> HuffDecodeError {
    match error {
        RequantError::CoefficientOutOfRange => HuffDecodeError::BadHuffmanData,
        RequantError::InvalidBandWidths => HuffDecodeError::InvalidBandWidths,
    }
}

fn bit_distance(start: BitPtr, end: BitPtr) -> Result<usize, HuffDecodeError> {
    let distance = BitPtr::bit_length(start, end);
    usize::try_from(distance).map_err(|_| HuffDecodeError::BadHuffmanData)
}

struct BandMap {
    ends: [usize; 39],
    count: usize,
}

impl BandMap {
    fn new(widths: &[u8]) -> Result<Self, HuffDecodeError> {
        let mut map = Self {
            ends: [0; 39],
            count: 0,
        };
        let mut line = 0_usize;
        for &width in widths.iter().take(39) {
            let width = usize::from(width);
            if width == 0 || line + width > 576 {
                return Err(HuffDecodeError::InvalidBandWidths);
            }
            line += width;
            map.ends[map.count] = line;
            map.count += 1;
            if line == 576 {
                break;
            }
        }
        if line != 576 || map.count == 0 {
            return Err(HuffDecodeError::InvalidBandWidths);
        }
        Ok(map)
    }
}

fn update_band_for_position(
    position: usize,
    bands: &BandMap,
    band: &mut usize,
) -> Result<(), HuffDecodeError> {
    while *band + 1 < bands.count && position >= bands.ends[*band] {
        *band += 1;
    }
    if *band >= bands.count {
        return Err(HuffDecodeError::InvalidBandWidths);
    }
    Ok(())
}

fn advance_band(
    position: usize,
    bands: &BandMap,
    band: &mut usize,
    region: &mut usize,
    regions_left: &mut usize,
    channel: &ChannelSideInfo,
) {
    if *band + 1 < bands.count && position >= bands.ends[*band] {
        *band += 1;
        if *region < 2 {
            *regions_left -= 1;
            if *regions_left == 0 {
                *region += 1;
                *regions_left = if *region == 1 {
                    usize::from(channel.region1_count) + 1
                } else {
                    0
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed;

    fn long_widths() -> [u8; 22] {
        let mut widths = [28; 22];
        widths[0] = 2;
        widths[1] = 2;
        widths[2..14].fill(29);
        widths
    }

    fn base_channel() -> ChannelSideInfo {
        ChannelSideInfo {
            part2_3_length: 32,
            global_gain: 210,
            ..ChannelSideInfo::default()
        }
    }

    #[test]
    fn region_boundary_selects_next_pair_table_and_zero_fills_tail() {
        let mut channel = base_channel();
        channel.big_values = 2;
        channel.region0_count = 0;
        channel.table_select = [1, 3, 1];
        // Table 1's one-bit zero symbol, then table 3's `10` -> (0,1), sign 0.
        let data = [0b1100_0000];
        let mut ptr = BitPtr::new(0);
        let samples =
            decode_channel(&mut ptr, &data, &channel, &[0; 39], &long_widths(), 4).unwrap();
        assert_eq!(&samples[..4], &[0, 0, 0, fixed::ONE]);
        assert!(samples[4..].iter().all(|&sample| sample == 0));
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 4);
    }

    #[test]
    fn count1_quad_uses_selected_table_and_requantizes_nonzero_values() {
        let mut channel = base_channel();
        channel.flags = FLAG_COUNT1TABLE_SELECT;
        let mut ptr = BitPtr::new(0);
        let samples =
            decode_channel(&mut ptr, &[0], &channel, &[0; 39], &long_widths(), 8).unwrap();
        assert_eq!(&samples[..4], &[fixed::ONE; 4]);
        assert!(samples[4..].iter().all(|&sample| sample == 0));
    }

    #[test]
    fn rejects_part_lengths_big_values_and_reserved_tables() {
        let mut ptr = BitPtr::new(0);
        let mut channel = base_channel();
        assert_eq!(
            decode_channel(&mut ptr, &[0; 4], &channel, &[0; 39], &long_widths(), 33),
            Err(HuffDecodeError::BadPart3Length)
        );
        channel.big_values = 289;
        assert_eq!(
            decode_channel(&mut ptr, &[0; 4], &channel, &[0; 39], &long_widths(), 8),
            Err(HuffDecodeError::BadBigValues)
        );
        channel.big_values = 1;
        channel.table_select[0] = 4;
        assert_eq!(
            decode_channel(&mut ptr, &[0; 4], &channel, &[0; 39], &long_widths(), 8),
            Err(HuffDecodeError::BadHuffmanTable)
        );
    }

    #[test]
    fn exhausted_part3_leaves_remaining_big_values_zero() {
        let mut channel = base_channel();
        channel.big_values = 2;
        channel.table_select[0] = 1;
        let mut ptr = BitPtr::new(0);
        // One table-1 zero pair consumes one bit. There are no bits left for
        // the second big-values pair, which the C loop leaves zero-filled.
        let samples = decode_channel(
            &mut ptr,
            &[0b1000_0000],
            &channel,
            &[0; 39],
            &long_widths(),
            1,
        )
        .unwrap();
        assert_eq!(&samples[..4], &[0; 4]);
    }

    #[test]
    fn truncated_count1_stuffing_ends_successfully() {
        let mut channel = base_channel();
        channel.big_values = 0;
        let mut ptr = BitPtr::new(0);
        // Table 32 needs more than one bit. Its incomplete code at the
        // declared part3 boundary is stuffing, not a malformed frame.
        let samples =
            decode_channel(&mut ptr, &[0], &channel, &[0; 39], &long_widths(), 1).unwrap();
        assert!(samples.iter().all(|&sample| sample == 0));
    }
}
