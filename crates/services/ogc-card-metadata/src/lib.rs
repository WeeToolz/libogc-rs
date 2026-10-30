#![no_std]

//! Memory-card system-block validation and verified commit support.

pub const BLOCK_SIZE: usize = 8192;
pub const HEADER_CHECKSUM_BYTES: usize = 508;
pub const DIRECTORY_CHECKSUM_BYTES: usize = 8188;
pub const FAT_CHECKSUM_START: usize = 4;

const HEADER_CHECKSUM_1: usize = 508;
const HEADER_CHECKSUM_2: usize = 510;
const DIRECTORY_UPDATED: usize = 8186;
const DIRECTORY_CHECKSUM_1: usize = 8188;
const DIRECTORY_CHECKSUM_2: usize = 8190;
const FAT_CHECKSUM_1: usize = 0;
const FAT_CHECKSUM_2: usize = 2;
const FAT_UPDATED: usize = 4;
const FAT_FREE_BLOCKS: usize = 6;
const FAT_ENTRIES: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Checksum {
    pub first: u16,
    pub second: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataError {
    InvalidLength,
    InvalidBlockCount,
    InvalidChecksum,
    InvalidFreeBlockCount,
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, MetadataError> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or(MetadataError::InvalidLength)?;
    Ok(u16::from_be_bytes([pair[0], pair[1]]))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), MetadataError> {
    let pair = bytes
        .get_mut(offset..offset + 2)
        .ok_or(MetadataError::InvalidLength)?;
    pair.copy_from_slice(&value.to_be_bytes());
    Ok(())
}

pub fn checksum(bytes: &[u8]) -> Result<Checksum, MetadataError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
        return Err(MetadataError::InvalidLength);
    }
    let mut first = 0u16;
    let mut second = 0u16;
    for pair in bytes.as_chunks::<2>().0 {
        let word = u16::from_be_bytes(*pair);
        first = first.wrapping_add(word);
        second = second.wrapping_add(word ^ 0xffff);
    }
    if first == 0xffff {
        first = 0;
    }
    if second == 0xffff {
        second = 0;
    }
    Ok(Checksum { first, second })
}

fn checksum_matches(
    block: &[u8],
    range: core::ops::Range<usize>,
    first_offset: usize,
    second_offset: usize,
) -> Result<bool, MetadataError> {
    let expected = checksum(block.get(range).ok_or(MetadataError::InvalidLength)?)?;
    Ok(read_u16(block, first_offset)? == expected.first
        && read_u16(block, second_offset)? == expected.second)
}

pub fn prepare_header(block: &mut [u8]) -> Result<(), MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    let value = checksum(&block[..HEADER_CHECKSUM_BYTES])?;
    write_u16(block, HEADER_CHECKSUM_1, value.first)?;
    write_u16(block, HEADER_CHECKSUM_2, value.second)
}

pub fn validate_header(block: &[u8]) -> Result<(), MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    checksum_matches(
        block,
        0..HEADER_CHECKSUM_BYTES,
        HEADER_CHECKSUM_1,
        HEADER_CHECKSUM_2,
    )?
    .then_some(())
    .ok_or(MetadataError::InvalidChecksum)
}

pub fn prepare_directory(block: &mut [u8]) -> Result<u16, MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    let updated = read_u16(block, DIRECTORY_UPDATED)?.wrapping_add(1);
    write_u16(block, DIRECTORY_UPDATED, updated)?;
    let value = checksum(&block[..DIRECTORY_CHECKSUM_BYTES])?;
    write_u16(block, DIRECTORY_CHECKSUM_1, value.first)?;
    write_u16(block, DIRECTORY_CHECKSUM_2, value.second)?;
    validate_directory(block)?;
    Ok(updated)
}

pub fn validate_directory(block: &[u8]) -> Result<u16, MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    if !checksum_matches(
        block,
        0..DIRECTORY_CHECKSUM_BYTES,
        DIRECTORY_CHECKSUM_1,
        DIRECTORY_CHECKSUM_2,
    )? {
        return Err(MetadataError::InvalidChecksum);
    }
    read_u16(block, DIRECTORY_UPDATED)
}

