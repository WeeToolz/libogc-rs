//! Mount the volume only after its required root metadata has passed validation.

use alloc::{vec, vec::Vec};

use crate::Error;

use super::{
    directory::{Entry, Record, new_file_set, next_record, valid_name},
    metadata::{MAX_UPCASE_BYTES, Upcase, check_bits},
    partition::locate,
    stream::{Cursor, Data, Stream, WriteSector},
};

pub(crate) struct Mounted<R, W = WriteSector> {
    pub(crate) data: Data<R, W>,
    pub(crate) root: Stream,
    pub(crate) upcase: Upcase,
    bitmap: Stream,
    allocation_hint: u32,
    pub(crate) write_blocked: bool,
}

impl<R: FnMut(u64, &mut [u8]) -> Result<(), Error>> Mounted<R, WriteSector> {
    pub(crate) fn open(sectors: u64, sector_size: usize, mut read: R) -> Result<Self, Error> {
        if !matches!(sector_size, 512 | 1024 | 2048 | 4096) {
            return Err(Error::Unsupported);
        }
        let mut scratch = vec![0; sector_size];
        let (base, geometry) = locate(sectors, &mut scratch, &mut read)?;
        let data = Data::new(base, geometry, sector_size, read);
        Self::finish_open(data, geometry)
    }
}

