//! Read exFAT streams by extent or FAT chain.

use alloc::{boxed::Box, vec, vec::Vec};

use crate::Error;

use super::boot::Geometry;

const MAX_DIRECTORY_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Stream {
    pub(crate) first_cluster: u32,
    pub(crate) length: u64,
    pub(crate) valid_length: u64,
    pub(crate) contiguous: bool,
}

impl Stream {
    pub(crate) const fn new(
        first_cluster: u32,
        length: u64,
        valid_length: u64,
        contiguous: bool,
    ) -> Self {
        Self {
            first_cluster,
            length,
            valid_length,
            contiguous,
        }
    }
}

/// Keep one position in a FAT chain. Each stream needs its own cursor.
#[derive(Default)]
pub(crate) struct Cursor {
    position: Option<(u64, u32)>,
    fat_revision: u64,
}

/// Detect repeats in a chain without an extra FAT lookup or a visited set.
struct CycleCheck {
    checkpoint: u32,
    span: u64,
    steps: u64,
}

impl CycleCheck {
    fn new(first: u32) -> Self {
        Self {
            checkpoint: first,
            span: 1,
            steps: 0,
        }
    }

    fn follow(&mut self, next: u32) -> Result<(), Error> {
        self.steps += 1;
        if next == self.checkpoint {
            return Err(Error::Corrupt);
        }
        if self.steps == self.span {
            self.checkpoint = next;
            self.span *= 2;
            self.steps = 0;
        }
        Ok(())
    }
}

pub(crate) type WriteSector = Box<dyn FnMut(u64, &[u8]) -> Result<(), Error> + Send>;

pub(crate) struct Data<R, W = WriteSector> {
    base: u64,
    geometry: Geometry,
    read_sector: R,
    write_sector: Option<W>,
    sector: Vec<u8>,
    cached_lba: Option<u64>,
    fat_revision: u64,
}

impl<R: FnMut(u64, &mut [u8]) -> Result<(), Error>> Data<R, WriteSector> {
    pub(crate) fn new(base: u64, geometry: Geometry, sector_size: usize, read_sector: R) -> Self {
        Self {
            base,
            geometry,
            read_sector,
            write_sector: None,
            sector: vec![0; sector_size],
            cached_lba: None,
            fat_revision: 0,
        }
    }
}

