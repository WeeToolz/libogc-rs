//! Directory entry sets and root metadata.

use alloc::{vec, vec::Vec};

use crate::Error;

use super::stream::Stream;

type Slot = [u8; 32];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub(crate) attributes: u16,
    pub(crate) name: Vec<u16>,
    pub(crate) name_hash: u16,
    pub(crate) stream: Stream,
    pub(crate) parent: Stream,
    pub(crate) entry_offset: u64,
    pub(crate) secondary_count: u8,
}

impl Entry {
    pub(crate) fn readonly(&self) -> bool {
        self.attributes & 1 != 0
    }

    pub(crate) fn directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Record {
    File(Entry),
    Bitmap(Stream),
    Upcase(Stream, u32),
}

pub(crate) fn next_record(
    read: &mut impl FnMut() -> Result<Option<Slot>, Error>,
) -> Result<Option<Record>, Error> {
    while let Some(entry) = read()? {
        let record = match entry[0] {
            0 => return Ok(None),
            1..=0x7f => continue,
            0x81 => {
                if entry[1] & 1 != 0 {
                    return Err(Error::Corrupt);
                }
                Record::Bitmap(metadata_stream(&entry))
            }
            0x82 => Record::Upcase(metadata_stream(&entry), le32(&entry, 4)),
            0x83 if entry[1] <= 11 => continue,
            0x85 => Record::File(file_entry(entry, read)?),
            0xa0..=0xbf => {
                for _ in 0..entry[1] {
                    if required(read)?[0] & 0xc0 != 0xc0 {
                        return Err(Error::Corrupt);
                    }
                }
                continue;
            }
            0x84..=0x9f => return Err(Error::Unsupported),
            _ => return Err(Error::Corrupt),
        };
        return Ok(Some(record));
    }
    Ok(None)
}

fn file_entry(
    primary: Slot,
    read: &mut impl FnMut() -> Result<Option<Slot>, Error>,
) -> Result<Entry, Error> {
    let count = usize::from(primary[1]);
    if count < 2 {
        return Err(Error::Corrupt);
    }
    let expected = le16(&primary, 2);
    let mut checksum = sum(sum(0, &primary[..2]), &primary[4..]);
    let stream_slot = required(read)?;
    if stream_slot[0] != 0xc0 || stream_slot[1] & 1 == 0 {
        return Err(Error::Corrupt);
    }
    checksum = sum(checksum, &stream_slot);
    let name_length = usize::from(stream_slot[3]);
    let name_slots = name_length.div_ceil(15);
    if name_length == 0 || name_slots + 1 > count {
        return Err(Error::Corrupt);
    }
    let mut name = Vec::with_capacity(name_length);
    for index in 0..count - 1 {
        let slot = required(read)?;
        if index < name_slots {
            if slot[0] != 0xc1 {
                return Err(Error::Corrupt);
            }
            let remaining = (name_length - name.len()).min(15);
            for offset in 0..remaining {
                name.push(le16(&slot, 2 + offset * 2));
            }
        } else if slot[0] & 0xe0 != 0xe0 {
            return Err(Error::Corrupt);
        }
        checksum = sum(checksum, &slot);
    }

    let length = le64(&stream_slot, 24);
    let valid_length = le64(&stream_slot, 8);
    let first_cluster = le32(&stream_slot, 20);
    let contiguous = stream_slot[1] & 2 != 0;
    let attributes = le16(&primary, 4);
    let entry = Entry {
        attributes,
        name,
        name_hash: le16(&stream_slot, 4),
        stream: Stream::new(first_cluster, length, valid_length, contiguous),
        parent: Stream::new(0, 0, 0, false),
        entry_offset: 0,
        secondary_count: primary[1],
    };
    if expected != checksum
        || valid_length > length
        || (length == 0 && (first_cluster != 0 || contiguous))
        || (length != 0 && first_cluster < 2)
        || (entry.directory() && (valid_length != length || length > 256 * 1024 * 1024))
        || !valid_name(&entry.name)
    {
        return Err(Error::Corrupt);
    }
    Ok(entry)
}

pub(crate) fn valid_name(name: &[u16]) -> bool {
    name != [46]
        && name != [46, 46]
        && !name
            .iter()
            .any(|&unit| matches!(unit, 0..=31 | 34 | 42 | 47 | 58 | 60 | 62 | 63 | 92 | 124))
        && char::decode_utf16(name.iter().copied()).all(|ch| ch.is_ok())
}

pub(crate) fn update_file_set(record: &mut [u8], stream: Stream) -> Result<(), Error> {
    if record.len() < 96 || record.len() != (usize::from(record[1]) + 1) * 32 {
        return Err(Error::Corrupt);
    }
    let stream_entry = &mut record[32..64];
    stream_entry[1] = (stream_entry[1] & !2) | if stream.contiguous { 2 } else { 0 };
    stream_entry[8..16].copy_from_slice(&stream.valid_length.to_le_bytes());
    stream_entry[20..24].copy_from_slice(&stream.first_cluster.to_le_bytes());
    stream_entry[24..32].copy_from_slice(&stream.length.to_le_bytes());
    let checksum = record[..2]
        .iter()
        .chain(&record[4..])
        .fold(0u16, |sum, &byte| {
            sum.rotate_right(1).wrapping_add(u16::from(byte))
        });
    record[2..4].copy_from_slice(&checksum.to_le_bytes());
    Ok(())
}

pub(crate) fn new_file_set(name: &[u16], name_hash: u16) -> Result<Vec<u8>, Error> {
    if name.is_empty() || name.len() > 255 || !valid_name(name) {
        return Err(Error::InvalidPath);
    }
    let name_slots = name.len().div_ceil(15);
    let secondary_count = 1 + name_slots;
    let mut record = vec![0; (secondary_count + 1) * 32];
    record[..2].copy_from_slice(&[0x85, secondary_count as u8]);
    record[4..6].copy_from_slice(&0x20u16.to_le_bytes());
    record[32..36].copy_from_slice(&[0xc0, 1, 0, name.len() as u8]);
    record[36..38].copy_from_slice(&name_hash.to_le_bytes());
    for (index, chunk) in name.chunks(15).enumerate() {
        let start = (index + 2) * 32;
        record[start] = 0xc1;
        for (unit_index, unit) in chunk.iter().enumerate() {
            let at = start + 2 + unit_index * 2;
            record[at..at + 2].copy_from_slice(&unit.to_le_bytes());
        }
    }
    let checksum = record[..2]
        .iter()
        .chain(&record[4..])
        .fold(0u16, |sum, &byte| {
            sum.rotate_right(1).wrapping_add(u16::from(byte))
        });
    record[2..4].copy_from_slice(&checksum.to_le_bytes());
    Ok(record)
}

fn metadata_stream(slot: &Slot) -> Stream {
    let length = le64(slot, 24);
    Stream::new(le32(slot, 20), length, length, false)
}

fn required(read: &mut impl FnMut() -> Result<Option<Slot>, Error>) -> Result<Slot, Error> {
    read()?.ok_or(Error::Corrupt)
}

fn sum(initial: u16, bytes: &[u8]) -> u16 {
    bytes.iter().fold(initial, |sum, &byte| {
        sum.rotate_right(1).wrapping_add(u16::from(byte))
    })
}

fn le16(slot: &Slot, offset: usize) -> u16 {
    u16::from_le_bytes(
        slot[offset..offset + 2]
            .try_into()
            .expect("entry field size"),
    )
}

fn le32(slot: &Slot, offset: usize) -> u32 {
    u32::from_le_bytes(
        slot[offset..offset + 4]
            .try_into()
            .expect("entry field size"),
    )
}

fn le64(slot: &Slot, offset: usize) -> u64 {
    u64::from_le_bytes(
        slot[offset..offset + 8]
            .try_into()
            .expect("entry field size"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_set_checks_checksum_and_stream_fields() {
        let mut primary = [0; 32];
        primary[..6].copy_from_slice(&[0x85, 2, 0xb3, 0x34, 0x20, 0]);
        let mut stream = [0; 32];
        stream[..6].copy_from_slice(&[0xc0, 3, 0, 5, 0xb8, 0x1c]);
        stream[8] = 12;
        stream[20] = 2;
        stream[24] = 16;
        let mut name = [0; 32];
        name[..12].copy_from_slice(&[0xc1, 0, b'a', 0, b'.', 0, b't', 0, b'x', 0, b't', 0]);
        let scan = |name| {
            let mut slots = [primary, stream, name].into_iter();
            next_record(&mut || Ok(slots.next()))
        };
        let Some(Record::File(entry)) = scan(name).unwrap() else {
            panic!("missing file");
        };
        assert_eq!(entry.name, [97, 46, 116, 120, 116]);
        assert_eq!(entry.name_hash, 0x1cb8);
        assert_eq!(entry.stream, Stream::new(2, 16, 12, true));
        assert!(!entry.directory());

        name[2] = b'b';
        assert_eq!(scan(name), Err(Error::Corrupt));
    }

    #[test]
    fn test_root_metadata_descriptor() {
        let mut bitmap = [0; 32];
        bitmap[0] = 0x81;
        bitmap[20] = 3;
        bitmap[24] = 32;
        let mut upcase = [0; 32];
        upcase[0] = 0x82;
        upcase[4..8].copy_from_slice(&[1, 2, 3, 4]);
        upcase[20] = 7;
        upcase[24] = 12;
        let mut slots = [bitmap, upcase, [0; 32]].into_iter();
        let mut read = || Ok(slots.next());
        assert_eq!(
            next_record(&mut read),
            Ok(Some(Record::Bitmap(Stream::new(3, 32, 32, false))))
        );
        assert_eq!(
            next_record(&mut read),
            Ok(Some(Record::Upcase(
                Stream::new(7, 12, 12, false),
                0x0403_0201
            )))
        );
        assert_eq!(next_record(&mut read), Ok(None));
    }

    #[test]
    fn test_new_file_set_encodes_name_and_valid_checksum() {
        let record = new_file_set(&[97, 46, 116, 120, 116], 0x1cb8).unwrap();
        assert_eq!(record.len(), 96);
        assert_eq!(&record[..2], &[0x85, 2]);
        assert_eq!(&record[32..38], &[0xc0, 1, 0, 5, 0xb8, 0x1c]);
        let mut slots = record.chunks_exact(32).map(|slot| {
            let mut result = [0; 32];
            result.copy_from_slice(slot);
            result
        });
        let Some(Record::File(entry)) = next_record(&mut || Ok(slots.next())).unwrap() else {
            panic!("new file set was not parsed");
        };
        assert_eq!(entry.name, [97, 46, 116, 120, 116]);
        assert_eq!(entry.stream, Stream::new(0, 0, 0, false));
    }
}
