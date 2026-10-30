//! Checked binary parsing and writing helpers shared by the tools.

use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoundsError {
    pub offset: usize,
    pub size: usize,
    pub length: usize,
}

impl fmt::Display for BoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "range {}..{} exceeds input length {}",
            self.offset,
            self.offset.saturating_add(self.size),
            self.length
        )
    }
}

impl std::error::Error for BoundsError {}

pub fn checked_range(
    length: usize,
    offset: usize,
    size: usize,
) -> Result<std::ops::Range<usize>, BoundsError> {
    let end = offset.checked_add(size).ok_or(BoundsError {
        offset,
        size,
        length,
    })?;
    if end > length {
        return Err(BoundsError {
            offset,
            size,
            length,
        });
    }
    Ok(offset..end)
}

pub fn read_u16_be(bytes: &[u8], offset: usize) -> Result<u16, BoundsError> {
    let range = checked_range(bytes.len(), offset, 2)?;
    Ok(u16::from_be_bytes([
        bytes[range.start],
        bytes[range.start + 1],
    ]))
}

pub fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, BoundsError> {
    let range = checked_range(bytes.len(), offset, 4)?;
    Ok(u32::from_be_bytes(
        bytes[range].try_into().expect("checked four-byte range"),
    ))
}

pub fn write_u16_be(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), BoundsError> {
    let range = checked_range(bytes.len(), offset, 2)?;
    bytes[range].copy_from_slice(&value.to_be_bytes());
    Ok(())
}

pub fn write_u32_be(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), BoundsError> {
    let range = checked_range(bytes.len(), offset, 4)?;
    bytes[range].copy_from_slice(&value.to_be_bytes());
    Ok(())
}

pub fn align_up(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return None;
    }
    value.checked_next_multiple_of(alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_big_endian_words() {
        let mut bytes = [0; 8];
        write_u32_be(&mut bytes, 2, 0x1234_abcd).unwrap();
        assert_eq!(read_u32_be(&bytes, 2).unwrap(), 0x1234_abcd);
        assert!(read_u16_be(&bytes, 7).is_err());
    }

    #[test]
    fn binary_access_checks_bytes_and_bounds() {
        let mut bytes = [0xaa; 8];
        write_u16_be(&mut bytes, 1, 0x1234).unwrap();
        write_u32_be(&mut bytes, 3, 0x89ab_cdef).unwrap();
        assert_eq!(bytes, [0xaa, 0x12, 0x34, 0x89, 0xab, 0xcd, 0xef, 0xaa]);
        assert_eq!(read_u16_be(&bytes, 1), Ok(0x1234));
        assert_eq!(read_u32_be(&bytes, 3), Ok(0x89ab_cdef));
        let error = BoundsError {
            offset: usize::MAX,
            size: 4,
            length: 8,
        };
        assert_eq!(read_u32_be(&bytes, usize::MAX), Err(error.clone()));
        assert_eq!(write_u32_be(&mut bytes, usize::MAX, 0), Err(error));
        assert_eq!(bytes, [0xaa, 0x12, 0x34, 0x89, 0xab, 0xcd, 0xef, 0xaa]);
        assert_eq!(checked_range(8, 8, 0), Ok(8..8));
        assert!(write_u16_be(&mut bytes, 7, 0).is_err());
        assert_eq!(bytes[7], 0xaa);
    }

    #[test]
    fn aligns_without_overflow() {
        assert_eq!(align_up(0x121, 32), Some(0x140));
        assert_eq!(align_up(usize::MAX, 32), None);
        assert_eq!(align_up(2, 3), None);
        assert_eq!(align_up(1, 0), None);
        assert_eq!(align_up(0, 32), Some(0));
        assert_eq!(align_up(32, 32), Some(32));
        assert_eq!(align_up(usize::MAX, 1), Some(usize::MAX));
        assert_eq!(align_up(usize::MAX - 31, 32), Some(usize::MAX - 31));
    }
}
