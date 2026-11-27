//! Validate and interpret the allocation bitmap and the up-case table.

use alloc::vec::Vec;

use crate::Error;

const MAPPING_COUNT: usize = 1 << 16;
pub(crate) const MAX_UPCASE_BYTES: u64 = (4 * MAPPING_COUNT) as u64;

pub(crate) struct Upcase(Vec<u16>);

impl Upcase {
    pub(crate) fn parse(bytes: &[u8], expected: u32) -> Result<Self, Error> {
        if bytes.is_empty()
            || !bytes.len().is_multiple_of(2)
            || bytes.len() as u64 > MAX_UPCASE_BYTES
        {
            return Err(Error::Corrupt);
        }
        let checksum = bytes.iter().fold(0u32, |sum, &byte| {
            sum.rotate_right(1).wrapping_add(u32::from(byte))
        });
        if checksum != expected {
            return Err(Error::Corrupt);
        }

        let mut map = Vec::with_capacity(MAPPING_COUNT);
        let mut words = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes);
        while let Some(word) = words.next() {
            let index = map.len();
            if index >= MAPPING_COUNT {
                return Err(Error::Corrupt);
            }
            if word == u16::MAX && index != usize::from(u16::MAX) {
                let count = usize::from(words.next().ok_or(Error::Corrupt)?);
                if count == 0 || index + count > MAPPING_COUNT {
                    return Err(Error::Corrupt);
                }
                map.extend((index..index + count).map(|x| x as u16));
            } else {
                map.push(word);
            }
        }
        if map.len() != MAPPING_COUNT
            || (0..128u8).any(|n| map[usize::from(n)] != u16::from(n.to_ascii_uppercase()))
        {
            return Err(Error::Corrupt);
        }
        Ok(Self(map))
    }

    fn map(&self, unit: u16) -> u16 {
        self.0[usize::from(unit)]
    }

    pub(crate) fn matches(&self, first: &[u16], second: &[u16]) -> bool {
        first.len() == second.len()
            && first
                .iter()
                .zip(second)
                .all(|(&a, &b)| self.map(a) == self.map(b))
    }

    pub(crate) fn hash(&self, name: impl IntoIterator<Item = u16>) -> u16 {
        name.into_iter().fold(0u16, |sum, unit| {
            let [low, high] = self.map(unit).to_le_bytes();
            sum.rotate_right(1)
                .wrapping_add(u16::from(low))
                .rotate_right(1)
                .wrapping_add(u16::from(high))
        })
    }
}

/// Check a bit range within a byte buffer. Bit zero is the least significant one.
pub(crate) fn check_bits(bytes: &[u8], start: usize, count: usize) -> Result<(), Error> {
    let end = start.checked_add(count).ok_or(Error::Corrupt)?;
    if end > bytes.len() * 8 {
        return Err(Error::Corrupt);
    }
    let mut bit = start;
    while bit < end {
        let shift = bit % 8;
        let width = (end - bit).min(8 - shift);
        let mask = (((1u16 << width) - 1) << shift) as u8;
        if bytes[bit / 8] & mask != mask {
            return Err(Error::Corrupt);
        }
        bit += width;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upcase_decodes_compressed_table_and_checks_bytes() {
        let mut bytes = alloc::vec::Vec::new();
        for byte in 0u8..128 {
            bytes.extend_from_slice(&u16::from(byte.to_ascii_uppercase()).to_le_bytes());
        }
        bytes.extend_from_slice(&[0xff, 0xff, 0x80, 0xff]);
        let table = Upcase::parse(&bytes, 0xd88e_3a8c).unwrap();
        assert!(table.matches(&[97, 46, 116, 120, 116], &[65, 46, 84, 88, 84]));
        assert!(!table.matches(&[97, 46], &[65]));
        assert_eq!(table.hash([97, 46, 116, 120, 116]), 0x1cb8);
        assert_eq!(table.map(0xffff), 0xffff);
        bytes[1] = 3;
        assert!(matches!(
            Upcase::parse(&bytes, 0xd88e_3a8c),
            Err(Error::Corrupt)
        ));
    }

    #[test]
    fn test_allocation_bits_check_only_selected_clusters() {
        assert_eq!(check_bits(&[0xf0, 0x03], 4, 6), Ok(()));
        assert_eq!(check_bits(&[0xf0, 0x03], 4, 7), Err(Error::Corrupt));
        assert_eq!(check_bits(&[0xf0, 0x03], 3, 7), Err(Error::Corrupt));
    }
}
