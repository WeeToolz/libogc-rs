//! Mount the volume only after its required root metadata has passed validation.

use alloc::vec;

use crate::Error;

use super::{
    directory::{Entry, Record, next_record, valid_name},
    metadata::{MAX_UPCASE_BYTES, Upcase, check_bits},
    partition::locate,
    stream::{Cursor, Data, Stream},
};

pub(crate) struct Mounted<R> {
    pub(crate) data: Data<R>,
    pub(crate) root: Stream,
    pub(crate) upcase: Upcase,
    bitmap: Stream,
}

impl<R: FnMut(u64, &mut [u8]) -> Result<(), Error>> Mounted<R> {
    pub(crate) fn open(sectors: u64, sector_size: usize, mut read: R) -> Result<Self, Error> {
        if !matches!(sector_size, 512 | 1024 | 2048 | 4096) {
            return Err(Error::Unsupported);
        }
        let mut scratch = vec![0; sector_size];
        let (base, geometry) = locate(sectors, &mut scratch, &mut read)?;
        let mut data = Data::new(base, geometry, sector_size, read);
        let root = data.root_stream(geometry.root_cluster)?;
        let mut bitmap = None;
        let mut upcase = None;
        let mut directory = Directory::new(root);
        while let Some(record) = directory.next(&mut data)? {
            match record {
                Record::Bitmap(stream) if bitmap.is_none() => bitmap = Some(stream),
                Record::Upcase(stream, checksum) if upcase.is_none() => {
                    upcase = Some((stream, checksum));
                }
                Record::File(_) => {}
                _ => return Err(Error::Corrupt),
            }
        }
        let bitmap = bitmap.ok_or(Error::Corrupt)?;
        let (upcase, checksum) = upcase.ok_or(Error::Corrupt)?;
        if bitmap.length < u64::from(geometry.cluster_count).div_ceil(8)
            || upcase.length == 0
            || upcase.length > MAX_UPCASE_BYTES
        {
            return Err(Error::Corrupt);
        }
        // Verify the bitmap's chain before using the bitmap to check allocation.
        data.validate(bitmap, |_, _, _| Ok(()))?;
        for stream in [root, bitmap, upcase] {
            validate_allocated(&mut data, bitmap, stream)?;
        }
        let mut bytes = vec![0; upcase.length as usize];
        let size = data.read(upcase, &mut Cursor::default(), 0, &mut bytes)?;
        if size != bytes.len() {
            return Err(Error::Corrupt);
        }
        let upcase = Upcase::parse(&bytes, checksum)?;
        Ok(Self {
            data,
            root,
            upcase,
            bitmap,
        })
    }

    /// Resolve components relative to the volume root. Every matched stream is checked.
    pub(crate) fn lookup(&mut self, path: &str) -> Result<Entry, Error> {
        let mut entry = Entry {
            attributes: 0x10,
            name: vec![u16::from(b'/')],
            name_hash: 0,
            stream: self.root,
        };
        let mut at_root = true;
        for component in path.split('/').filter(|part| !part.is_empty()) {
            if !entry.directory() {
                return Err(Error::NotDirectory);
            }
            let mut name = [0u16; 255];
            let mut len = 0;
            for unit in component.encode_utf16() {
                *name.get_mut(len).ok_or(Error::InvalidPath)? = unit;
                len += 1;
            }
            let name = &name[..len];
            if !valid_name(name) {
                return Err(Error::InvalidPath);
            }
            let hash = self.upcase.hash(name.iter().copied());
            let mut walk = Directory::new(entry.stream);
            entry = loop {
                let candidate = self.next_file(&mut walk, at_root)?.ok_or(Error::NotFound)?;
                if candidate.name_hash == hash && self.upcase.matches(&candidate.name, name) {
                    break candidate;
                }
            };
            self.validate(entry.stream)?;
            at_root = false;
        }
        if path.ends_with('/') && !entry.directory() {
            return Err(Error::NotDirectory);
        }
        Ok(entry)
    }

    pub(crate) fn next_file(
        &mut self,
        walk: &mut Directory,
        at_root: bool,
    ) -> Result<Option<Entry>, Error> {
        while let Some(record) = walk.next(&mut self.data)? {
            match record {
                Record::File(entry) => {
                    if self.upcase.hash(entry.name.iter().copied()) != entry.name_hash {
                        return Err(Error::Corrupt);
                    }
                    return Ok(Some(entry));
                }
                _ if at_root => continue,
                _ => return Err(Error::Corrupt),
            }
        }
        Ok(None)
    }