impl<R, W> Mounted<R, W>
where
    R: FnMut(u64, &mut [u8]) -> Result<(), Error>,
    W: FnMut(u64, &[u8]) -> Result<(), Error>,
{
    pub(crate) fn open_writable(
        sectors: u64,
        sector_size: usize,
        mut read: R,
        write: W,
    ) -> Result<Self, Error> {
        if !matches!(sector_size, 512 | 1024 | 2048 | 4096) {
            return Err(Error::Unsupported);
        }
        let mut scratch = vec![0; sector_size];
        let (base, geometry) = locate(sectors, &mut scratch, &mut read)?;
        let mut data = Data::new_writable(base, geometry, sector_size, read, write);
        // The backup boot sector contains stale volume flags.
        let mut flags = [0; 2];
        data.read_raw(0, 106, &mut flags)?;
        if u16::from_le_bytes(flags) & 2 != 0 {
            return Err(Error::Dirty);
        }
        Self::finish_open(data, geometry)
    }

    fn finish_open(mut data: Data<R, W>, geometry: super::boot::Geometry) -> Result<Self, Error> {
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
            allocation_hint: 2,
            write_blocked: false,
        })
    }

    /// Resolve components relative to the volume root. Every matched stream is checked.
    pub(crate) fn lookup(&mut self, path: &str) -> Result<Entry, Error> {
        let mut entry = Entry {
            attributes: 0x10,
            name: vec![u16::from(b'/')],
            name_hash: 0,
            stream: self.root,
            parent: self.root,
            entry_offset: 0,
            secondary_count: 0,
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
                let mut candidate = self.next_file(&mut walk, at_root)?.ok_or(Error::NotFound)?;
                if candidate.name_hash == hash && self.upcase.matches(&candidate.name, name) {
                    candidate.parent = entry.stream;
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
                Record::File(mut entry) => {
                    if self.upcase.hash(entry.name.iter().copied()) != entry.name_hash {
                        return Err(Error::Corrupt);
                    }
                    entry.parent = walk.stream;
                    let entry_bytes = u64::from(entry.secondary_count + 1) * 32;
                    entry.entry_offset =
                        walk.offset.checked_sub(entry_bytes).ok_or(Error::Corrupt)?;
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

    pub(crate) fn update_entry(&mut self, entry: &Entry, stream: Stream) -> Result<(), Error> {
        let size = usize::from(entry.secondary_count + 1) * 32;
        let mut record = vec![0; size];
        if self.data.read(
            entry.parent,
            &mut Cursor::default(),
            entry.entry_offset,
            &mut record,
        )? != size
        {
            return Err(Error::Corrupt);
        }
        super::directory::update_file_set(&mut record, stream)?;
        if self.data.write(
            entry.parent,
            &mut Cursor::default(),
            entry.entry_offset,
            &record,
        )? != size
        {
            return Err(Error::Io);
        }
        Ok(())
    }

    pub(crate) fn refresh_entry(&mut self, entry: &mut Entry) -> Result<(), Error> {
        let size = usize::from(entry.secondary_count + 1) * 32;
        let mut bytes = vec![0; size];
        if self.data.read(
            entry.parent,
            &mut Cursor::default(),
            entry.entry_offset,
            &mut bytes,
        )? != size
        {
            return Err(Error::Corrupt);
        }
        let mut slots = bytes.chunks_exact(32).map(|chunk| {
            let mut slot = [0; 32];
            slot.copy_from_slice(chunk);
            slot
        });
        let Some(Record::File(current)) = next_record(&mut || Ok(slots.next()))? else {
            return Err(Error::InvalidHandle);
        };
        if current.name != entry.name || current.name_hash != entry.name_hash {
            return Err(Error::InvalidHandle);
        }
        self.validate(current.stream)?;
        entry.stream = current.stream;
        entry.attributes = current.attributes;
        Ok(())
    }

    pub(crate) fn create_file(&mut self, path: &str) -> Result<Entry, Error> {
        if path.ends_with('/') || path.is_empty() {
            return Err(Error::InvalidPath);
        }
        match self.lookup(path) {
            Ok(_) => return Err(Error::AlreadyExists),
            Err(Error::NotFound) => {}
            Err(error) => return Err(error),
        }
        let (parent_path, leaf) = path.rsplit_once('/').unwrap_or(("", path));
        let parent_path = if parent_path.is_empty() {
            "/"
        } else {
            parent_path
        };
        let parent = self.lookup(parent_path)?;
        if !parent.directory() {
            return Err(Error::NotDirectory);
        }
        let name: Vec<u16> = leaf.encode_utf16().collect();
        if !valid_name(&name) || name.len() > 255 {
            return Err(Error::InvalidPath);
        }
        let name_hash = self.upcase.hash(name.iter().copied());
        let mut record = new_file_set(&name, name_hash)?;
        super::directory::update_file_set(&mut record, Stream::new(0, 0, 0, false))?;
        let offset = self.find_end_slot(parent.stream, record.len())?;
        if self.write_blocked {
            return Err(Error::Dirty);
        }
        self.write_blocked = true;
        self.set_volume_dirty(true)?;
        if self
            .data
            .write(parent.stream, &mut Cursor::default(), offset, &record)?
            != record.len()
        {
            return Err(Error::Io);
        }
        let end_offset = offset + record.len() as u64;
        if self
            .data
            .write(parent.stream, &mut Cursor::default(), end_offset, &[0; 32])?
            != 32
        {
            return Err(Error::Io);
        }
        self.set_volume_dirty(false)?;
        self.write_blocked = false;
        Ok(Entry {
            attributes: 0x20,
            name,
            name_hash,
            stream: Stream::new(0, 0, 0, false),
            parent: parent.stream,
            entry_offset: offset,
            secondary_count: record[1],
        })
    }

    fn find_end_slot(&mut self, directory: Stream, entry_set_bytes: usize) -> Result<u64, Error> {
        let required = entry_set_bytes.checked_add(32).ok_or(Error::NoSpace)? as u64;
        let mut offset = 0u64;
        let mut slot = [0; 32];
        while offset
            .checked_add(required)
            .is_some_and(|end| end <= directory.length)
        {
            if self
                .data
                .read(directory, &mut Cursor::default(), offset, &mut slot)?
                != slot.len()
            {
                return Err(Error::Corrupt);
            }
            if slot[0] == 0 {
                return Ok(offset);
            }
            offset += 32;
        }
        Err(Error::NoSpace)
    }

    pub(crate) fn write_file(
        &mut self,
        entry: &mut Entry,
        cursor: &mut Cursor,
        offset: u64,
        input: &[u8],
    ) -> Result<usize, Error> {
        if input.is_empty() {
            return Ok(0);
        }
        if self.write_blocked {
            return Err(Error::Dirty);
        }
        let end = offset
            .checked_add(input.len() as u64)
            .ok_or(Error::NoSpace)?;
        self.refresh_entry(entry)?;
        if entry.readonly() {
            return Err(Error::ReadOnly);
        }
        let old = entry.stream;
        let length = old.length.max(end);
        let cluster_bytes =
            u64::from(self.data.geometry().sectors_per_cluster) * self.data.sector_size() as u64;
        let required = length.div_ceil(cluster_bytes);
        self.check_free_clusters(required - old.length.div_ceil(cluster_bytes))?;
        self.write_blocked = true;
        self.set_volume_dirty(true)?;
        let mut updated = self.ensure_capacity(old, required)?;
        updated.length = length;
        if offset > old.valid_length {
            let mut zero_offset = old.valid_length;
            let zeros = [0; 512];
            while zero_offset < offset {
                let count = (offset - zero_offset).min(zeros.len() as u64) as usize;
                self.data
                    .write(updated, cursor, zero_offset, &zeros[..count])?;
                zero_offset += count as u64;
            }
        }
        let written = self.data.write(updated, cursor, offset, input)?;
        if written != input.len() {
            return Err(Error::Io);
        }
        updated.valid_length = old.valid_length.max(end);
        if updated != old {
            self.update_entry(entry, updated)?;
            entry.stream = updated;
        }
        self.set_volume_dirty(false)?;
        self.write_blocked = false;
        Ok(written)
    }

    pub(crate) fn truncate_file(
        &mut self,
        entry: &mut Entry,
        new_length: u64,
    ) -> Result<(), Error> {
        self.refresh_entry(entry)?;
        if entry.readonly() {
            return Err(Error::ReadOnly);
        }
        let old = entry.stream;
        if new_length == old.length {
            return Ok(());
        }
        if self.write_blocked {
            return Err(Error::Dirty);
        }
        let cluster_bytes =
            u64::from(self.data.geometry().sectors_per_cluster) * self.data.sector_size() as u64;
        let old_count = old.length.div_ceil(cluster_bytes);
        let new_count = new_length.div_ceil(cluster_bytes);
        self.check_free_clusters(new_count.saturating_sub(old_count))?;
        self.write_blocked = true;
        self.set_volume_dirty(true)?;
        let mut stream = if new_length > old.length {
            let mut stream = self.ensure_capacity(old, new_count)?;
            stream.length = new_length;
            let mut offset = old.valid_length;
            let zeros = [0; 512];
            while offset < new_length {
                let count = (new_length - offset).min(zeros.len() as u64) as usize;
                self.data
                    .write(stream, &mut Cursor::default(), offset, &zeros[..count])?;
                offset += count as u64;
            }
            stream.valid_length = new_length;
            stream
        } else {
            if new_count < old_count {
                self.release_clusters(old, new_count, old_count)?;
            }
            if new_length == 0 {
                Stream::new(0, 0, 0, false)
            } else {
                Stream::new(
                    old.first_cluster,
                    new_length,
                    old.valid_length.min(new_length),
                    old.contiguous,
                )
            }
        };
        if new_length != 0 {
            stream.valid_length = stream.valid_length.min(new_length);
        }
        self.update_entry(entry, stream)?;
        entry.stream = stream;
        self.set_volume_dirty(false)?;
        self.write_blocked = false;
        Ok(())
    }

    fn release_clusters(
        &mut self,
        stream: Stream,
        keep_count: u64,
        old_count: u64,
    ) -> Result<(), Error> {
        if stream.contiguous {
            let first_free = u64::from(stream.first_cluster)
                .checked_add(keep_count)
                .ok_or(Error::Corrupt)?;
            let end = u64::from(stream.first_cluster)
                .checked_add(old_count)
                .ok_or(Error::Corrupt)?;
            for cluster in first_free..end {
                self.set_allocated(u32::try_from(cluster).map_err(|_| Error::Corrupt)?, false)?;
            }
            return Ok(());
        }
        let mut cluster = stream.first_cluster;
        if keep_count > 0 {
            for _ in 1..keep_count {
                cluster = self.data.next_cluster(cluster)?.ok_or(Error::Corrupt)?;
            }
            let first_free = self.data.next_cluster(cluster)?.ok_or(Error::Corrupt)?;
            self.data.set_fat_entry(cluster, u32::MAX)?;
            cluster = first_free;
        }
        for index in keep_count..old_count {
            let next = self.data.next_cluster(cluster)?;
            self.set_allocated(cluster, false)?;
            self.data.set_fat_entry(cluster, 0)?;
            if let Some(next) = next {
                cluster = next;
            } else if index + 1 != old_count {
                return Err(Error::Corrupt);
            }
        }
        Ok(())
    }

    fn ensure_capacity(&mut self, stream: Stream, required: u64) -> Result<Stream, Error> {
        let cluster_bytes =
            u64::from(self.data.geometry().sectors_per_cluster) * self.data.sector_size() as u64;
        let current = stream.length.div_ceil(cluster_bytes);
        if required <= current {
            return Ok(stream);
        }
        let additional = required - current;
        if stream.contiguous && self.can_extend_contiguous(stream, current, additional)? {
            let start = if current == 0 {
                self.allocation_hint.max(2)
            } else {
                stream
                    .first_cluster
                    .checked_add(u32::try_from(current).map_err(|_| Error::NoSpace)?)
                    .ok_or(Error::NoSpace)?
            };
            let first = if current == 0 {
                start
            } else {
                stream.first_cluster
            };
            for cluster in start..start + u32::try_from(additional).map_err(|_| Error::NoSpace)? {
                self.set_allocated(cluster, true)?;
            }
            return Ok(Stream::new(first, stream.length, stream.valid_length, true));
        }

        let mut previous = if current == 0 {
            None
        } else if stream.contiguous {
            let last = stream
                .first_cluster
                .checked_add(u32::try_from(current - 1).map_err(|_| Error::Corrupt)?)
                .ok_or(Error::Corrupt)?;
            for cluster in stream.first_cluster..last {
                self.data.set_fat_entry(cluster, cluster + 1)?;
            }
            Some(last)
        } else {
            let mut cluster = stream.first_cluster;
            for _ in 1..current {
                cluster = self.data.next_cluster(cluster)?.ok_or(Error::Corrupt)?;
            }
            Some(cluster)
        };

        let mut first = stream.first_cluster;
        for _ in 0..additional {
            let cluster = self.allocate_cluster()?;
            if let Some(last) = previous {
                self.data.set_fat_entry(last, cluster)?;
            } else {
                first = cluster;
            }
            self.data.set_fat_entry(cluster, u32::MAX)?;
            previous = Some(cluster);
        }
        Ok(Stream::new(
            first,
            stream.length,
            stream.valid_length,
            false,
        ))
    }

    fn can_extend_contiguous(
        &mut self,
        stream: Stream,
        current: u64,
        additional: u64,
    ) -> Result<bool, Error> {
        let start = if current == 0 {
            self.allocation_hint
        } else {
            match stream
                .first_cluster
                .checked_add(u32::try_from(current).map_err(|_| Error::NoSpace)?)
            {
                Some(cluster) => cluster,
                None => return Ok(false),
            }
        };
        let end = u64::from(start) + additional;
        if end > u64::from(self.data.geometry().cluster_count) + 2 {
            return Ok(false);
        }
        for cluster in start..u32::try_from(end).map_err(|_| Error::NoSpace)? {
            if self.is_allocated(cluster)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn check_free_clusters(&mut self, mut needed: u64) -> Result<(), Error> {
        let count = u64::from(self.data.geometry().cluster_count);
        if needed > count {
            return Err(Error::NoSpace);
        }
        let mut offset = 0;
        let mut cursor = Cursor::default();
        let mut bytes = [0; 512];
        while needed != 0 && offset < count.div_ceil(8) {
            let size = (count.div_ceil(8) - offset).min(bytes.len() as u64) as usize;
            if self
                .data
                .read(self.bitmap, &mut cursor, offset, &mut bytes[..size])?
                != size
            {
                return Err(Error::Corrupt);
            }
            for (index, &value) in bytes[..size].iter().enumerate() {
                let remaining_bits = count - (offset + index as u64) * 8;
                let mask = u8::MAX >> (8 - remaining_bits.min(8));
                needed = needed.saturating_sub(u64::from((!value & mask).count_ones()));
                if needed == 0 {
                    return Ok(());
                }
            }
            offset += size as u64;
        }
        if needed == 0 {
            Ok(())
        } else {
            Err(Error::NoSpace)
        }
    }

    fn is_allocated(&mut self, cluster: u32) -> Result<bool, Error> {
        let bit = u64::from(cluster.checked_sub(2).ok_or(Error::Corrupt)?);
        let mut value = [0];
        if self
            .data
            .read(self.bitmap, &mut Cursor::default(), bit / 8, &mut value)?
            != 1
        {
            return Err(Error::Corrupt);
        }
        Ok(value[0] & (1 << (bit % 8)) != 0)
    }

    fn set_allocated(&mut self, cluster: u32, allocated: bool) -> Result<(), Error> {
        let bit = u64::from(cluster.checked_sub(2).ok_or(Error::Corrupt)?);
        let mut value = [0];
        self.data
            .read(self.bitmap, &mut Cursor::default(), bit / 8, &mut value)?;
        let mask = 1 << (bit % 8);
        if allocated {
            value[0] |= mask;
        } else {
            value[0] &= !mask;
        }
        self.data
            .write(self.bitmap, &mut Cursor::default(), bit / 8, &value)?;
        Ok(())
    }

    fn find_free_cluster(&mut self) -> Result<u32, Error> {
        let count = self.data.geometry().cluster_count;
        let start = self.allocation_hint.max(2).min(count + 1);
        for cluster in start..count + 2 {
            if !self.is_allocated(cluster)? {
                self.set_allocated(cluster, true)?;
                self.allocation_hint = cluster.saturating_add(1).max(2);
                return Ok(cluster);
            }
        }
        for cluster in 2..start {
            if !self.is_allocated(cluster)? {
                self.set_allocated(cluster, true)?;
                self.allocation_hint = cluster.saturating_add(1).max(2);
                return Ok(cluster);
            }
        }
        Err(Error::NoSpace)
    }

    fn allocate_cluster(&mut self) -> Result<u32, Error> {
        let cluster = self.find_free_cluster()?;
        let cluster_sector = self
            .data
            .geometry()
            .cluster_sector(cluster)
            .ok_or(Error::Corrupt)?;
        let zero = alloc::vec![0; self.data.sector_size()];
        for sector in 0..self.data.geometry().sectors_per_cluster {
            self.data
                .write_raw(cluster_sector + u64::from(sector), 0, &zero)?;
        }
        Ok(cluster)
    }

    fn set_volume_dirty(&mut self, dirty: bool) -> Result<(), Error> {
        let mut flags = [0; 2];
        self.data.read_raw(0, 106, &mut flags)?;
        let value = u16::from_le_bytes(flags);
        let updated = if dirty { value | 2 } else { value & !2 };
        self.data.write_raw(0, 106, &updated.to_le_bytes())
    }
}

pub(crate) struct Directory {
    pub(crate) stream: Stream,
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

    pub(crate) fn next<R, W>(&mut self, data: &mut Data<R, W>) -> Result<Option<Record>, Error>
    where
        R: FnMut(u64, &mut [u8]) -> Result<(), Error>,
        W: FnMut(u64, &[u8]) -> Result<(), Error>,
    {
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

fn validate_allocated<R, W>(
    data: &mut Data<R, W>,
    bitmap: Stream,
    stream: Stream,
) -> Result<(), Error>
where
    R: FnMut(u64, &mut [u8]) -> Result<(), Error>,
    W: FnMut(u64, &[u8]) -> Result<(), Error>,
{
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
    use std::{cell::RefCell, rc::Rc, vec::Vec};

    fn writable_image() -> Rc<RefCell<Vec<[u8; 512]>>> {
        let mut image = vec![[0; 512]; 4096];
        let boot = boot_region();
        image[..12].copy_from_slice(&boot);
        image[12..24].copy_from_slice(&boot);
        image[24][8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        image[24][12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        image[24][16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        image[24][20..24].copy_from_slice(&u32::MAX.to_le_bytes());

        let root = &mut image[56];
        root[0] = 0x81;
        root[20..24].copy_from_slice(&3u32.to_le_bytes());
        root[24..32].copy_from_slice(&505u64.to_le_bytes());
        root[32] = 0x82;
        root[36..40].copy_from_slice(&0xd88e_3a8cu32.to_le_bytes());
        root[52..56].copy_from_slice(&4u32.to_le_bytes());
        root[56..64].copy_from_slice(&260u64.to_le_bytes());
        root[64..70].copy_from_slice(&[0x85, 2, 0x13, 0x35, 0x20, 0]);
        root[96..102].copy_from_slice(&[0xc0, 3, 0, 5, 0xb8, 0x1c]);
        root[104..112].copy_from_slice(&12u64.to_le_bytes());
        root[116..120].copy_from_slice(&5u32.to_le_bytes());
        root[120..128].copy_from_slice(&16u64.to_le_bytes());
        root[128..140].copy_from_slice(&[0xc1, 0, b'a', 0, b'.', 0, b't', 0, b'x', 0, b't', 0]);

        image[57][0] = 0b0000_1111;
        for byte in 0u8..128 {
            let offset = usize::from(byte) * 2;
            image[58][offset..offset + 2]
                .copy_from_slice(&u16::from(byte.to_ascii_uppercase()).to_le_bytes());
        }
        image[58][256..260].copy_from_slice(&[0xff, 0xff, 0x80, 0xff]);
        image[59][..12].copy_from_slice(b"hello world!");
        Rc::new(RefCell::new(image))
    }

    fn callbacks(
        image: Rc<RefCell<Vec<[u8; 512]>>>,
        fail_sector: Option<u64>,
    ) -> (
        impl FnMut(u64, &mut [u8]) -> Result<(), Error>,
        impl FnMut(u64, &[u8]) -> Result<(), Error>,
    ) {
        let read_image = image.clone();
        let write_image = image;
        (
            move |lba, out: &mut [u8]| {
                out.copy_from_slice(&read_image.borrow()[lba as usize]);
                Ok(())
            },
            move |lba, data: &[u8]| {
                if fail_sector == Some(lba) {
                    return Err(Error::Io);
                }
                write_image.borrow_mut()[lba as usize].copy_from_slice(data);
                Ok(())
            },
        )
    }

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

    #[test]
    fn test_writable_mount_ignores_stale_backup_dirty_flag() {
        let image = writable_image();
        image.borrow_mut()[12][106] |= 2;
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut file = mounted.lookup("a.txt").unwrap();
        mounted
            .write_file(&mut file, &mut Cursor::default(), 0, b"x")
            .unwrap();
        assert_eq!(image.borrow()[12][106] & 2, 2);
    }

    #[test]
    fn test_growth_without_space_does_not_modify_image() {
        let image = writable_image();
        image.borrow_mut()[57].fill(0xff);
        image.borrow_mut()[57][1] &= !1;
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut file = mounted.lookup("a.txt").unwrap();
        let before = image.borrow().clone();
        assert_eq!(
            mounted.write_file(&mut file, &mut Cursor::default(), 1024, b"x"),
            Err(Error::NoSpace)
        );
        assert!(*image.borrow() == before);
        assert_eq!(mounted.truncate_file(&mut file, 1025), Err(Error::NoSpace));
        assert!(*image.borrow() == before);
        assert_eq!(
            mounted.write_file(&mut file, &mut Cursor::default(), 0, b"x"),
            Ok(1)
        );
    }

    #[test]
    fn test_readonly_file_rejects_write_and_truncate() {
        let image = writable_image();
        {
            let mut image = image.borrow_mut();
            let record = &mut image[56][64..160];
            record[4] |= 1;
            super::super::directory::update_file_set(record, Stream::new(5, 16, 12, true)).unwrap();
        }
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut file = mounted.lookup("a.txt").unwrap();
        let before = image.borrow().clone();
        assert_eq!(
            mounted.write_file(&mut file, &mut Cursor::default(), 0, b"x"),
            Err(Error::ReadOnly)
        );
        assert_eq!(mounted.truncate_file(&mut file, 0), Err(Error::ReadOnly));
        assert!(*image.borrow() == before);
    }

    #[test]
    fn test_fat_cursor_follows_reallocated_chain() {
        let image = writable_image();
        image.borrow_mut()[57][0] |= 1 << 4;
        let (read, write) = callbacks(image, None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut file = mounted.lookup("a.txt").unwrap();
        let mut stale = mounted.lookup("a.txt").unwrap();
        let mut cursor = Cursor::default();
        let mut byte = [0];
        mounted
            .write_file(&mut file, &mut Cursor::default(), 512, b"a")
            .unwrap();
        mounted.refresh_entry(&mut stale).unwrap();
        mounted
            .data
            .read(stale.stream, &mut cursor, 512, &mut byte)
            .unwrap();
        assert_eq!(&byte, b"a");
        mounted.truncate_file(&mut file, 0).unwrap();
        let mut other = mounted.create_file("other.txt").unwrap();
        mounted
            .write_file(&mut other, &mut Cursor::default(), 512, b"b")
            .unwrap();
        mounted
            .write_file(&mut file, &mut Cursor::default(), 512, b"c")
            .unwrap();
        mounted.refresh_entry(&mut stale).unwrap();
        mounted
            .data
            .read(stale.stream, &mut cursor, 512, &mut byte)
            .unwrap();
        assert_eq!(&byte, b"c");
    }

    #[test]
    fn test_write_grows_contiguous_file_and_survives_remount() {
        let image = writable_image();
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut entry = mounted.lookup("/a.txt").unwrap();
        let mut second_handle_entry = mounted.lookup("/a.txt").unwrap();
        assert_eq!(entry.entry_offset, 64);
        assert_eq!(
            mounted.write_file(&mut entry, &mut Cursor::default(), 514, b"XYZ"),
            Ok(3)
        );
        assert_eq!(entry.stream, Stream::new(5, 517, 517, true));
        assert_eq!(
            mounted.write_file(&mut second_handle_entry, &mut Cursor::default(), 517, b"!"),
            Ok(1)
        );
        assert_eq!(second_handle_entry.stream, Stream::new(5, 518, 518, true));
        assert_ne!(image.borrow()[57][0] & 0b0001_0000, 0);
        assert_eq!(
            u16::from_le_bytes(image.borrow()[0][106..108].try_into().unwrap()) & 2,
            0
        );
        drop(mounted);

        let (read, _) = callbacks(image.clone(), None);
        let mut reopened = Mounted::open(4096, 512, read).unwrap();
        let entry = reopened.lookup("/a.txt").unwrap();
        assert_eq!(entry.stream, Stream::new(5, 518, 518, true));
        let mut bytes = vec![0; 518];
        assert_eq!(
            reopened
                .data
                .read(entry.stream, &mut Cursor::default(), 0, &mut bytes),
            Ok(518)
        );
        assert_eq!(&bytes[..12], b"hello world!");
        assert!(bytes[12..514].iter().all(|&byte| byte == 0));
        assert_eq!(&bytes[514..], b"XYZ!");
    }

    #[test]
    fn test_write_failure_leaves_volume_dirty_and_blocks_more_writes() {
        let image = writable_image();
        let (read, write) = callbacks(image.clone(), Some(60));
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut entry = mounted.lookup("/a.txt").unwrap();
        assert_eq!(
            mounted.write_file(&mut entry, &mut Cursor::default(), 514, b"XYZ"),
            Err(Error::Io)
        );
        assert_ne!(
            u16::from_le_bytes(image.borrow()[0][106..108].try_into().unwrap()) & 2,
            0
        );
        assert_eq!(
            mounted.write_file(&mut entry, &mut Cursor::default(), 0, b"x"),
            Err(Error::Dirty)
        );
        drop(mounted);
        let (read, write) = callbacks(image, None);
        assert!(matches!(
            Mounted::open_writable(4096, 512, read, write),
            Err(Error::Dirty)
        ));
    }

    #[test]
    fn test_write_growth_converts_contiguous_file_to_fragmented_chain() {
        let image = writable_image();
        image.borrow_mut()[57][0] |= 1 << 4;
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut entry = mounted.lookup("/a.txt").unwrap();
        assert_eq!(
            mounted.write_file(&mut entry, &mut Cursor::default(), 514, b"XYZ"),
            Ok(3)
        );
        assert_eq!(entry.stream, Stream::new(5, 517, 517, false));
        assert_eq!(
            u32::from_le_bytes(image.borrow()[24][20..24].try_into().unwrap()),
            7
        );
        assert_eq!(
            u32::from_le_bytes(image.borrow()[24][28..32].try_into().unwrap()),
            u32::MAX
        );
        drop(mounted);

        let (read, _) = callbacks(image, None);
        let mut reopened = Mounted::open(4096, 512, read).unwrap();
        let entry = reopened.lookup("/a.txt").unwrap();
        assert_eq!(entry.stream, Stream::new(5, 517, 517, false));
        let mut bytes = vec![0; 517];
        assert_eq!(
            reopened
                .data
                .read(entry.stream, &mut Cursor::default(), 0, &mut bytes),
            Ok(517)
        );
        assert_eq!(&bytes[..12], b"hello world!");
        assert!(bytes[12..514].iter().all(|&byte| byte == 0));
        assert_eq!(&bytes[514..], b"XYZ");
    }

    #[test]
    fn test_create_write_and_remount_new_file() {
        let image = writable_image();
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut entry = mounted.create_file("/new.txt").unwrap();
        assert_eq!(entry.entry_offset, 160);
        assert_eq!(
            mounted.write_file(&mut entry, &mut Cursor::default(), 0, b"new file"),
            Ok(8)
        );
        assert_eq!(mounted.create_file("new.txt"), Err(Error::AlreadyExists));
        drop(mounted);

        let (read, _) = callbacks(image, None);
        let mut reopened = Mounted::open(4096, 512, read).unwrap();
        let entry = reopened.lookup("/new.txt").unwrap();
        assert_eq!(entry.stream.length, 8);
        let mut bytes = [0; 8];
        assert_eq!(
            reopened
                .data
                .read(entry.stream, &mut Cursor::default(), 0, &mut bytes),
            Ok(8)
        );
        assert_eq!(&bytes, b"new file");
    }

    #[test]
    fn test_truncate_frees_clusters_and_persists_empty_file() {
        let image = writable_image();
        let (read, write) = callbacks(image.clone(), None);
        let mut mounted = Mounted::open_writable(4096, 512, read, write).unwrap();
        let mut entry = mounted.lookup("/a.txt").unwrap();
        mounted.truncate_file(&mut entry, 0).unwrap();
        assert_eq!(entry.stream, Stream::new(0, 0, 0, false));
        assert_eq!(image.borrow()[57][0] & 0b0000_1000, 0);
        drop(mounted);

        let (read, _) = callbacks(image, None);
        let mut reopened = Mounted::open(4096, 512, read).unwrap();
        assert_eq!(
            reopened.lookup("/a.txt").unwrap().stream,
            Stream::new(0, 0, 0, false)
        );
    }
}
