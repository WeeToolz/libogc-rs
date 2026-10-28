//! Layer III Huffman symbol decoding from `huffman.c`.

use crate::{
    bit::{BitError, BitPtr},
    tables::huffman::{self, Entry},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HuffmanError {
    InvalidTable,
    InvalidTableData,
    Truncated,
}

fn peek(ptr: BitPtr, data: &[u8], bits: u8) -> Result<u32, HuffmanError> {
    let available = ptr.available_bits(data.len());
    let take = usize::from(bits).min(available) as u32;
    let mut cursor = ptr;
    let value = cursor
        .read(data, take)
        .map_err(|_| HuffmanError::Truncated)?;
    Ok(value << (u32::from(bits) - take))
}

fn decode_symbol(
    ptr: &mut BitPtr,
    data: &[u8],
    entries: &[Entry],
    startbits: u8,
) -> Result<Entry, HuffmanError> {
    let mut base = 0_usize;
    let mut width = startbits;
    loop {
        let index = base
            .checked_add(peek(*ptr, data, width)? as usize)
            .ok_or(HuffmanError::InvalidTableData)?;
        let entry = *entries.get(index).ok_or(HuffmanError::InvalidTableData)?;
        match entry {
            Entry::Pointer { offset, bits } => {
                if ptr.available_bits(data.len()) < usize::from(width) {
                    return Err(HuffmanError::Truncated);
                }
                ptr.skip(usize::from(width), data)
                    .map_err(|_| HuffmanError::Truncated)?;
                base = usize::from(offset);
                width = bits;
            }
            Entry::Pair { len, .. } | Entry::Quad { len, .. } => {
                if ptr.available_bits(data.len()) < usize::from(len) {
                    return Err(HuffmanError::Truncated);
                }
                ptr.skip(usize::from(len), data)
                    .map_err(|_| HuffmanError::Truncated)?;
                return Ok(entry);
            }
        }
    }
}

fn read_sign(ptr: &mut BitPtr, data: &[u8]) -> Result<bool, HuffmanError> {
    ptr.read(data, 1)
        .map(|bit| bit != 0)
        .map_err(|error| match error {
            BitError::OutOfBounds | BitError::TooManyBits => HuffmanError::Truncated,
        })
}

fn signed(value: i32, negative: bool) -> i32 {
    if negative { -value } else { value }
}

/// Decode one big-values pair using a Layer III pair table.
///
/// The returned values include linbits extensions and sign bits. Tables 4
/// and 14 are reserved and return [`HuffmanError::InvalidTable`].
pub fn decode_pair(ptr: &mut BitPtr, data: &[u8], table: u8) -> Result<(i32, i32), HuffmanError> {
    let config = huffman::PAIR_TABLES
        .get(usize::from(table))
        .and_then(|table| *table)
        .ok_or(HuffmanError::InvalidTable)?;
    if table == 0 {
        return Ok((0, 0));
    }
    let Entry::Pair { x, y, .. } = decode_symbol(ptr, data, config.entries, config.startbits)?
    else {
        return Err(HuffmanError::InvalidTableData);
    };
    // C decodes each coefficient completely before it reads the next one:
    // x linbits, x sign, then y linbits, y sign. Keep this order because
    // linbits and sign bits share the same bitstream.
    let x = extend(ptr, data, x, config.linbits)?;
    let x = if x == 0 {
        0
    } else {
        signed(x, read_sign(ptr, data)?)
    };
    let y = extend(ptr, data, y, config.linbits)?;
    let y = if y == 0 {
        0
    } else {
        signed(y, read_sign(ptr, data)?)
    };
    Ok((x, y))
}

fn extend(ptr: &mut BitPtr, data: &[u8], value: u8, linbits: u8) -> Result<i32, HuffmanError> {
    if value == 15 && linbits != 0 {
        let extra = ptr
            .read(data, u32::from(linbits))
            .map_err(|_| HuffmanError::Truncated)?;
        Ok(15 + extra as i32)
    } else {
        Ok(i32::from(value))
    }
}

/// Decode one count1 quadruple using table A (`0`) or B (`1`), including
/// sign bits for each nonzero coefficient.
pub fn decode_quad(ptr: &mut BitPtr, data: &[u8], table: usize) -> Result<[i8; 4], HuffmanError> {
    let entries = *huffman::QUAD_TABLES
        .get(table)
        .ok_or(HuffmanError::InvalidTable)?;
    let Entry::Quad { values, .. } = decode_symbol(ptr, data, entries, 4)? else {
        return Err(HuffmanError::InvalidTableData);
    };
    let mut result = [0_i8; 4];
    for (result, value) in result.iter_mut().zip(values) {
        if value != 0 {
            *result = if read_sign(ptr, data)? {
                -(value as i8)
            } else {
                value as i8
            };
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_one_pair_reads_code_then_x_and_y_sign_bits() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(decode_pair(&mut ptr, &[0b0000_1000], 1), Ok((1, -1)));
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 5);
    }

    #[test]
    fn pair_zero_code_consumes_one_bit_and_reserved_tables_fail() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(decode_pair(&mut ptr, &[0b1000_0000], 1), Ok((0, 0)));
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 1);
        assert_eq!(
            decode_pair(&mut ptr, &[0], 4),
            Err(HuffmanError::InvalidTable)
        );
    }

    #[test]
    fn quad_table_b_decodes_each_value_sign_in_order() {
        let mut ptr = BitPtr::new(0);
        // Code 0000 is (1,1,1,1); sign bits 0,1,0,1 follow it.
        assert_eq!(decode_quad(&mut ptr, &[0b0000_0101], 1), Ok([1, -1, 1, -1]));
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 8);
    }

    #[test]
    fn double_linbits_escape_reads_x_sign_before_y_extension() {
        // Table 24 code 0011 is (15, 15). C reads x extension 8, x sign,
        // y extension 8, then y sign. Reordering these bits changes y.
        let mut ptr = BitPtr::new(0);
        assert_eq!(
            decode_pair(&mut ptr, &[0b0011_1000, 0b1100_0100], 24),
            Ok((-23, -23))
        );
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 14);
    }

    #[test]
    fn pair_linbits_escape_adds_extension_before_sign() {
        // Table 16 reaches (15, 0) through 0000 0110 0. Its linbits
        // extension is one, followed by the x sign bit.
        let mut ptr = BitPtr::new(0);
        assert_eq!(
            decode_pair(&mut ptr, &[0b0000_0110, 0b0100_0000], 16),
            Ok((16, 0))
        );
        assert_eq!(BitPtr::bit_length(BitPtr::new(0), ptr), 11);
    }
}