    pub(crate) fn validate(&mut self, stream: Stream) -> Result<(), Error> {
        validate_allocated(&mut self.data, self.bitmap, stream)
    }
}

pub(crate) struct Directory {
    stream: Stream,
    offset: u64,
    cursor: Cursor,
}

impl Directory {
    pub(crate) fn new(stream: Stream) -> Self {
        Self {
            stream,
            offset: 0,
            cursor: Cursor::default(),
        }
    }

    pub(crate) fn next<R: FnMut(u64, &mut [u8]) -> Result<(), Error>>(
        &mut self,
        data: &mut Data<R>,
    ) -> Result<Option<Record>, Error> {
        let record = next_record(&mut || {
            let mut slot = [0; 32];
            let size = data.read(self.stream, &mut self.cursor, self.offset, &mut slot)?;
            if size == 0 {
                return Ok(None);
            }
            if size != slot.len() {
                return Err(Error::Corrupt);
            }
            self.offset += slot.len() as u64;
            Ok(Some(slot))
        })?;
        if record.is_none() {
            self.offset = self.stream.length;
        }
        Ok(record)
    }
}

fn validate_allocated<R: FnMut(u64, &mut [u8]) -> Result<(), Error>>(
    data: &mut Data<R>,
    bitmap: Stream,
    stream: Stream,
) -> Result<(), Error> {
    let mut cursor = Cursor::default();
    data.validate(stream, |data, first, count| {
        let mut bit = u64::from(first - 2);
        let end = bit + u64::from(count);
        let mut bytes = [0; 128];
        while bit < end {
            let offset = (bit % 8) as usize;
            let count = (end - bit).min((8 * bytes.len() - offset) as u64) as usize;
            let slice = &mut bytes[..(offset + count).div_ceil(8)];
            if data.read(bitmap, &mut cursor, bit / 8, slice)? != slice.len() {
                return Err(Error::Corrupt);
            }
            check_bits(slice, offset, count)?;
            bit += count as u64;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::super::boot::tests::boot_region;
    use super::*;

    #[test]
    fn test_mount_rejects_free_root_cluster() {
        let region = boot_region();
        let run = |bits| {
            Mounted::open(4096, 512, |lba, out: &mut [u8]| {
                out.fill(0);
                match lba {
                    0..=23 => out.copy_from_slice(&region[(lba % 12) as usize]),
                    24 => out[8..20].fill(0xff),
                    56 => {
                        out[0] = 0x81;
                        out[20] = 3;
                        out[24..26].copy_from_slice(&505u16.to_le_bytes());
                        out[32] = 0x82;
                        out[36..40].copy_from_slice(&0xd88e_3a8cu32.to_le_bytes());
                        out[52] = 4;
                        out[56..58].copy_from_slice(&260u16.to_le_bytes());
                        out[64..70].copy_from_slice(&[0x85, 2, 0x13, 0x35, 0x20, 0]);
                        out[96..102].copy_from_slice(&[0xc0, 3, 0, 5, 0xb8, 0x1c]);
                        out[104] = 12;
                        out[116] = 5;
                        out[120] = 16;
                        out[128..140].copy_from_slice(&[
                            0xc1, 0, b'a', 0, b'.', 0, b't', 0, b'x', 0, b't', 0,
                        ]);
                    }
                    57 => out[0] = bits,
                    58 => {
                        for b in 0u8..128 {
                            let i = usize::from(b) * 2;
                            out[i] = b.to_ascii_uppercase();
                        }
                        out[256..260].copy_from_slice(&[0xff, 0xff, 0x80, 0xff]);
                    }
                    _ => return Err(Error::Io),
                }
                Ok(())
            })
            .and_then(|mut mount| {
                let file = mount.lookup("/A.TXT")?;
                assert_eq!(file.stream, Stream::new(5, 16, 12, true));
                assert_eq!(mount.lookup("missing"), Err(Error::NotFound));
                assert_eq!(mount.lookup("/a.txt/child"), Err(Error::NotDirectory));
                assert_eq!(mount.lookup("/../a.txt"), Err(Error::InvalidPath));
                Ok(())
            })
        };
        assert_eq!(run(0b1111), Ok(()));
        assert_eq!(run(0b1110), Err(Error::Corrupt));
    }
}