impl<R, W> Data<R, W>
where
    R: FnMut(u64, &mut [u8]) -> Result<(), Error>,
    W: FnMut(u64, &[u8]) -> Result<(), Error>,
{
    pub(crate) fn new_writable(
        base: u64,
        geometry: Geometry,
        sector_size: usize,
        read_sector: R,
        write_sector: W,
    ) -> Self {
        Self {
            base,
            geometry,
            read_sector,
            write_sector: Some(write_sector),
            sector: vec![0; sector_size],
            cached_lba: None,
            fat_revision: 0,
        }
    }

    fn sector(&mut self, relative_lba: u64) -> Result<&[u8], Error> {
        if relative_lba >= self.geometry.volume_sectors {
            return Err(Error::Corrupt);
        }
        if self.cached_lba != Some(relative_lba) {
            let lba = self.base.checked_add(relative_lba).ok_or(Error::Corrupt)?;
            self.cached_lba = None;
            (self.read_sector)(lba, &mut self.sector)?;
            self.cached_lba = Some(relative_lba);
        }
        Ok(&self.sector)
    }

    pub(crate) fn write(
        &mut self,
        stream: Stream,
        cursor: &mut Cursor,
        offset: u64,
        input: &[u8],
    ) -> Result<usize, Error> {
        if self.write_sector.is_none() {
            return Err(Error::ReadOnly);
        }
        let cluster_bytes = self.cluster_bytes();
        let sector_bytes = self.sector.len();
        let end = offset
            .checked_add(input.len() as u64)
            .ok_or(Error::Corrupt)?;
        if end > stream.length {
            return Err(Error::NoSpace);
        }
        let mut copied = 0;
        while copied < input.len() {
            let position = offset + copied as u64;
            let cluster = self.cluster_at(stream, cursor, position / cluster_bytes)?;
            let cluster_sector = self
                .geometry
                .cluster_sector(cluster)
                .ok_or(Error::Corrupt)?;
            let within_cluster = position % cluster_bytes;
            let relative_lba = cluster_sector + within_cluster / sector_bytes as u64;
            let start = (within_cluster % sector_bytes as u64) as usize;
            let count = (input.len() - copied).min(sector_bytes - start);
            if start != 0 || count != sector_bytes {
                self.sector(relative_lba)?;
            }
            self.sector[start..start + count].copy_from_slice(&input[copied..copied + count]);
            let lba = self.base.checked_add(relative_lba).ok_or(Error::Corrupt)?;
            self.cached_lba = None;
            self.write_sector
                .as_mut()
                .expect("writable data has a sector callback")(lba, &self.sector)?;
            copied += count;
        }
        Ok(copied)
    }

    pub(crate) fn write_raw(
        &mut self,
        relative_lba: u64,
        offset: usize,
        input: &[u8],
    ) -> Result<(), Error> {
        if self.write_sector.is_none()
            || offset
                .checked_add(input.len())
                .is_none_or(|end| end > self.sector.len())
        {
            return Err(if self.write_sector.is_none() {
                Error::ReadOnly
            } else {
                Error::Corrupt
            });
        }
        if offset != 0 || input.len() != self.sector.len() {
            self.sector(relative_lba)?;
        }
        self.sector[offset..offset + input.len()].copy_from_slice(input);
        let lba = self.base.checked_add(relative_lba).ok_or(Error::Corrupt)?;
        self.cached_lba = None;
        self.write_sector
            .as_mut()
            .expect("writable data has a sector callback")(lba, &self.sector)
    }

    pub(crate) fn read_raw(
        &mut self,
        relative_lba: u64,
        offset: usize,
        out: &mut [u8],
    ) -> Result<(), Error> {
        if offset
            .checked_add(out.len())
            .is_none_or(|end| end > self.sector.len())
        {
            return Err(Error::Corrupt);
        }
        let sector = self.sector(relative_lba)?;
        out.copy_from_slice(&sector[offset..offset + out.len()]);
        Ok(())
    }

    pub(crate) fn geometry(&self) -> Geometry {
        self.geometry
    }

    pub(crate) fn sector_size(&self) -> usize {
        self.sector.len()
    }

    pub(crate) fn next_cluster(&mut self, cluster: u32) -> Result<Option<u32>, Error> {
        self.next(cluster)
    }

    pub(crate) fn set_fat_entry(&mut self, cluster: u32, value: u32) -> Result<(), Error> {
        if self.geometry.cluster_sector(cluster).is_none() {
            return Err(Error::Corrupt);
        }
        let byte = u64::from(cluster) * 4;
        let sector_size = self.sector.len() as u64;
        let lba = u64::from(self.geometry.fat_sector) + byte / sector_size;
        let offset = (byte % sector_size) as usize;
        // A failed callback may also have changed the FAT on disk.
        self.fat_revision = self.fat_revision.wrapping_add(1);
        self.write_raw(lba, offset, &value.to_le_bytes())
    }

    fn cluster_bytes(&self) -> u64 {
        self.sector.len() as u64 * u64::from(self.geometry.sectors_per_cluster)
    }

    fn next(&mut self, cluster: u32) -> Result<Option<u32>, Error> {
        self.geometry
            .cluster_sector(cluster)
            .ok_or(Error::Corrupt)?;
        let byte = u64::from(cluster) * 4;
        let sector_size = self.sector.len() as u64;
        let offset = (byte % sector_size) as usize;
        let lba = u64::from(self.geometry.fat_sector) + byte / sector_size;
        let sector = self.sector(lba)?;
        let next = u32::from_le_bytes(
            sector[offset..offset + 4]
                .try_into()
                .expect("FAT entry size"),
        );
        if next == u32::MAX {
            Ok(None)
        } else {
            self.geometry.cluster_sector(next).ok_or(Error::Corrupt)?;
            Ok(Some(next))
        }
    }

    pub(crate) fn root_stream(&mut self, first: u32) -> Result<Stream, Error> {
        let max_clusters = (MAX_DIRECTORY_BYTES / self.cluster_bytes())
            .min(u64::from(self.geometry.cluster_count));
        let mut cluster = first;
        let mut cycle = CycleCheck::new(first);
        for count in 1..=max_clusters {
            let Some(next) = self.next(cluster)? else {
                let length = count * self.cluster_bytes();
                return Ok(Stream::new(first, length, length, false));
            };
            cycle.follow(next)?;
            cluster = next;
        }
        Err(Error::Corrupt)
    }

    /// Check the range or chain. The callback checks allocation bits a run at a time.
    pub(crate) fn validate(
        &mut self,
        stream: Stream,
        mut allocated: impl FnMut(&mut Self, u32, u32) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let count = stream.length.div_ceil(self.cluster_bytes());
        if stream.valid_length > stream.length || count > u64::from(self.geometry.cluster_count) {
            return Err(Error::Corrupt);
        }
        if count == 0 {
            return if stream.first_cluster == 0 && !stream.contiguous {
                Ok(())
            } else {
                Err(Error::Corrupt)
            };
        }
        self.geometry
            .cluster_sector(stream.first_cluster)
            .ok_or(Error::Corrupt)?;
        if stream.contiguous {
            let last = stream
                .first_cluster
                .checked_add(count as u32 - 1)
                .ok_or(Error::Corrupt)?;
            self.geometry.cluster_sector(last).ok_or(Error::Corrupt)?;
            return allocated(self, stream.first_cluster, count as u32);
        }

        let mut cluster = stream.first_cluster;
        let mut cycle = CycleCheck::new(cluster);
        for index in 1..=count {
            allocated(self, cluster, 1)?;
            match self.next(cluster)? {
                None if index == count => return Ok(()),
                Some(next) if index < count => {
                    cycle.follow(next)?;
                    cluster = next;
                }
                _ => return Err(Error::Corrupt),
            }
        }
        Err(Error::Corrupt)
    }

    fn cluster_at(
        &mut self,
        stream: Stream,
        cursor: &mut Cursor,
        index: u64,
    ) -> Result<u32, Error> {
        if stream.contiguous {
            return stream
                .first_cluster
                .checked_add(u32::try_from(index).map_err(|_| Error::Corrupt)?)
                .ok_or(Error::Corrupt);
        }
        if cursor.fat_revision != self.fat_revision {
            cursor.position = None;
            cursor.fat_revision = self.fat_revision;
        }
        let (from, mut cluster) = cursor
            .position
            .filter(|&(i, _)| i <= index)
            .unwrap_or((0, stream.first_cluster));
        for _ in from..index {
            cluster = self.next(cluster)?.ok_or(Error::Corrupt)?;
        }
        cursor.position = Some((index, cluster));
        Ok(cluster)
    }

    /// A caller must validate the stream before exposing it to file operations.
    pub(crate) fn read(
        &mut self,
        stream: Stream,
        cursor: &mut Cursor,
        offset: u64,
        out: &mut [u8],
    ) -> Result<usize, Error> {
        let size = stream.length.saturating_sub(offset).min(out.len() as u64) as usize;
        let initialized = stream.valid_length.saturating_sub(offset).min(size as u64) as usize;
        let cluster_bytes = self.cluster_bytes();
        let sector_bytes = self.sector.len();
        let mut copied = 0;
        while copied < initialized {
            let position = offset + copied as u64;
            let cluster = self.cluster_at(stream, cursor, position / cluster_bytes)?;
            let cluster_sector = self
                .geometry
                .cluster_sector(cluster)
                .ok_or(Error::Corrupt)?;
            let within_cluster = position % cluster_bytes;
            let sector = self.sector(cluster_sector + within_cluster / sector_bytes as u64)?;
            let start = (within_cluster % sector_bytes as u64) as usize;
            let count = (initialized - copied).min(sector_bytes - start);
            out[copied..copied + count].copy_from_slice(&sector[start..start + count]);
            copied += count;
        }
        out[initialized..size].fill(0);
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> Geometry {
        Geometry {
            volume_sectors: 10_000_032,
            fat_sector: 24,
            heap_sector: 32,
            cluster_count: 10_000_000,
            sectors_per_cluster: 1,
            root_cluster: 2,
        }
    }

    #[test]
    fn test_read_stream_large_offset_and_uninitialized_tail() {
        let geometry = geometry();
        let mut reads = std::vec::Vec::new();
        let mut io = Data::new(0, geometry, 512, |lba, out: &mut [u8]| {
            reads.push(lba);
            out.fill(0x87);
            Ok(())
        });
        let boundary = (1u64 << 32) + 2;
        let stream = Stream::new(2, boundary + 20, boundary, true);
        io.validate(stream, |_, _, _| Ok(())).unwrap();
        let mut out = [0xa5; 8];
        assert_eq!(
            io.read(stream, &mut Cursor::default(), boundary - 4, &mut out),
            Ok(8)
        );
        assert_eq!(out, [0x87, 0x87, 0x87, 0x87, 0, 0, 0, 0]);
        assert_eq!(reads, [8_388_639, 8_388_640]);
    }

    #[test]
    fn test_read_stream_follows_fat_and_rejects_a_cycle() {
        let geometry = geometry();
        let mut io = Data::new(0, geometry, 512, |lba, out: &mut [u8]| {
            out.fill(0);
            match lba {
                24 => {
                    out[8..12].copy_from_slice(&5u32.to_le_bytes());
                    out[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
                    out[24..28].copy_from_slice(&7u32.to_le_bytes());
                    out[28..32].copy_from_slice(&6u32.to_le_bytes());
                }
                32 => out.fill(b'A'),
                35 => out.fill(b'B'),
                _ => return Err(Error::Io),
            }
            Ok(())
        });
        let stream = Stream::new(2, 1024, 1024, false);
        let mut visited = std::vec::Vec::new();
        io.validate(stream, |_, cluster, count| {
            assert_eq!(count, 1);
            visited.push(cluster);
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, [2, 5]);
        let mut cursor = Cursor::default();
        let mut out = [0; 4];
        assert_eq!(io.read(stream, &mut cursor, 510, &mut out), Ok(4));
        assert_eq!(&out, b"AABB");
        assert_eq!(io.read(stream, &mut cursor, 0, &mut out), Ok(4));
        assert_eq!(&out, b"AAAA");
        assert_eq!(io.root_stream(6), Err(Error::Corrupt));
        assert_eq!(
            io.validate(Stream::new(2, 512, 512, false), |_, _, _| Ok(())),
            Err(Error::Corrupt)
        );
    }

    #[test]
    fn test_write_stream_preserves_neighbor_bytes_and_persists() {
        use core::cell::RefCell;

        let geometry = Geometry {
            volume_sectors: 4096,
            fat_sector: 24,
            heap_sector: 32,
            cluster_count: 4040,
            sectors_per_cluster: 1,
            root_cluster: 2,
        };
        let sectors = RefCell::new(std::vec![0x5au8; 4096 * 512]);
        let mut io = Data::new_writable(
            0,
            geometry,
            512,
            |_, out: &mut [u8]| {
                out.fill(0x5a);
                Ok(())
            },
            |lba, bytes: &[u8]| {
                let start = lba as usize * 512;
                sectors.borrow_mut()[start..start + bytes.len()].copy_from_slice(bytes);
                Ok(())
            },
        );
        let stream = Stream::new(2, 512, 512, true);
        assert_eq!(io.write(stream, &mut Cursor::default(), 10, b"abc"), Ok(3));
        let persisted = sectors.borrow();
        assert_eq!(&persisted[32 * 512..32 * 512 + 10], &[0x5a; 10]);
        assert_eq!(&persisted[32 * 512 + 10..32 * 512 + 13], b"abc");
        assert_eq!(&persisted[32 * 512 + 13..33 * 512], &[0x5a; 499]);
    }

    #[test]
    fn test_write_stream_rejects_read_only_and_out_of_range_writes() {
        let geometry = geometry();
        let mut readonly = Data::new(0, geometry, 512, |_, out: &mut [u8]| {
            out.fill(0);
            Ok(())
        });
        let stream = Stream::new(2, 512, 512, true);
        assert_eq!(
            readonly.write(stream, &mut Cursor::default(), 0, b"x"),
            Err(Error::ReadOnly)
        );

        let mut writable = Data::new_writable(
            0,
            geometry,
            512,
            |_, out: &mut [u8]| {
                out.fill(0);
                Ok(())
            },
            |_, _| Ok(()),
        );
        assert_eq!(
            writable.write(stream, &mut Cursor::default(), 512, b"x"),
            Err(Error::NoSpace)
        );
    }

    #[test]
    fn test_write_stream_returns_device_failure() {
        let geometry = geometry();
        let mut io = Data::new_writable(
            0,
            geometry,
            512,
            |_, out: &mut [u8]| {
                out.fill(0);
                Ok(())
            },
            |_, _| Err(Error::Io),
        );
        assert_eq!(
            io.write(
                Stream::new(2, 512, 512, true),
                &mut Cursor::default(),
                0,
                &[1; 512]
            ),
            Err(Error::Io)
        );
    }
}