pub fn prepare_fat(block: &mut [u8], block_count: u16) -> Result<u16, MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    let updated = read_u16(block, FAT_UPDATED)?.wrapping_add(1);
    write_u16(block, FAT_UPDATED, updated)?;
    let value = checksum(&block[FAT_CHECKSUM_START..])?;
    write_u16(block, FAT_CHECKSUM_1, value.first)?;
    write_u16(block, FAT_CHECKSUM_2, value.second)?;
    validate_fat(block, block_count)?;
    Ok(updated)
}

pub fn validate_fat(block: &[u8], block_count: u16) -> Result<u16, MetadataError> {
    if block.len() != BLOCK_SIZE {
        return Err(MetadataError::InvalidLength);
    }
    if block_count < 5 || usize::from(block_count - 5) > (BLOCK_SIZE - FAT_ENTRIES) / 2 {
        return Err(MetadataError::InvalidBlockCount);
    }
    if !checksum_matches(
        block,
        FAT_CHECKSUM_START..BLOCK_SIZE,
        FAT_CHECKSUM_1,
        FAT_CHECKSUM_2,
    )? {
        return Err(MetadataError::InvalidChecksum);
    }
    let mut free = 0u16;
    for index in 0..usize::from(block_count - 5) {
        if read_u16(block, FAT_ENTRIES + index * 2)? == 0 {
            free = free.wrapping_add(1);
        }
    }
    if read_u16(block, FAT_FREE_BLOCKS)? != free {
        return Err(MetadataError::InvalidFreeBlockCount);
    }
    read_u16(block, FAT_UPDATED)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyIndex {
    Primary,
    Backup,
}

impl CopyIndex {
    pub const fn other(self) -> Self {
        match self {
            Self::Primary => Self::Backup,
            Self::Backup => Self::Primary,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Backup => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopySelection {
    pub newest: CopyIndex,
    pub next_target: CopyIndex,
    pub degraded: bool,
}

fn first_is_newer(first: u16, second: u16) -> bool {
    let distance = first.wrapping_sub(second);
    distance != 0 && distance < 0x8000
}

pub fn select_copy(
    primary: Result<u16, MetadataError>,
    backup: Result<u16, MetadataError>,
) -> Result<CopySelection, MetadataError> {
    let (newest, degraded) = match (primary, backup) {
        (Ok(first), Ok(second)) => {
            if first_is_newer(second, first) {
                (CopyIndex::Backup, false)
            } else {
                (CopyIndex::Primary, false)
            }
        }
        (Ok(_), Err(_)) => (CopyIndex::Primary, true),
        (Err(_), Ok(_)) => (CopyIndex::Backup, true),
        (Err(_), Err(_)) => return Err(MetadataError::InvalidChecksum),
    };
    Ok(CopySelection {
        newest,
        next_target: newest.other(),
        degraded,
    })
}

pub trait BlockIo {
    type Error;

    fn erase(&mut self, sector: u32) -> Result<(), Self::Error>;
    fn write(&mut self, sector: u32, block: &[u8]) -> Result<(), Self::Error>;
    fn read(&mut self, sector: u32, block: &mut [u8]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataBlock {
    Directory,
    Fat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairCommit {
    Create,
    Delete,
}

pub fn commit_pair<E>(
    operation: PairCommit,
    mut commit: impl FnMut(MetadataBlock) -> Result<(), E>,
) -> Result<(), E> {
    let order = match operation {
        PairCommit::Create => [MetadataBlock::Fat, MetadataBlock::Directory],
        PairCommit::Delete => [MetadataBlock::Directory, MetadataBlock::Fat],
    };
    commit(order[0])?;
    commit(order[1])
}

pub fn commit_system_blocks<E>(mut commit: impl FnMut(u32) -> Result<(), E>) -> Result<(), E> {
    for sector in 0..5 {
        commit(sector)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitError<E> {
    InvalidLength,
    Io(E),
    ReadbackMismatch,
}

pub fn commit_verified<I: BlockIo>(
    io: &mut I,
    sector: u32,
    expected: &[u8],
    readback: &mut [u8],
) -> Result<(), CommitError<I::Error>> {
    if expected.len() != BLOCK_SIZE || readback.len() != BLOCK_SIZE {
        return Err(CommitError::InvalidLength);
    }
    io.erase(sector).map_err(CommitError::Io)?;
    io.write(sector, expected).map_err(CommitError::Io)?;
    io.read(sector, readback).map_err(CommitError::Io)?;
    if readback != expected {
        return Err(CommitError::ReadbackMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn checksum_uses_fixed_known_vector() {
        assert_eq!(
            checksum(&[0x12, 0x34, 0xab, 0xcd]).unwrap(),
            Checksum {
                first: 0xbe01,
                second: 0x41fd,
            }
        );
    }

    #[test]
    fn protocol_apis_are_exercised_directly() {
        assert!(checksum(&[0, 1]).is_ok());
        let mut header = vec![0; BLOCK_SIZE];
        prepare_header(&mut header).unwrap();
        validate_header(&header).unwrap();

        let mut directory = vec![0; BLOCK_SIZE];
        assert_eq!(prepare_directory(&mut directory), Ok(1));
        assert_eq!(validate_directory(&directory), Ok(1));

        let mut fat = vec![0; BLOCK_SIZE];
        write_u16(&mut fat, FAT_FREE_BLOCKS, 59).unwrap();
        assert_eq!(prepare_fat(&mut fat, 64), Ok(1));
        assert_eq!(validate_fat(&fat, 64), Ok(1));

        assert_eq!(CopyIndex::Primary.index(), 0);
        assert_eq!(CopyIndex::Primary.other(), CopyIndex::Backup);
        let selection = select_copy(Ok(1), Ok(0)).unwrap();
        assert_eq!(selection.newest, CopyIndex::Primary);

        let mut media = Media::new();
        media.erase(0).unwrap();
        media.write(0, &header).unwrap();
        let mut block = vec![0; BLOCK_SIZE];
        media.read(0, &mut block).unwrap();
        assert_eq!(block, header);

        let mut committed = Vec::new();
        commit_pair(PairCommit::Create, |kind| {
            committed.push(kind);
            Ok::<_, TestError>(())
        })
        .unwrap();
        assert_eq!(
            committed,
            vec![MetadataBlock::Fat, MetadataBlock::Directory]
        );

        let mut sectors = Vec::new();
        commit_system_blocks(|sector| {
            sectors.push(sector);
            Ok::<_, TestError>(())
        })
        .unwrap();
        assert_eq!(sectors, vec![0, 1, 2, 3, 4]);

        let expected = vec![0x5a; BLOCK_SIZE];
        let mut readback = vec![0; BLOCK_SIZE];
        commit_verified(&mut media, 1, &expected, &mut readback).unwrap();
        assert_eq!(readback, expected);
    }

    #[test]
    fn copy_selection_recovers_and_handles_wrap() {
        let bad = Err(MetadataError::InvalidChecksum);
        assert_eq!(select_copy(Ok(7), bad).unwrap().newest, CopyIndex::Primary);
        assert_eq!(select_copy(bad, Ok(9)).unwrap().newest, CopyIndex::Backup);
        assert!(select_copy(bad, bad).is_err());
        assert_eq!(
            select_copy(Ok(u16::MAX), Ok(0)).unwrap().newest,
            CopyIndex::Backup
        );
        assert_eq!(
            select_copy(Ok(5), Ok(5)).unwrap().newest,
            CopyIndex::Primary
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestError {
        Injected,
    }

    struct Media {
        bytes: Vec<u8>,
        fail_step: Option<usize>,
        corrupt_readback: bool,
        step: usize,
    }

    impl Media {
        fn new() -> Self {
            Self {
                bytes: vec![0xff; BLOCK_SIZE * 5],
                fail_step: None,
                corrupt_readback: false,
                step: 0,
            }
        }

        fn pass(&mut self) -> Result<(), TestError> {
            let step = self.step;
            self.step += 1;
            if self.fail_step == Some(step) {
                Err(TestError::Injected)
            } else {
                Ok(())
            }
        }
    }

    impl BlockIo for Media {
        type Error = TestError;

        fn erase(&mut self, sector: u32) -> Result<(), Self::Error> {
            self.pass()?;
            let start = sector as usize * BLOCK_SIZE;
            self.bytes[start..start + BLOCK_SIZE].fill(0xff);
            Ok(())
        }

        fn write(&mut self, sector: u32, block: &[u8]) -> Result<(), Self::Error> {
            self.pass()?;
            let start = sector as usize * BLOCK_SIZE;
            self.bytes[start..start + BLOCK_SIZE].copy_from_slice(block);
            Ok(())
        }

        fn read(&mut self, sector: u32, block: &mut [u8]) -> Result<(), Self::Error> {
            self.pass()?;
            let start = sector as usize * BLOCK_SIZE;
            block.copy_from_slice(&self.bytes[start..start + BLOCK_SIZE]);
            if self.corrupt_readback {
                block[0] ^= 1;
            }
            Ok(())
        }
    }

    #[test]
    fn fat_preparation_produces_known_checksum_and_rejects_corruption() {
        let mut fat = vec![0; BLOCK_SIZE];
        fat[6..8].copy_from_slice(&[0, 59]);
        assert_eq!(prepare_fat(&mut fat, 64), Ok(1));
        assert_eq!(
            &fat[..10],
            &[0x00, 0x3c, 0xef, 0xc6, 0x00, 0x01, 0x00, 0x3b, 0x00, 0x00]
        );
        assert_eq!(validate_fat(&fat, 64), Ok(1));
        fat[10] ^= 1;
        assert_eq!(validate_fat(&fat, 64), Err(MetadataError::InvalidChecksum));
    }

    #[test]
    fn verified_commit_rejects_invalid_buffers_before_media_changes() {
        for (expected_len, readback_len) in [
            (BLOCK_SIZE - 1, BLOCK_SIZE),
            (BLOCK_SIZE, BLOCK_SIZE - 1),
            (BLOCK_SIZE + 1, BLOCK_SIZE),
        ] {
            let mut media = Media::new();
            let original = media.bytes.clone();
            let expected = vec![0x5a; expected_len];
            let mut readback = vec![0xa5; readback_len];
            assert_eq!(
                commit_verified(&mut media, 2, &expected, &mut readback),
                Err(CommitError::InvalidLength)
            );
            assert_eq!(media.step, 0);
            assert_eq!(media.bytes, original);
            assert!(readback.iter().all(|byte| *byte == 0xa5));
        }
    }

    #[test]
    fn verified_commit_persists_exact_bytes() {
        let mut media = Media::new();
        let expected = vec![0x5a; BLOCK_SIZE];
        let mut readback = vec![0; BLOCK_SIZE];
        commit_verified(&mut media, 2, &expected, &mut readback).unwrap();
        assert_eq!(&media.bytes[2 * BLOCK_SIZE..3 * BLOCK_SIZE], expected);
    }

    #[test]
    fn verified_commit_reports_each_transport_failure() {
        for fail_step in 0..3 {
            let mut media = Media::new();
            media.fail_step = Some(fail_step);
            let expected = vec![0x5a; BLOCK_SIZE];
            let mut readback = vec![0; BLOCK_SIZE];
            assert_eq!(
                commit_verified(&mut media, 2, &expected, &mut readback),
                Err(CommitError::Io(TestError::Injected))
            );
        }
    }

    #[test]
    fn verified_commit_rejects_corrupt_readback() {
        let mut media = Media::new();
        media.corrupt_readback = true;
        let expected = vec![0x5a; BLOCK_SIZE];
        let mut readback = vec![0; BLOCK_SIZE];
        assert_eq!(
            commit_verified(&mut media, 2, &expected, &mut readback),
            Err(CommitError::ReadbackMismatch)
        );
    }

    fn valid_directory(updated: u16, used: bool) -> Vec<u8> {
        let mut block = vec![0xff; BLOCK_SIZE];
        write_u16(&mut block, DIRECTORY_UPDATED, updated.wrapping_sub(1)).unwrap();
        if used {
            block[0..4].copy_from_slice(b"RUST");
            write_u16(&mut block, 54, 5).unwrap();
        }
        prepare_directory(&mut block).unwrap();
        block
    }

    fn valid_fat(updated: u16, allocated: bool) -> Vec<u8> {
        let mut block = vec![0; BLOCK_SIZE];
        write_u16(&mut block, FAT_UPDATED, updated.wrapping_sub(1)).unwrap();
        write_u16(&mut block, FAT_FREE_BLOCKS, if allocated { 58 } else { 59 }).unwrap();
        if allocated {
            write_u16(&mut block, FAT_ENTRIES, 0xffff).unwrap();
        }
        prepare_fat(&mut block, 64).unwrap();
        block
    }

    fn selected_directory(media: &Media) -> &[u8] {
        let primary = &media.bytes[BLOCK_SIZE..BLOCK_SIZE * 2];
        let backup = &media.bytes[BLOCK_SIZE * 2..BLOCK_SIZE * 3];
        match select_copy(validate_directory(primary), validate_directory(backup))
            .unwrap()
            .newest
        {
            CopyIndex::Primary => primary,
            CopyIndex::Backup => backup,
        }
    }

    fn selected_fat(media: &Media) -> &[u8] {
        let primary = &media.bytes[BLOCK_SIZE * 3..BLOCK_SIZE * 4];
        let backup = &media.bytes[BLOCK_SIZE * 4..BLOCK_SIZE * 5];
        match select_copy(validate_fat(primary, 64), validate_fat(backup, 64))
            .unwrap()
            .newest
        {
            CopyIndex::Primary => primary,
            CopyIndex::Backup => backup,
        }
    }

    fn initial_media(used: bool, allocated: bool) -> Media {
        let mut media = Media::new();
        let old_dir = valid_directory(1, used);
        let old_fat = valid_fat(1, allocated);
        media.bytes[BLOCK_SIZE..BLOCK_SIZE * 2].copy_from_slice(&old_dir);
        media.bytes[BLOCK_SIZE * 2..BLOCK_SIZE * 3].copy_from_slice(&old_dir);
        media.bytes[BLOCK_SIZE * 3..BLOCK_SIZE * 4].copy_from_slice(&old_fat);
        media.bytes[BLOCK_SIZE * 4..BLOCK_SIZE * 5].copy_from_slice(&old_fat);
        media
    }

    fn run_pair(
        media: &mut Media,
        operation: PairCommit,
        directory: &[u8],
        fat: &[u8],
    ) -> Result<(), CommitError<TestError>> {
        let mut directory_readback = vec![0; BLOCK_SIZE];
        let mut fat_readback = vec![0; BLOCK_SIZE];
        commit_pair(operation, |kind| match kind {
            MetadataBlock::Directory => {
                commit_verified(media, 2, directory, &mut directory_readback)
            }
            MetadataBlock::Fat => commit_verified(media, 4, fat, &mut fat_readback),
        })
    }

    #[test]
    fn failed_create_never_publishes_an_entry_without_allocated_blocks() {
        let new_dir = valid_directory(2, true);
        let new_fat = valid_fat(2, true);
        for fail_step in 0..6 {
            let mut media = initial_media(false, false);
            media.fail_step = Some(fail_step);
            assert!(run_pair(&mut media, PairCommit::Create, &new_dir, &new_fat).is_err());
            let entry_used = selected_directory(&media)[0] != 0xff;
            let first_block_allocated = read_u16(selected_fat(&media), FAT_ENTRIES).unwrap() != 0;
            assert!(!entry_used || first_block_allocated);
        }
    }

    #[test]
    fn failed_delete_never_leaves_an_entry_pointing_to_freed_blocks() {
        let new_dir = valid_directory(2, false);
        let new_fat = valid_fat(2, false);
        for fail_step in 0..6 {
            let mut media = initial_media(true, true);
            media.fail_step = Some(fail_step);
            assert!(run_pair(&mut media, PairCommit::Delete, &new_dir, &new_fat).is_err());
            let entry_used = selected_directory(&media)[0] != 0xff;
            let first_block_allocated = read_u16(selected_fat(&media), FAT_ENTRIES).unwrap() != 0;
            assert!(!entry_used || first_block_allocated);
        }
    }

    #[test]
    fn failed_status_commit_remounts_to_an_old_or_new_valid_directory() {
        let mut new_directory = valid_directory(1, true);
        new_directory[7] = 2;
        prepare_directory(&mut new_directory).unwrap();
        for fail_step in 0..3 {
            let mut media = initial_media(true, true);
            media.fail_step = Some(fail_step);
            let mut readback = vec![0; BLOCK_SIZE];
            assert!(commit_verified(&mut media, 2, &new_directory, &mut readback).is_err());
            let selected = selected_directory(&media);
            validate_directory(selected).unwrap();
            assert!(selected[7] == 0xff || selected[7] == 2);
        }
    }

    #[test]
    fn format_sequence_stops_at_every_failed_media_boundary() {
        let formatted: Vec<Vec<u8>> = (0..5)
            .map(|sector| {
                let mut block = vec![sector as u8; BLOCK_SIZE];
                match sector {
                    0 => prepare_header(&mut block).unwrap(),
                    1 | 2 => {
                        write_u16(&mut block, DIRECTORY_UPDATED, sector as u16).unwrap();
                        prepare_directory(&mut block).unwrap();
                    }
                    3 | 4 => {
                        block.fill(0);
                        write_u16(&mut block, FAT_UPDATED, sector as u16).unwrap();
                        write_u16(&mut block, FAT_FREE_BLOCKS, 59).unwrap();
                        prepare_fat(&mut block, 64).unwrap();
                    }
                    _ => unreachable!(),
                }
                block
            })
            .collect();
        for fail_step in 0..15 {
            let mut media = initial_media(false, false);
            let mut old_header = vec![0xff; BLOCK_SIZE];
            prepare_header(&mut old_header).unwrap();
            media.bytes[..BLOCK_SIZE].copy_from_slice(&old_header);
            media.fail_step = Some(fail_step);
            let mut readback = vec![0; BLOCK_SIZE];
            let result = commit_system_blocks(|sector| {
                commit_verified(
                    &mut media,
                    sector,
                    &formatted[sector as usize],
                    &mut readback,
                )
            });
            assert!(result.is_err());
            if validate_header(&media.bytes[..BLOCK_SIZE]).is_ok() {
                select_copy(
                    validate_directory(&media.bytes[BLOCK_SIZE..BLOCK_SIZE * 2]),
                    validate_directory(&media.bytes[BLOCK_SIZE * 2..BLOCK_SIZE * 3]),
                )
                .unwrap();
                select_copy(
                    validate_fat(&media.bytes[BLOCK_SIZE * 3..BLOCK_SIZE * 4], 64),
                    validate_fat(&media.bytes[BLOCK_SIZE * 4..BLOCK_SIZE * 5], 64),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn directory_and_fat_validation_use_persisted_bytes() {
        let mut header = vec![0xff; BLOCK_SIZE];
        header[0] = 0x52;
        prepare_header(&mut header).unwrap();
        assert_eq!(validate_header(&header), Ok(()));
        header[20] ^= 1;
        assert_eq!(
            validate_header(&header),
            Err(MetadataError::InvalidChecksum)
        );

        let mut directory = vec![0xff; BLOCK_SIZE];
        write_u16(&mut directory, DIRECTORY_UPDATED, 41).unwrap();
        assert_eq!(prepare_directory(&mut directory).unwrap(), 42);
        assert_eq!(validate_directory(&directory).unwrap(), 42);
        directory[100] ^= 1;
        assert_eq!(
            validate_directory(&directory),
            Err(MetadataError::InvalidChecksum)
        );

        let mut fat = vec![0; BLOCK_SIZE];
        write_u16(&mut fat, FAT_FREE_BLOCKS, 59).unwrap();
        assert_eq!(prepare_fat(&mut fat, 64).unwrap(), 1);
        assert_eq!(validate_fat(&fat, 64).unwrap(), 1);
        write_u16(&mut fat, FAT_FREE_BLOCKS, 58).unwrap();
        let value = checksum(&fat[FAT_CHECKSUM_START..]).unwrap();
        write_u16(&mut fat, FAT_CHECKSUM_1, value.first).unwrap();
        write_u16(&mut fat, FAT_CHECKSUM_2, value.second).unwrap();
        assert_eq!(
            validate_fat(&fat, 64),
            Err(MetadataError::InvalidFreeBlockCount)
        );
    }
}
