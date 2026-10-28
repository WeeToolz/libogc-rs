//! Big-endian bit access matching libmad's `bit.c`.

const CRC_POLY: u16 = 0x8005;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitPtr {
    /// Index of the current byte in the input buffer.
    pub byte: usize,
    /// Cached current byte, as in `struct mad_bitptr`.
    pub cache: u16,
    /// Unconsumed bits in `cache`; 8 means the cursor is byte aligned.
    pub left: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitError {
    OutOfBounds,
    TooManyBits,
}

impl BitPtr {
    pub const fn new(byte: usize) -> Self {
        Self {
            byte,
            cache: 0,
            left: 8,
        }
    }

    pub fn bits_left(&self) -> u16 {
        self.left
    }

    pub fn next_byte(&self) -> usize {
        if self.left == 8 {
            self.byte
        } else {
            self.byte + 1
        }
    }

    pub fn bit_length(begin: Self, end: Self) -> isize {
        begin.left as isize
            + 8 * (end.byte as isize - begin.byte as isize - 1)
            + (8 - end.left) as isize
    }

    /// Return the available bits, or zero when the cursor is past `end`.
    pub(crate) fn available_bits(self, end: usize) -> usize {
        Self::bit_length(self, Self::new(end)).max(0) as usize
    }

    pub fn skip(&mut self, len: usize, data: &[u8]) -> Result<(), BitError> {
        self.byte = self
            .byte
            .checked_add(len / 8)
            .ok_or(BitError::OutOfBounds)?;
        let left = self.left as isize - (len % 8) as isize;
        if left < 0 {
            self.byte = self.byte.checked_add(1).ok_or(BitError::OutOfBounds)?;
            self.left = (left + 8) as u16;
        } else {
            self.left = left as u16;
        }
        if self.left < 8 {
            self.cache = *data.get(self.byte).ok_or(BitError::OutOfBounds)? as u16;
        }
        Ok(())
    }

    /// Read at most 32 bits. On an error, keep the cursor unchanged.
    pub fn read(&mut self, data: &[u8], len: u32) -> Result<u32, BitError> {
        if len == 0 {
            return Ok(0);
        }
        if len > 32 {
            return Err(BitError::TooManyBits);
        }
        let mut cursor = *self;
        if cursor.left == 8 {
            cursor.cache = *data.get(cursor.byte).ok_or(BitError::OutOfBounds)? as u16;
        }
        let mut remaining = len;
        let mut value;
        if remaining < cursor.left as u32 {
            let mask = (1_u32 << cursor.left) - 1;
            value = (cursor.cache as u32 & mask) >> (cursor.left as u32 - remaining);
            cursor.left -= remaining as u16;
            *self = cursor;
            return Ok(value);
        }
        let mask = (1_u32 << cursor.left) - 1;
        value = cursor.cache as u32 & mask;
        remaining -= cursor.left as u32;
        cursor.byte = cursor.byte.checked_add(1).ok_or(BitError::OutOfBounds)?;
        cursor.left = 8;
        while remaining >= 8 {
            value =
                value.wrapping_shl(8) | *data.get(cursor.byte).ok_or(BitError::OutOfBounds)? as u32;
            cursor.byte = cursor.byte.checked_add(1).ok_or(BitError::OutOfBounds)?;
            remaining -= 8;
        }
        if remaining > 0 {
            cursor.cache = *data.get(cursor.byte).ok_or(BitError::OutOfBounds)? as u16;
            value = value.wrapping_shl(remaining) | (cursor.cache as u32 >> (8 - remaining));
            cursor.left -= remaining as u16;
        }
        *self = cursor;
        Ok(value)
    }

    pub fn write(&mut self, data: &mut [u8], len: u32, value: u32) -> Result<(), BitError> {
        if len > 32 {
            return Err(BitError::TooManyBits);
        }
        let mut cursor = *self;
        let mut out = data.to_vec();
        for i in 0..len {
            if cursor.left == 8 {
                cursor.cache = *out.get(cursor.byte).ok_or(BitError::OutOfBounds)? as u16;
            }
            cursor.left -= 1;
            let bit = ((value >> (len - i - 1)) & 1) as u8;
            let mask = 1_u8 << cursor.left;
            let slot = out.get_mut(cursor.byte).ok_or(BitError::OutOfBounds)?;
            *slot = (*slot & !mask) | (bit * mask);
            cursor.cache = *slot as u16;
            if cursor.left == 0 {
                cursor.byte += 1;
                cursor.left = 8;
            }
        }
        data.copy_from_slice(&out);
        *self = cursor;
        Ok(())
    }
}

pub fn crc(mut ptr: BitPtr, data: &[u8], mut len: u32, init: u16) -> Result<u16, BitError> {
    let mut crc = init;
    while len >= 8 {
        let byte = ptr.read(data, 8)? as u8;
        crc = (crc << 8) ^ crc_table_entry(((crc >> 8) as u8) ^ byte);
        len -= 8;
    }
    while len != 0 {
        let msb = (ptr.read(data, 1)? as u16) ^ (crc >> 15);
        crc <<= 1;
        if msb & 1 != 0 {
            crc ^= CRC_POLY;
        }
        len -= 1;
    }
    Ok(crc)
}

const fn crc_table_entry(byte: u8) -> u16 {
    let mut c = (byte as u16) << 8;
    let mut i = 0;
    while i < 8 {
        c = if c & 0x8000 != 0 {
            (c << 1) ^ CRC_POLY
        } else {
            c << 1
        };
        i += 1;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cross_byte_boundaries_and_tracks_c_cursor() {
        let data = [0b1011_0101, 0b0110_1001];
        let mut ptr = BitPtr::new(0);
        assert_eq!(ptr.read(&data, 3), Ok(5));
        assert_eq!(
            ptr,
            BitPtr {
                byte: 0,
                cache: 0b1011_0101,
                left: 5
            }
        );
        assert_eq!(ptr.read(&data, 7), Ok(0b0101_0101));
        assert_eq!(
            ptr,
            BitPtr {
                byte: 1,
                cache: 0b0110_1001,
                left: 6
            }
        );
    }

    #[test]
    fn writes_and_crc_match_mpeg_bit_order() {
        let mut bytes = [0_u8; 2];
        let mut ptr = BitPtr::new(0);
        ptr.write(&mut bytes, 12, 0xabc).unwrap();
        assert_eq!(bytes, [0xab, 0xc0]);
        let mut read_ptr = BitPtr::new(0);
        assert_eq!(read_ptr.read(&bytes, 12), Ok(0xabc));
        assert_eq!(crc(BitPtr::new(0), &[0xff, 0xff], 16, 0xffff), Ok(0x0000));
    }

    #[test]
    fn bit_distances_preserve_partial_bytes_and_signed_results() {
        let mut ptr = BitPtr::new(0);
        ptr.read(&[0xff, 0xff], 3).unwrap();
        assert_eq!(ptr.available_bits(2), 13);
        assert_eq!(ptr.available_bits(0), 0);
        assert_eq!(BitPtr::bit_length(ptr, BitPtr::new(0)), -3);
        assert_eq!(BitPtr::new(2).available_bits(2), 0);
    }

    #[test]
    fn read_failure_does_not_advance_cursor() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(ptr.read(&[0xaa], 9), Err(BitError::OutOfBounds));
        assert_eq!(ptr, BitPtr::new(0));
    }
}
