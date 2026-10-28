#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

mod fat_error;
mod file_transfer;
mod rename_transaction;
mod sector_cache;
mod timestamps;
pub use fat_error::{FatError, fat_errno};

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::{RefCell, UnsafeCell};
use core::cmp::min;
use core::ffi::{CStr, c_char, c_void};
use core::mem::size_of;
use core::ptr::{addr_of_mut, null_mut};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use ogc_filesystem_sys::DiscInterface;
use ogc_lwp_sys as lwp;
use ogc_sysbase::{self as sysbase, Stat, StatVfs};
use ogc_system_sys as system;

const MIN_SECTOR_BYTES: usize = 512;
const MAX_SECTOR_BYTES: usize = 4096;
pub const ATTR_ARCHIVE: u8 = 0x20;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_VOLUME: u8 = 0x08;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_READONLY: u8 = 0x01;
const FAT_ATTR_DIRECTORY: u8 = ATTR_DIRECTORY;
const FAT_ATTR_VOLUME_ID: u8 = ATTR_VOLUME;
const FAT_ATTR_LFN: u8 = 0x0f;
const FAT_ATTR_ARCHIVE: u8 = ATTR_ARCHIVE;
const FAT12_EOC: u32 = 0x0ff8;
const FAT12_BAD: u32 = 0x0ff7;
const FAT16_EOC: u32 = 0xfff8;
const FAT16_BAD: u32 = 0xfff7;
const FAT32_EOC: u32 = 0x0fff_fff8;
const FAT32_BAD: u32 = 0x0fff_fff7;
const FAT12_CLUSTER_LIMIT: u32 = 4_085;
const FAT16_CLUSTER_LIMIT: u32 = 65_525;
const FILE_MAGIC: u32 = 0x4641_5431;
const NAME_MAX: usize = 255;
const PATHCONF_NAME_MAX: i32 = 3;
const PATHCONF_PATH_MAX: i32 = 4;
const MEDIA_MONITOR_STACK_SIZE: usize = 8 * 1024;
const MEDIA_MONITOR_PRIORITY: u8 = 30;
const DEFAULT_CACHE_PAGES: u32 = 4;
const DEFAULT_SECTORS_PER_PAGE: u32 = 64;
const FEATURE_MEDIUM_CANWRITE: u32 = 0x0000_0002;
const ARGV_MAGIC: i32 = 0x5f61_7267;

#[repr(align(32))]
struct AlignedSector([u8; MAX_SECTOR_BYTES]);

const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;
const S_IRUSR: u32 = 0o0400;
const S_IWUSR: u32 = 0o0200;
const S_IRGRP: u32 = 0o0040;
const S_IWGRP: u32 = 0o0020;
const S_IROTH: u32 = 0o0004;
const S_IWOTH: u32 = 0o0002;
const ST_RDONLY: u32 = 0x0001;
const ST_NOSUID: u32 = 0x0002;

static FAT_MUTEX_READY: AtomicBool = AtomicBool::new(false);
static FAT_MUTEX_STARTING: AtomicBool = AtomicBool::new(false);
static mut FAT_MUTEX: lwp::Mutex = lwp::LWP_MUTEX_NULL;

fn fat_mutex() -> lwp::Mutex {
    if FAT_MUTEX_READY.load(Ordering::Acquire) {
        return unsafe { FAT_MUTEX };
    }
    if FAT_MUTEX_STARTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let mut handle = lwp::LWP_MUTEX_NULL;
        // SAFETY: `handle` is a valid unique output reference.
        let result = unsafe { lwp::mutex_init(&mut handle, 1) };
        if result != lwp::LWP_SUCCESSFUL {
            FAT_MUTEX_STARTING.store(false, Ordering::Release);
            panic!("FAT scheduler mutex allocation failed");
        }
        unsafe { FAT_MUTEX = handle };
        FAT_MUTEX_READY.store(true, Ordering::Release);
        FAT_MUTEX_STARTING.store(false, Ordering::Release);
        handle
    } else {
        while !FAT_MUTEX_READY.load(Ordering::Acquire) {
            lwp::yield_thread();
        }
        unsafe { FAT_MUTEX }
    }
}

struct SpinLock<T> {
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> SpinGuard<'_, T> {
        let mutex = fat_mutex();
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            panic!("FAT scheduler mutex lock failed");
        }
        SpinGuard { lock: self, mutex }
    }
}

struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
    mutex: lwp::Mutex,
}

impl<T> core::ops::Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        let _ = lwp::mutex_unlock(self.mutex);
    }
}

#[derive(Copy, Clone)]
pub enum MountSource {
    Sd,
    Usb,
    Usb2,
    CardA,
    CardB,
}

#[derive(Copy, Clone)]
pub struct MountOptions {
    pub mount_name: &'static CStr,
    pub read_only: bool,
}

static FILE_CLOSE_FAILURES: AtomicU32 = AtomicU32::new(0);
static LAST_FILE_CLOSE_ERRNO: AtomicI32 = AtomicI32::new(0);

/// Cleanup errors remain visible when Drop cannot return a result.
pub fn file_close_failures() -> u32 {
    FILE_CLOSE_FAILURES.load(Ordering::Acquire)
}

/// Positive Newlib errno from the most recent failed file cleanup.
pub fn last_file_close_errno() -> i32 {
    LAST_FILE_CLOSE_ERRNO.load(Ordering::Acquire)
}

fn record_file_close_failure(error: FatError) {
    LAST_FILE_CLOSE_ERRNO.store(fat_errno(error), Ordering::Release);
    FILE_CLOSE_FAILURES.fetch_add(1, Ordering::AcqRel);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

impl FatType {
    fn eoc_marker(self) -> u32 {
        match self {
            Self::Fat12 => FAT12_EOC,
            Self::Fat16 => FAT16_EOC,
            Self::Fat32 => FAT32_EOC,
        }
    }

    fn bad_marker(self) -> u32 {
        match self {
            Self::Fat12 => FAT12_BAD,
            Self::Fat16 => FAT16_BAD,
            Self::Fat32 => FAT32_BAD,
        }
    }

    fn fat_entry_capacity(self, fat_bytes: u32) -> Option<u32> {
        match self {
            Self::Fat12 => fat_bytes.checked_mul(2).map(|bytes| bytes / 3),
            Self::Fat16 => Some(fat_bytes / 2),
            Self::Fat32 => Some(fat_bytes / 4),
        }
    }
}

fn decode_fat12_entry(low: u8, high: u8, cluster: u32) -> u32 {
    let packed = u16::from(low) | (u16::from(high) << 8);
    if cluster & 1 == 0 {
        u32::from(packed & 0x0fff)
    } else {
        u32::from(packed >> 4)
    }
}

fn encode_fat12_entry(low: u8, high: u8, cluster: u32, value: u32) -> (u8, u8) {
    let value = (value & 0x0fff) as u16;
    if cluster & 1 == 0 {
        (value as u8, (high & 0xf0) | ((value >> 8) as u8 & 0x0f))
    } else {
        (
            (low & 0x0f) | ((value << 4) as u8 & 0xf0),
            (value >> 4) as u8,
        )
    }
}

struct FatMount {
    disc: &'static DiscInterface,
    read_only: bool,
    online: AtomicBool,
    media_generation: AtomicU32,
    fat_type: FatType,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    fat_count: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
    root_dir_start: u32,
    root_dir_sectors: u32,
    first_fat_sector: u32,
    active_fat: u8,
    mirror_fats: bool,
    first_data_sector: u32,
    cluster_count: u32,
    fat_cache: RefCell<sector_cache::SectorCache>,
    next_free_hint: u32,
    last_alloc_cluster: u32,
    fs_info_sector: Option<u32>,
    boot_label: [u8; 11],
    cwd_cluster: u32,
    open_files: BTreeMap<(u32, u16), OpenFileUse>,
    open_dirs: BTreeMap<u32, u32>,
}

#[derive(Copy, Clone)]
struct OpenFileUse {
    readers: u32,
    writer: bool,
    size: u32,
}

impl FatMount {
    fn ensure_online(&self) -> Result<(), FatError> {
        if self.online.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(FatError::MediaRemoved)
        }
    }

    fn check_media(&self) -> Result<(), FatError> {
        self.ensure_online()?;
        let Some(is_inserted) = self.disc.is_inserted else {
            self.mark_removed();
            return Err(FatError::DeviceUnavailable);
        };
        if unsafe { is_inserted() } {
            Ok(())
        } else {
            self.mark_removed();
            Err(FatError::MediaRemoved)
        }
    }

    fn mark_removed(&self) {
        if self.online.swap(false, Ordering::AcqRel) {
            self.media_generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn is_online(&self) -> bool {
        self.online.load(Ordering::Acquire)
    }

    pub fn media_generation(&self) -> u32 {
        self.media_generation.load(Ordering::Acquire)
    }

    fn cluster_size(&self) -> u32 {
        self.sectors_per_cluster as u32 * self.bytes_per_sector as u32
    }

    fn is_eoc(&self, cluster: u32) -> bool {
        cluster >= self.fat_type.eoc_marker()
    }

    fn is_bad_cluster(&self, cluster: u32) -> bool {
        cluster == self.fat_type.bad_marker()
    }

    fn is_fixed_root(&self, cluster: u32) -> bool {
        self.fat_type != FatType::Fat32 && cluster == self.root_cluster
    }

    fn acquire_file(
        &mut self,
        sector: u32,
        offset: u16,
        writable: bool,
        size: u32,
    ) -> Result<(), FatError> {
        let key = (sector, offset);
        if let Some(open) = self.open_files.get_mut(&key) {
            if writable || open.writer {
                return Err(FatError::Busy);
            }
            open.readers = open.readers.checked_add(1).ok_or(FatError::Busy)?;
        } else {
            self.open_files.insert(
                key,
                OpenFileUse {
                    readers: u32::from(!writable),
                    writer: writable,
                    size,
                },
            );
        }
        Ok(())
    }

    fn release_file(&mut self, sector: u32, offset: u16, writable: bool) {
        let key = (sector, offset);
        let remove = if let Some(open) = self.open_files.get_mut(&key) {
            if writable {
                open.writer = false;
            } else {
                open.readers = open.readers.saturating_sub(1);
            }
            !open.writer && open.readers == 0
        } else {
            false
        };
        if remove {
            self.open_files.remove(&key);
        }
    }

    fn file_is_open(&self, sector: u32, offset: u16) -> bool {
        self.open_files.contains_key(&(sector, offset))
    }

    fn publish_file_size(&mut self, file: &FatFileState) -> Result<(), FatError> {
        let open = self
            .open_files
            .get_mut(&(file.dir_sector, file.dir_offset))
            .ok_or(FatError::BadDescriptor)?;
        open.size = file.size;
        Ok(())
    }

    fn acquire_dir(&mut self, cluster: u32) -> Result<(), FatError> {
        let count = self.open_dirs.entry(cluster).or_insert(0);
        *count = count.checked_add(1).ok_or(FatError::Busy)?;
        Ok(())
    }

    fn release_dir(&mut self, cluster: u32) {
        let remove = if let Some(count) = self.open_dirs.get_mut(&cluster) {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        };
        if remove {
            self.open_dirs.remove(&cluster);
        }
    }

    fn dir_is_open(&self, cluster: u32) -> bool {
        self.open_dirs.contains_key(&cluster)
    }

    fn has_open_handles(&self) -> bool {
        !self.open_files.is_empty() || !self.open_dirs.is_empty()
    }

    fn cluster_to_sector(&self, cluster: u32) -> Result<u32, FatError> {
        let last_cluster = self.cluster_count.checked_add(1).ok_or(FatError::Io)?;
        if cluster < 2 || cluster > last_cluster {
            return Err(FatError::Io);
        }
        let sector_delta = (cluster - 2)
            .checked_mul(self.sectors_per_cluster as u32)
            .ok_or(FatError::Io)?;
        self.first_data_sector
            .checked_add(sector_delta)
            .ok_or(FatError::Io)
    }

    fn read_sector(&self, lba: u32, buffer: &mut [u8]) -> Result<(), FatError> {
        if buffer.len() < self.bytes_per_sector as usize {
            return Err(FatError::Io);
        }
        self.ensure_online()?;
        let Some(read_sectors) = self.disc.read_sectors else {
            return Err(FatError::Io);
        };
        if read_disc_sector_aligned(read_sectors, lba, self.bytes_per_sector as usize, buffer) {
            Ok(())
        } else if self.check_media().is_err() {
            Err(FatError::MediaRemoved)
        } else {
            Err(FatError::Io)
        }
    }

    fn write_sector(&self, lba: u32, buffer: &[u8]) -> Result<(), FatError> {
        if buffer.len() < self.bytes_per_sector as usize {
            return Err(FatError::Io);
        }
        self.ensure_online()?;
        if self.read_only {
            return Err(FatError::ReadOnly);
        }
        let Some(write_sectors) = self.disc.write_sectors else {
            return Err(FatError::Io);
        };
        // A failed write can have changed media; never retain its old image.
        self.fat_cache.borrow_mut().invalidate(lba);
        let mut sector = AlignedSector([0; MAX_SECTOR_BYTES]);
        sector.0[..self.bytes_per_sector as usize]
            .copy_from_slice(&buffer[..self.bytes_per_sector as usize]);
        unsafe {
            if write_sectors(lba, 1, sector.0.as_ptr().cast()) {
                Ok(())
            } else if self.check_media().is_err() {
                Err(FatError::MediaRemoved)
            } else {
                Err(FatError::Io)
            }
        }
    }

    fn flush_transport(&self) -> Result<(), FatError> {
        self.ensure_online()?;
        let usb_slot = [
            &ogc_usbstorage_sys::USB_STORAGE_INTERFACE,
            &ogc_usbstorage_sys::USB_STORAGE2_INTERFACE,
        ]
        .iter()
        .position(|disc| core::ptr::eq(self.disc, *disc));
        if let Some(slot) = usb_slot {
            let rc = ogc_usbstorage_sys::flush_drive(slot);
            if rc < 0 {
                if self.check_media().is_err() {
                    return Err(FatError::MediaRemoved);
                }
                return Err(FatError::Io);
            }
        }
        Ok(())
    }

    fn load_or_create_fs_info(&mut self) -> Result<(), FatError> {
        let Some(sector) = self.fs_info_sector else {
            return Ok(());
        };
        if self.fat_type != FatType::Fat32 {
            return Ok(());
        }
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        self.read_sector(sector, &mut buffer)?;
        let valid = &buffer[0..4] == b"RRaA"
            && &buffer[0x1e4..0x1e8] == b"rrAa"
            && buffer[0x1fe] == 0x55
            && buffer[0x1ff] == 0xaa;
        if valid {
            let free = u32::from_le_bytes(buffer[0x1e8..0x1ec].try_into().unwrap());
            let last = u32::from_le_bytes(buffer[0x1ec..0x1f0].try_into().unwrap());
            if last >= 2 && last <= self.cluster_count + 1 {
                self.next_free_hint = last;
                self.last_alloc_cluster = last;
            }
            // A valid cached free count is advisory. The Rust implementation
            // still scans the FAT for statvfs/allocation correctness.
            if free != 0 && free != u32::MAX {
                return Ok(());
            }
        }
        if self.read_only {
            return Ok(());
        }
        self.sync_fs_info()
    }

    fn sync_fs_info(&mut self) -> Result<(), FatError> {
        let Some(sector) = self.fs_info_sector else {
            return Ok(());
        };
        if self.fat_type != FatType::Fat32 || self.read_only {
            return Ok(());
        }
        let free_clusters = count_free_clusters(self)?;
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        buffer[0..4].copy_from_slice(b"RRaA");
        buffer[0x1e4..0x1e8].copy_from_slice(b"rrAa");
        buffer[0x1e8..0x1ec].copy_from_slice(&free_clusters.to_le_bytes());
        buffer[0x1ec..0x1f0].copy_from_slice(&self.last_alloc_cluster.to_le_bytes());
        buffer[0x1fe] = 0x55;
        buffer[0x1ff] = 0xaa;
        self.write_sector(sector, &buffer)?;
        Ok(())
    }

    fn read_fat_sector(&self, sector: u32, buffer: &mut [u8]) -> Result<(), FatError> {
        self.ensure_online()?;
        let output = buffer
            .get_mut(..self.bytes_per_sector as usize)
            .ok_or(FatError::Io)?;
        self.fat_cache
            .borrow_mut()
            .read(sector, output, |bytes| self.read_sector(sector, bytes))
    }

    fn read_fat_entry(&self, cluster: u32) -> Result<u32, FatError> {
        let fat_start = (self.active_fat as u32)
            .checked_mul(self.sectors_per_fat)
            .and_then(|delta| self.first_fat_sector.checked_add(delta))
            .ok_or(FatError::Io)?;

        match self.fat_type {
            FatType::Fat12 => {
                let byte_offset = cluster.checked_mul(3).ok_or(FatError::Io)? / 2;
                let sector = fat_start
                    .checked_add(byte_offset / self.bytes_per_sector as u32)
                    .ok_or(FatError::Io)?;
                let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                let mut first = [0u8; MAX_SECTOR_BYTES];
                self.read_fat_sector(sector, &mut first)?;
                let low = first[offset] as u16;
                let high = if offset + 1 < self.bytes_per_sector as usize {
                    first[offset + 1] as u16
                } else {
                    let mut second = [0u8; MAX_SECTOR_BYTES];
                    self.read_fat_sector(sector.checked_add(1).ok_or(FatError::Io)?, &mut second)?;
                    second[0] as u16
                };
                Ok(decode_fat12_entry(low as u8, high as u8, cluster))
            }
            FatType::Fat16 => {
                let byte_offset = cluster.checked_mul(2).ok_or(FatError::Io)?;
                let sector = fat_start
                    .checked_add(byte_offset / self.bytes_per_sector as u32)
                    .ok_or(FatError::Io)?;
                let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                let mut buffer = [0u8; MAX_SECTOR_BYTES];
                self.read_fat_sector(sector, &mut buffer)?;
                Ok(u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as u32)
            }
            FatType::Fat32 => {
                let byte_offset = cluster.checked_mul(4).ok_or(FatError::Io)?;
                let sector = fat_start
                    .checked_add(byte_offset / self.bytes_per_sector as u32)
                    .ok_or(FatError::Io)?;
                let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                let mut buffer = [0u8; MAX_SECTOR_BYTES];
                self.read_fat_sector(sector, &mut buffer)?;
                let value = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());
                Ok(value & 0x0fff_ffff)
            }
        }
    }

    fn write_fat_entry(&self, cluster: u32, value: u32) -> Result<(), FatError> {
        let fat_range = if self.mirror_fats {
            0..self.fat_count as u32
        } else {
            self.active_fat as u32..self.active_fat as u32 + 1
        };
        for fat_index in fat_range {
            let fat_start = fat_index
                .checked_mul(self.sectors_per_fat)
                .and_then(|delta| self.first_fat_sector.checked_add(delta))
                .ok_or(FatError::Io)?;
            match self.fat_type {
                FatType::Fat12 => {
                    let byte_offset = cluster.checked_mul(3).ok_or(FatError::Io)? / 2;
                    let sector = fat_start
                        .checked_add(byte_offset / self.bytes_per_sector as u32)
                        .ok_or(FatError::Io)?;
                    let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                    let mut first = [0u8; MAX_SECTOR_BYTES];
                    self.read_sector(sector, &mut first)?;
                    let next_sector = sector.checked_add(1).ok_or(FatError::Io)?;
                    let (existing_high, mut second) = if offset + 1 < self.bytes_per_sector as usize
                    {
                        (first[offset + 1], None)
                    } else {
                        let mut buffer = [0u8; MAX_SECTOR_BYTES];
                        self.read_sector(next_sector, &mut buffer)?;
                        (buffer[0], Some(buffer))
                    };
                    let (encoded_low, encoded_high) =
                        encode_fat12_entry(first[offset], existing_high, cluster, value);
                    first[offset] = encoded_low;
                    if let Some(ref mut second) = second {
                        second[0] = encoded_high;
                        self.write_sector(sector, &first)?;
                        self.write_sector(next_sector, second)?;
                    } else {
                        first[offset + 1] = encoded_high;
                        self.write_sector(sector, &first)?;
                    }
                }
                FatType::Fat16 => {
                    let byte_offset = cluster.checked_mul(2).ok_or(FatError::Io)?;
                    let sector = fat_start
                        .checked_add(byte_offset / self.bytes_per_sector as u32)
                        .ok_or(FatError::Io)?;
                    let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                    let mut buffer = [0u8; MAX_SECTOR_BYTES];
                    self.read_sector(sector, &mut buffer)?;
                    buffer[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
                    self.write_sector(sector, &buffer)?;
                }
                FatType::Fat32 => {
                    let value = value & 0x0fff_ffff;
                    let byte_offset = cluster.checked_mul(4).ok_or(FatError::Io)?;
                    let sector = fat_start
                        .checked_add(byte_offset / self.bytes_per_sector as u32)
                        .ok_or(FatError::Io)?;
                    let offset = (byte_offset % self.bytes_per_sector as u32) as usize;
                    let mut buffer = [0u8; MAX_SECTOR_BYTES];
                    self.read_sector(sector, &mut buffer)?;
                    let existing =
                        u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());
                    let encoded = (existing & 0xf000_0000) | value;
                    buffer[offset..offset + 4].copy_from_slice(&encoded.to_le_bytes());
                    self.write_sector(sector, &buffer)?;
                }
            }
        }
        Ok(())
    }

    fn zero_cluster(&self, cluster: u32) -> Result<(), FatError> {
        let sector = self.cluster_to_sector(cluster)?;
        let buffer = [0u8; MAX_SECTOR_BYTES];
        for idx in 0..self.sectors_per_cluster as u32 {
            self.write_sector(sector + idx, &buffer)?;
        }
        Ok(())
    }

    fn allocate_cluster(&mut self, previous: Option<u32>) -> Result<u32, FatError> {
        if self.read_only {
            return Err(FatError::ReadOnly);
        }

        let start = self.next_free_hint.max(2);
        for pass in 0..2 {
            let range_start = if pass == 0 { start } else { 2 };
            let range_end = if pass == 0 {
                self.cluster_count + 1
            } else {
                start.saturating_sub(1)
            };
            for cluster in range_start..=range_end {
                if self.read_fat_entry(cluster)? == 0 {
                    self.write_fat_entry(cluster, self.fat_type.eoc_marker())?;
                    if let Err(error) = self.zero_cluster(cluster) {
                        self.write_fat_entry(cluster, 0)?;
                        return Err(error);
                    }
                    if let Some(prev) = previous {
                        // A failed multi-copy FAT update can have committed
                        // a link. Keep its allocated target valid; freeing
                        // it here could leave a chain pointing at free space.
                        self.write_fat_entry(prev, cluster)?;
                    }
                    self.next_free_hint = cluster.saturating_add(1);
                    self.last_alloc_cluster = cluster;
                    return Ok(cluster);
                }
            }
        }

        Err(FatError::NoSpace)
    }

    fn nth_cluster(
        &mut self,
        start_cluster: &mut u32,
        index: u32,
        allocate: bool,
    ) -> Result<u32, FatError> {
        if index > self.cluster_count {
            return Err(FatError::Io);
        }
        if *start_cluster == 0 {
            if !allocate {
                return Err(FatError::NotFound);
            }
            *start_cluster = self.allocate_cluster(None)?;
        }

        let mut current = *start_cluster;
        for _ in 0..index {
            if current < 2 || current > self.cluster_count + 1 {
                return Err(FatError::Io);
            }
            let next = self.read_fat_entry(current)?;
            if self.is_eoc(next) {
                if !allocate {
                    return Err(FatError::NotFound);
                }
                current = self.allocate_cluster(Some(current))?;
            } else if next == 0 || self.is_bad_cluster(next) {
                return Err(FatError::Io);
            } else {
                current = next;
            }
        }
        Ok(current)
    }

    fn update_short_entry(
        &self,
        sector: u32,
        offset: u16,
        start_cluster: u32,
        size: u32,
    ) -> Result<(), FatError> {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        self.read_sector(sector, &mut buffer)?;
        let offset = offset as usize;
        buffer[offset + 20..offset + 22]
            .copy_from_slice(&((start_cluster >> 16) as u16).to_le_bytes());
        buffer[offset + 26..offset + 28].copy_from_slice(&(start_cluster as u16).to_le_bytes());
        buffer[offset + 28..offset + 32].copy_from_slice(&size.to_le_bytes());
        buffer[offset + 11] |= FAT_ATTR_ARCHIVE;
        self.write_sector(sector, &buffer)?;
        Ok(())
    }
}

struct DirectoryWalk {
    cluster: u32,
    sector_index: u32,
    fixed_root: bool,
}

impl DirectoryWalk {
    fn new(mount: &FatMount, cluster: u32) -> Result<Self, FatError> {
        if mount.is_fixed_root(cluster) {
            return Ok(Self {
                cluster,
                sector_index: 0,
                fixed_root: true,
            });
        }
        if cluster < 2 || cluster > mount.cluster_count + 1 {
            return Err(FatError::Io);
        }
        Ok(Self {
            cluster,
            sector_index: 0,
            fixed_root: false,
        })
    }

    fn next_sector(&mut self, mount: &mut FatMount, extend: bool) -> Result<Option<u32>, FatError> {
        if self.fixed_root {
            if self.sector_index >= mount.root_dir_sectors {
                return Ok(None);
            }
            let sector = mount
                .root_dir_start
                .checked_add(self.sector_index)
                .ok_or(FatError::Io)?;
            self.sector_index += 1;
            return Ok(Some(sector));
        }

        if self.sector_index >= mount.sectors_per_cluster as u32 {
            let next = mount.read_fat_entry(self.cluster)?;
            if mount.is_bad_cluster(next) || next == 0 {
                return Err(FatError::Io);
            }
            if mount.is_eoc(next) {
                if !extend {
                    return Ok(None);
                }
                self.cluster = mount.allocate_cluster(Some(self.cluster))?;
            } else {
                if next < 2 || next > mount.cluster_count + 1 {
                    return Err(FatError::Io);
                }
                self.cluster = next;
            }
            self.sector_index = 0;
        }

        let sector = mount
            .cluster_to_sector(self.cluster)?
            .checked_add(self.sector_index)
            .ok_or(FatError::Io)?;
        self.sector_index += 1;
        Ok(Some(sector))
    }
}

struct RegisteredMount {
    dev_name_with_nul: Box<[u8]>,
    mount: SpinLock<FatMount>,
    manage_transport: bool,
    _cache_pages: u32,
    _sectors_per_page: u32,
    devoptab: sysbase::Devoptab,
    registered: bool,
}

unsafe impl Send for RegisteredMount {}

#[repr(C)]
struct FatFileState {
    magic: u32,
    mount: *const SpinLock<FatMount>,
    dir_sector: u32,
    dir_offset: u16,
    _pad: u16,
    flags: i32,
    start_cluster: u32,
    size: u32,
    position: u32,
    dirty: u32,
}

#[derive(Clone)]
struct DirEntry {
    name: Vec<u8>,
    attr: u8,
    start_cluster: u32,
    size: u32,
    entry_span: Vec<EntryPos>,
    short_sector: u32,
    short_offset: u16,
}

impl DirEntry {
    fn cluster_blocks(&self, mount: &mut FatMount) -> u32 {
        cluster_blocks(mount, self.start_cluster)
    }
}

fn cluster_blocks(mount: &mut FatMount, start_cluster: u32) -> u32 {
    if start_cluster < 2 || start_cluster > mount.cluster_count + 1 {
        return 0;
    }
    let mut count = 0u32;
    let mut cluster = start_cluster;
    for _ in 0..=mount.cluster_count {
        let Ok(next) = mount.read_fat_entry(cluster) else {
            break;
        };
        count = count.saturating_add(mount.sectors_per_cluster as u32);
        if mount.is_eoc(next) || next == 0 || mount.is_bad_cluster(next) {
            break;
        }
        if next < 2 || next > mount.cluster_count + 1 {
            break;
        }
        cluster = next;
    }
    count
}

#[derive(Clone)]
struct EntryPos {
    sector: u32,
    offset: u16,
}

#[derive(Copy, Clone)]
struct LfnEntry {
    ordinal: u8,
    checksum: u8,
    format_valid: bool,
    chars: [u16; 13],
}

struct DirState {
    magic: u32,
    mount: *const SpinLock<FatMount>,
    initial_cluster: u32,
    cluster: u32,
    sector_index: u32,
    entry_index: u32,
}

const DIR_MAGIC: u32 = 0x4449_5231;

/// An open file on a mounted FAT volume.
///
/// The handle keeps the mount busy until it is dropped. Use [`FatFile::read`]
/// to read file contents without exposing the FAT implementation details.
pub struct FatFile {
    state: FatFileState,
}

impl FatFile {
    /// Open a file for reading by its mounted path, for example `sd:/apps/x/boot.dol`.
    pub fn open(path: &CStr) -> Result<Self, FatError> {
        Self::open_with_flags(path, 0)
    }

    /// Create a file when it does not exist and open it for reading and writing.
    pub fn create(path: &CStr) -> Result<Self, FatError> {
        Self::open_with_flags(path, sysbase::O_RDWR | sysbase::O_CREAT)
    }

    /// Create a new file and fail if a file with the same name already exists.
    pub fn create_new(path: &CStr) -> Result<Self, FatError> {
        Self::open_with_flags(path, sysbase::O_RDWR | sysbase::O_CREAT | sysbase::O_EXCL)
    }

    /// Create a file or replace its current contents, then open it for reading and writing.
    pub fn create_truncate(path: &CStr) -> Result<Self, FatError> {
        Self::open_with_flags(path, sysbase::O_RDWR | sysbase::O_CREAT | sysbase::O_TRUNC)
    }

    /// Open with exact Newlib flags. No alternative access mode is tried.
    pub fn open_with_flags(path: &CStr, flags: i32) -> Result<Self, FatError> {
        let known = sysbase::O_ACCMODE
            | sysbase::O_APPEND
            | sysbase::O_CREAT
            | sysbase::O_TRUNC
            | sysbase::O_EXCL;
        let access = flags & sysbase::O_ACCMODE;
        if flags & !known != 0
            || access == sysbase::O_ACCMODE
            || (flags & (sysbase::O_TRUNC | sysbase::O_APPEND) != 0 && access == sysbase::O_RDONLY)
            || (flags & sysbase::O_EXCL != 0 && flags & sysbase::O_CREAT == 0)
        {
            return Err(FatError::PathError);
        }
        let mut state = FatFileState {
            magic: 0,
            mount: core::ptr::null(),
            dir_sector: 0,
            dir_offset: 0,
            _pad: 0,
            flags: 0,
            start_cluster: 0,
            size: 0,
            position: 0,
            dirty: 0,
        };
        open_file_from_path(path, flags, &mut state)?;
        Ok(Self { state })
    }

    /// Return the file size in bytes.
    pub fn len(&self) -> u32 {
        self.state.size
    }

    /// Return true when the file has no data.
    pub fn is_empty(&self) -> bool {
        self.state.size == 0
    }

    /// Read bytes at the current file position.
    pub fn read(&mut self, output: &mut [u8]) -> Result<usize, FatError> {
        if self.state.flags & sysbase::O_ACCMODE == sysbase::O_WRONLY {
            return Err(FatError::BadDescriptor);
        }
        if output.is_empty() {
            return Ok(0);
        }
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        let mut mount = lock.lock();
        mount.ensure_online()?;
        read_file_bytes(&mut mount, &mut self.state, output)
    }

    /// Set the current file position. The position may be past the file end.
    pub fn seek(&mut self, position: u32) {
        self.state.position = position;
    }

    /// Return the shared open-file cursor.
    pub fn position(&self) -> u32 {
        self.state.position
    }

    /// Check the live mount before changing the cursor.
    pub fn seek_checked(&mut self, position: u32) -> Result<(), FatError> {
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        lock.lock().ensure_online()?;
        self.seek(position);
        Ok(())
    }

    /// Read current attributes from the medium and return the open-file size.
    pub fn metadata(&self) -> Result<(u32, bool), FatError> {
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        let mount = lock.lock();
        let mut sector = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(self.state.dir_sector, &mut sector)?;
        let readonly =
            mount.read_only || sector[self.state.dir_offset as usize + 11] & ATTR_READONLY != 0;
        Ok((self.state.size, readonly))
    }

    /// Change the on-disk read-only attribute through this live handle.
    pub fn set_readonly(&mut self, readonly: bool) -> Result<(), FatError> {
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::BadDescriptor)?;
        let mount = lock.lock();
        update_readonly(
            &mount,
            self.state.dir_sector,
            self.state.dir_offset,
            readonly,
        )
    }

    /// Read FAT timestamp fields from this handle's current directory entry.
    pub fn timestamps(&self) -> Result<FatTimestamps, FatError> {
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::BadDescriptor)?;
        read_timestamps(&lock.lock(), self.state.dir_sector, self.state.dir_offset)
    }

    /// Set selected FAT timestamps. Modification has two-second resolution;
    /// access has one-day resolution. Unselected fields remain unchanged.
    pub fn set_times(
        &mut self,
        accessed: Option<u64>,
        modified: Option<u64>,
    ) -> Result<(), FatError> {
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::BadDescriptor)?;
        write_timestamps(
            &lock.lock(),
            self.state.dir_sector,
            self.state.dir_offset,
            accessed,
            modified,
        )
    }

    /// Write bytes at the current position. Return the number of bytes written.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, FatError> {
        if self.state.flags & (sysbase::O_WRONLY | sysbase::O_RDWR) == 0 {
            return Err(FatError::BadDescriptor);
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        let mut mount = lock.lock();
        if self.state.flags & sysbase::O_APPEND != 0 {
            self.state.position = self.state.size;
        }
        mount.ensure_online()?;
        let len = min(bytes.len(), (u32::MAX - self.state.position) as usize);
        if len == 0 {
            return Err(FatError::FileTooLarge);
        }
        write_file_bytes(&mut mount, &mut self.state, &bytes[..len])
    }

    /// Set the file size in bytes.
    pub fn truncate(&mut self, size: u32) -> Result<(), FatError> {
        if self.state.flags & (sysbase::O_WRONLY | sysbase::O_RDWR) == 0 {
            return Err(FatError::BadDescriptor);
        }
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        let mut mount = lock.lock();
        mount.ensure_online()?;
        truncate_file(&mut mount, &mut self.state, size)?;
        mount.flush_transport()
    }

    /// Flush file metadata and transport buffers.
    pub fn flush(&mut self) -> Result<(), FatError> {
        flush_file_state(&mut self.state)
    }

    /// Flush, release the mount reference, and report the final cleanup result.
    pub fn close(mut self) -> Result<(), FatError> {
        close_file_state(&mut self.state).inspect_err(|error| record_file_close_failure(*error))
    }
}

impl Drop for FatFile {
    fn drop(&mut self) {
        if self.state.magic != FILE_MAGIC {
            return;
        }
        close_file_state(&mut self.state).unwrap_or_else(record_file_close_failure);
    }
}

/// A directory item returned by [`FatDirectory::next_entry`].
pub struct FatDirEntry {
    /// The filename bytes on disk. FAT filenames do not always contain UTF-8.
    pub name: Vec<u8>,
    pub is_directory: bool,
    pub size: u32,
}

/// Basic file metadata returned by [`stat`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FatMetadata {
    pub is_directory: bool,
    pub size: u32,
    pub blocks: u32,
    pub readonly: bool,
}

/// Return basic metadata for a mounted path.
pub fn stat(path: &CStr) -> Result<FatMetadata, FatError> {
    let mut raw = Stat::default();
    stat_path(path, &mut raw)?;
    Ok(FatMetadata {
        is_directory: raw.st_mode & S_IFDIR == S_IFDIR,
        size: raw.st_size.max(0) as u32,
        blocks: raw.st_blocks.max(0) as u32,
        readonly: raw.st_mode & S_IWUSR == 0,
    })
}

/// Return the DOS attribute byte for a file or directory.
pub fn get_attributes(path: &CStr) -> Result<u8, FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
    Ok(entry.attr)
}

/// Set DOS attributes while preserving structural FAT directory bits.
///
/// libfat writes the complete byte. Rust rejects attempts to turn a normal
/// entry into a directory, volume label, or LFN entry because that can corrupt
/// the directory structure. The ordinary archive/system/hidden/read-only bits
/// remain fully writable.
pub fn set_attributes(path: &CStr, attributes: u8) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    if mount.read_only {
        return Err(FatError::ReadOnly);
    }
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
    let structural = FAT_ATTR_DIRECTORY | FAT_ATTR_VOLUME_ID;
    if attributes == FAT_ATTR_LFN || (attributes & structural) != (entry.attr & structural) {
        return Err(FatError::PathError);
    }
    let mut sector = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(entry.short_sector, &mut sector)?;
    sector[entry.short_offset as usize + 11] = attributes;
    mount.write_sector(entry.short_sector, &sector)?;
    mount.flush_transport()
}

/// Change only the read-only attribute while holding the volume lock.
pub fn set_readonly(path: &CStr, readonly: bool) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotSupported)?;
    if entry.entry_span.is_empty() {
        return Err(FatError::NotSupported);
    }
    update_readonly(&mount, entry.short_sector, entry.short_offset, readonly)
}

fn update_readonly(
    mount: &FatMount,
    sector: u32,
    offset: u16,
    readonly: bool,
) -> Result<(), FatError> {
    mount.ensure_online()?;
    if mount.read_only {
        return Err(FatError::ReadOnly);
    }
    let mut bytes = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut bytes)?;
    let attribute = &mut bytes[offset as usize + 11];
    if readonly {
        *attribute |= ATTR_READONLY;
    } else {
        *attribute &= !ATTR_READONLY;
    }
    mount.write_sector(sector, &bytes)?;
    mount.flush_transport()
}

/// Resolve every component on the medium and return stored long names.
/// Dot and parent components cannot traverse a regular file. Root parents stay
/// at the mounted root. Both long names and generated short aliases are valid.
pub fn canonicalize(path: &CStr) -> Result<Vec<u8>, FatError> {
    let (key, relative) = split_mount_path(path)?;
    if !relative.starts_with(b"/") {
        return Err(FatError::PathError);
    }
    let mut mount = lock_mount_by_key(&key)?;
    mount.ensure_online()?;
    let mut stack = Vec::<(Vec<u8>, u32)>::new();
    let mut cluster = mount.root_cluster;
    let mut directory = true;
    for part in relative.split(|b| *b == b'/').skip(1) {
        if !directory {
            return Err(FatError::NotDirectory);
        }
        match part {
            b"" | b"." => {}
            b".." => {
                stack.pop();
                cluster = stack
                    .last()
                    .map_or(mount.root_cluster, |(_, cluster)| *cluster);
            }
            _ => {
                let entry = find_entry_in_directory(&mut mount, cluster, part)?
                    .ok_or(FatError::NotFound)?;
                directory = entry.attr & FAT_ATTR_DIRECTORY != 0;
                cluster = entry.start_cluster;
                stack.push((entry.name, cluster));
            }
        }
    }
    let mut output = key;
    output.push(b'/');
    for (index, (name, _)) in stack.into_iter().enumerate() {
        if index != 0 {
            output.push(b'/');
        }
        output.extend_from_slice(&name);
    }
    Ok(output)
}

/// On-disk FAT calendar values. A zero date is absent, not an epoch timestamp.
#[derive(Clone, Copy, Debug, Default)]
pub struct FatTimestamps {
    pub accessed: Option<u64>,
    pub modified: Option<u64>,
    pub created: Option<(u64, u32)>,
}

fn read_timestamps(mount: &FatMount, sector: u32, offset: u16) -> Result<FatTimestamps, FatError> {
    let mut bytes = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut bytes)?;
    let entry = &bytes[offset as usize..offset as usize + 32];
    let field = |index| u16::from_le_bytes([entry[index], entry[index + 1]]);
    let decode = |date, time| {
        if date == 0 {
            Ok(None)
        } else {
            timestamps::decode(date, time).map(Some).ok_or(FatError::Io)
        }
    };
    let created = decode(field(16), field(14))?.map(|seconds| {
        let fraction = u64::from(entry[13]);
        (
            seconds + fraction / 100,
            (fraction % 100) as u32 * 10_000_000,
        )
    });
    if created.is_some() && entry[13] > 199 {
        return Err(FatError::Io);
    }
    Ok(FatTimestamps {
        accessed: decode(field(18), 0)?,
        modified: decode(field(24), field(22))?,
        created,
    })
}

fn write_timestamps(
    mount: &FatMount,
    sector: u32,
    offset: u16,
    accessed: Option<u64>,
    modified: Option<u64>,
) -> Result<(), FatError> {
    // Validate both fields before any media operation.
    let encode = |value: Option<u64>| {
        value
            .map(|seconds| timestamps::encode(seconds).ok_or(FatError::PathError))
            .transpose()
    };
    let accessed = encode(accessed)?;
    let modified = encode(modified)?;
    mount.ensure_online()?;
    if mount.read_only {
        return Err(FatError::ReadOnly);
    }
    let mut bytes = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut bytes)?;
    let entry = &mut bytes[offset as usize..offset as usize + 32];
    if entry[11] & ATTR_READONLY != 0 {
        return Err(FatError::ReadOnly);
    }
    if let Some((date, _)) = accessed {
        entry[18..20].copy_from_slice(&date.to_le_bytes());
    }
    if let Some((date, time)) = modified {
        entry[22..24].copy_from_slice(&time.to_le_bytes());
        entry[24..26].copy_from_slice(&date.to_le_bytes());
    }
    if accessed.is_some() || modified.is_some() {
        mount.write_sector(sector, &bytes)?;
    }
    mount.flush_transport()
}

/// Read stored timestamps. FAT roots have no timestamp fields.
pub fn get_times(path: &CStr) -> Result<FatTimestamps, FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    mount.ensure_online()?;
    match resolve_path(&mut mount, relative)? {
        Some(entry) if !entry.entry_span.is_empty() => {
            read_timestamps(&mount, entry.short_sector, entry.short_offset)
        }
        _ => Ok(FatTimestamps::default()),
    }
}

/// Set selected stored timestamps. Mounted roots have no timestamp fields.
pub fn set_times(
    path: &CStr,
    accessed: Option<u64>,
    modified: Option<u64>,
) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotSupported)?;
    if entry.entry_span.is_empty() {
        return Err(FatError::NotSupported);
    }
    write_timestamps(
        &mount,
        entry.short_sector,
        entry.short_offset,
        accessed,
        modified,
    )
}

/// Return the mounted volume label, matching libfat's root-label-first policy.
pub fn volume_label(name: &CStr) -> Result<Vec<u8>, FatError> {
    let key = normalize_volume_lookup_name(name).ok_or(FatError::PathError)?;
    let mut mount = lock_mount_by_key(&key)?;
    if let Some(label) = root_volume_label(&mut mount)? {
        return Ok(normalize_volume_label(&label));
    }
    Ok(normalize_volume_label(&mount.boot_label))
}

/// Create a directory at a mounted path.
pub fn create_directory(path: &CStr) -> Result<(), FatError> {
    mkdir_path(path)
}

/// Remove a file at a mounted path.
pub fn remove_file(path: &CStr) -> Result<(), FatError> {
    unlink_path(path)
}

/// Remove an empty directory at a mounted path.
pub fn remove_directory(path: &CStr) -> Result<(), FatError> {
    rmdir_path(path)
}

/// Rename a file or directory on the same mounted volume.
pub fn rename(old_path: &CStr, new_path: &CStr) -> Result<(), FatError> {
    rename_path(old_path, new_path)
}

/// Rename with std replacement semantics. Reject open files and nonempty
/// target directories before changing either directory entry.
pub fn rename_replace(old_path: &CStr, new_path: &CStr) -> Result<(), FatError> {
    rename_path_impl(old_path, new_path, true)
}

/// Remove a duplicate directory entry left by an interrupted FAT rename.
///
/// This only unlinks `alias_path` when both paths resolve to the same directory
/// cluster on the same mounted volume. It never frees the shared cluster chain.
pub fn repair_interrupted_directory_rename(
    primary_path: &CStr,
    alias_path: &CStr,
) -> Result<bool, FatError> {
    let (primary_key, primary_relative) = split_mount_path(primary_path)?;
    let (alias_key, alias_relative) = split_mount_path(alias_path)?;
    if primary_key != alias_key || ascii_fold(primary_relative) == ascii_fold(alias_relative) {
        return Ok(false);
    }
    let mut mount = lock_mount_by_key(&primary_key)?;
    let primary = resolve_path(&mut mount, primary_relative)?.ok_or(FatError::NotFound)?;
    let alias = resolve_path(&mut mount, alias_relative)?.ok_or(FatError::NotFound)?;
    if primary.attr & FAT_ATTR_DIRECTORY == 0
        || alias.attr & FAT_ATTR_DIRECTORY == 0
        || primary.start_cluster != alias.start_cluster
    {
        return Ok(false);
    }
    if mount.dir_is_open(primary.start_cluster) {
        return Err(FatError::Busy);
    }
    mark_entry_deleted(&mount, &alias.entry_span)?;
    mount.flush_transport()?;
    Ok(true)
}

/// An open directory on a mounted FAT volume.
pub struct FatDirectory {
    state: DirState,
    finished: bool,
}

impl FatDirectory {
    /// Open a directory by its mounted path, for example `sd:/apps`.
    pub fn open(path: &CStr) -> Result<Self, FatError> {
        let (mut mount, relative) = mount_and_relative_from_path(path)?;
        let cluster = if path_is_root(relative) {
            mount.root_cluster
        } else {
            let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
            if entry.attr & FAT_ATTR_DIRECTORY == 0 {
                return Err(FatError::NotDirectory);
            }
            entry.start_cluster
        };
        mount.acquire_dir(cluster)?;
        Ok(Self {
            state: DirState {
                magic: DIR_MAGIC,
                mount: mount.lock as *const SpinLock<FatMount>,
                initial_cluster: cluster,
                cluster,
                sector_index: 0,
                entry_index: 0,
            },
            finished: false,
        })
    }

    /// Return the next directory item, or `None` when the directory is complete.
    pub fn next_entry(&mut self) -> Result<Option<FatDirEntry>, FatError> {
        if self.finished {
            return Ok(None);
        }
        let lock = unsafe { self.state.mount.as_ref() }.ok_or(FatError::Io)?;
        let mut mount = lock.lock();
        let mut filename = [0u8; NAME_MAX + 1];
        let mut stat = Stat::default();
        if !dirnext_entry(
            &mut self.state,
            &mut mount,
            filename.as_mut_ptr(),
            Some(&mut stat),
        )? {
            self.finished = true;
            return Ok(None);
        }
        let end = filename
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(FatError::Io)?;
        Ok(Some(FatDirEntry {
            name: filename[..end].to_vec(),
            is_directory: stat.st_mode & S_IFDIR == S_IFDIR,
            size: stat.st_size.max(0) as u32,
        }))
    }
}

impl Drop for FatDirectory {
    fn drop(&mut self) {
        if self.state.magic != DIR_MAGIC {
            return;
        }
        if let Some(lock) = unsafe { self.state.mount.as_ref() } {
            let mut mount = lock.lock();
            mount.release_dir(self.state.initial_cluster);
        }
        self.state.magic = 0;
    }
}

static MOUNTS: SpinLock<BTreeMap<Vec<u8>, Box<RegisteredMount>>> = SpinLock::new(BTreeMap::new());

#[repr(align(32))]
struct MediaMonitorStack([u8; MEDIA_MONITOR_STACK_SIZE]);

static MEDIA_MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);
static MEDIA_MONITOR_STOP: AtomicBool = AtomicBool::new(false);
static mut MEDIA_MONITOR_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;
static mut MEDIA_MONITOR_STACK: MediaMonitorStack =
    MediaMonitorStack([0; MEDIA_MONITOR_STACK_SIZE]);

unsafe extern "C" fn media_monitor_main(_arg: *mut c_void) -> *mut c_void {
    loop {
        lwp::sleep_ticks(lwp::TICKS_PER_SECOND / 4);
        if MEDIA_MONITOR_STOP.load(Ordering::Acquire) {
            break;
        }
        let _ = poll_media();
    }
    MEDIA_MONITOR_RUNNING.store(false, Ordering::Release);
    null_mut()
}

fn ensure_media_monitor() -> bool {
    if MEDIA_MONITOR_RUNNING.load(Ordering::Acquire) {
        return true;
    }
    if MEDIA_MONITOR_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return true;
    }
    MEDIA_MONITOR_STOP.store(false, Ordering::Release);
    let result = unsafe {
        lwp::create_thread(
            addr_of_mut!(MEDIA_MONITOR_THREAD),
            Some(media_monitor_main),
            null_mut(),
            addr_of_mut!(MEDIA_MONITOR_STACK.0).cast(),
            MEDIA_MONITOR_STACK_SIZE as u32,
            MEDIA_MONITOR_PRIORITY,
        )
    };
    if result != lwp::LWP_SUCCESSFUL {
        MEDIA_MONITOR_RUNNING.store(false, Ordering::Release);
        return false;
    }
    true
}

fn stop_media_monitor() {
    if !MEDIA_MONITOR_RUNNING.load(Ordering::Acquire) {
        return;
    }
    MEDIA_MONITOR_STOP.store(true, Ordering::Release);
    let thread = unsafe { MEDIA_MONITOR_THREAD };
    if thread != lwp::LWP_THREAD_NULL && thread != lwp::get_self() {
        // SAFETY: null requests that the synchronous join discard the result.
        let _ = unsafe { lwp::join_thread(thread, null_mut()) };
    }
    unsafe {
        MEDIA_MONITOR_THREAD = lwp::LWP_THREAD_NULL;
    }
    MEDIA_MONITOR_RUNNING.store(false, Ordering::Release);
}

pub fn mount_sd() -> Result<(), FatError> {
    mount(
        MountSource::Sd,
        MountOptions {
            mount_name: c"sd",
            read_only: false,
        },
    )
}

pub fn mount_usb() -> Result<(), FatError> {
    mount(
        MountSource::Usb,
        MountOptions {
            mount_name: c"usb",
            read_only: false,
        },
    )
}

/// Mount the SD Gecko in GameCube memory card slot A as `carda:`.
pub fn mount_card_a() -> Result<(), FatError> {
    mount(
        MountSource::CardA,
        MountOptions {
            mount_name: c"carda",
            read_only: false,
        },
    )
}

/// Mount the SD Gecko in GameCube memory card slot B as `cardb:`.
pub fn mount_card_b() -> Result<(), FatError> {
    mount(
        MountSource::CardB,
        MountOptions {
            mount_name: c"cardb",
            read_only: false,
        },
    )
}

/// Check for inserted media without mounting or reading its FAT volume.
///
/// If the interface is not already mounted, this starts the device before the
/// check. A failed startup reports no available device. A successful startup
/// is shut down when the check reports no media.
mod media_presence;
use media_presence::check_device_presence;

pub fn device_is_inserted(source: MountSource) -> Result<bool, FatError> {
    let disc = mount_interface(source);
    let Some(startup) = disc.startup else {
        return Err(FatError::DeviceUnavailable);
    };
    let Some(is_inserted) = disc.is_inserted else {
        return Err(FatError::DeviceUnavailable);
    };
    let mounted = is_mounted(mount_name(source));
    check_device_presence(
        mounted,
        || unsafe { startup() },
        || unsafe { is_inserted() },
        || shutdown_disc(disc),
    )
}

pub fn mount(source: MountSource, options: MountOptions) -> Result<(), FatError> {
    mount_disc(
        mount_interface(source),
        options.mount_name,
        options.read_only,
        0,
        true,
        DEFAULT_CACHE_PAGES,
        DEFAULT_SECTORS_PER_PAGE,
    )
}

fn mount_disc(
    disc: &'static DiscInterface,
    mount_name: &CStr,
    read_only: bool,
    start_sector: u32,
    manage_transport: bool,
    cache_pages: u32,
    sectors_per_page: u32,
) -> Result<(), FatError> {
    let dev_name = normalize_mount_name(mount_name).ok_or(FatError::PathError)?;
    let dev_name_for_cstr = dev_name.clone();
    let dev_name_cstr =
        CStr::from_bytes_with_nul(dev_name_for_cstr.as_slice()).map_err(|_| FatError::PathError)?;
    let key = dev_name[..dev_name.len() - 1].to_vec();

    {
        let mut mounts = MOUNTS.lock();
        let remove_stale = if let Some(existing) = mounts.get_mut(&key) {
            let mount = existing.mount.lock();
            if mount.is_online() {
                // libfat fatMount() is idempotent: if the device name is
                // already registered, mounting succeeds without rebuilding it.
                return Ok(());
            }
            if mount.has_open_handles() {
                return Err(FatError::Busy);
            }
            drop(mount);
            if existing.registered {
                if sysbase::remove_device(dev_name_cstr) < 0 {
                    return Err(FatError::Io);
                }
                existing.registered = false;
            }
            true
        } else {
            false
        };
        if remove_stale {
            if let Some(stale) = mounts.get(&key)
                && stale.manage_transport
            {
                shutdown_disc(stale.mount.lock().disc)?;
            }
            mounts.remove(&key);
        }
    }

    // libfat checks the global devoptab before starting or probing the medium.
    // Preserve that behavior even when the existing name belongs to a device
    // registered outside this Rust mount table.
    if sysbase::find_device(dev_name_cstr) >= 0 {
        return Ok(());
    }

    if manage_transport {
        let Some(startup) = disc.startup else {
            return Err(FatError::DeviceUnavailable);
        };
        let Some(is_inserted) = disc.is_inserted else {
            return Err(FatError::DeviceUnavailable);
        };
        if !check_device_presence(
            false,
            || unsafe { startup() },
            || unsafe { is_inserted() },
            || shutdown_disc(disc),
        )? {
            return Err(FatError::DeviceUnavailable);
        }
    }

    let mount = match parse_fat_mount_at(disc, read_only, start_sector) {
        Ok(mount) => mount,
        Err(error) => {
            if manage_transport && shutdown_disc(disc).is_err() {
                return Err(FatError::Io);
            }
            return Err(error);
        }
    };
    let mut mounts = MOUNTS.lock();
    if mounts.contains_key(&key) || sysbase::find_device(dev_name_cstr) >= 0 {
        drop(mounts);
        if manage_transport {
            shutdown_disc(disc)?;
        }
        return Err(FatError::AlreadyMounted);
    }
    let mut registered = Box::new(RegisteredMount {
        dev_name_with_nul: dev_name.into_boxed_slice(),
        mount: SpinLock::new(mount),
        manage_transport,
        _cache_pages: cache_pages.max(2),
        _sectors_per_page: sectors_per_page.max(8),
        devoptab: empty_devoptab(),
        registered: false,
    });

    registered.devoptab.name = registered.dev_name_with_nul.as_ptr().cast();
    registered.devoptab.struct_size = size_of::<FatFileState>();
    registered.devoptab.open_r = Some(fat_open_r);
    registered.devoptab.close_r = Some(fat_close_r);
    registered.devoptab.read_r = Some(fat_read_r);
    registered.devoptab.seek_r = Some(fat_seek_r);
    registered.devoptab.fstat_r = Some(fat_fstat_r);
    registered.devoptab.stat_r = Some(fat_stat_r);
    registered.devoptab.link_r = Some(fat_link_r);
    registered.devoptab.chdir_r = Some(fat_chdir_r);
    registered.devoptab.write_r = Some(fat_write_r);
    registered.devoptab.unlink_r = Some(fat_unlink_r);
    registered.devoptab.rename_r = Some(fat_rename_r);
    registered.devoptab.mkdir_r = Some(fat_mkdir_r);
    registered.devoptab.dir_state_size = size_of::<DirState>();
    registered.devoptab.diropen_r = Some(fat_diropen_r);
    registered.devoptab.dirreset_r = Some(fat_dirreset_r);
    registered.devoptab.dirnext_r = Some(fat_dirnext_r);
    registered.devoptab.dirclose_r = Some(fat_dirclose_r);
    registered.devoptab.statvfs_r = Some(fat_statvfs_r);
    registered.devoptab.ftruncate_r = Some(fat_ftruncate_r);
    registered.devoptab.fsync_r = Some(fat_fsync_r);
    registered.devoptab.rmdir_r = Some(fat_rmdir_r);
    registered.devoptab.lstat_r = Some(fat_stat_r);
    registered.devoptab.fpathconf_r = Some(fat_fpathconf_r);
    registered.devoptab.pathconf_r = Some(fat_pathconf_r);
    registered.devoptab.device_data = addr_of_mut!(registered.mount).cast();

    // SAFETY: the boxed registration has a stable address and is retained in
    // `mounts` until `RemoveDevice` succeeds during unmount.
    if unsafe { sysbase::add_device(&registered.devoptab) } < 0 {
        if manage_transport {
            shutdown_disc(disc)?;
        }
        return Err(FatError::Io);
    }
    registered.registered = true;
    mounts.insert(key.clone(), registered);
    drop(mounts);
    if !ensure_media_monitor() {
        return match unmount(dev_name_cstr) {
            Ok(()) => Err(FatError::Io),
            Err(error) => Err(error),
        };
    }
    Ok(())
}

fn c_mount(
    name: &CStr,
    disc: &'static DiscInterface,
    start_sector: u32,
    cache_pages: u32,
    sectors_per_page: u32,
) -> Result<(), FatError> {
    let read_only = disc.features & FEATURE_MEDIUM_CANWRITE == 0;
    mount_disc(
        disc,
        name,
        read_only,
        start_sector,
        false,
        cache_pages.max(2),
        sectors_per_page.max(8),
    )
}

/// # Safety
/// `name` must be a valid NUL-terminated string. `interface` and its callbacks must remain valid for the mount lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fatMount(
    name: *const c_char,
    interface: *const DiscInterface,
    start_sector: u32,
    cache_size: u32,
    sectors_per_page: u32,
) -> bool {
    if name.is_null() || interface.is_null() {
        return false;
    }
    let name = unsafe { CStr::from_ptr(name) };
    // C requires a stable DISC_INTERFACE for the complete mount lifetime.
    let disc: &'static DiscInterface = unsafe { &*interface };
    c_mount(name, disc, start_sector, cache_size, sectors_per_page).is_ok()
}

/// # Safety
/// `name` must be a valid NUL-terminated string. `interface` and its callbacks must remain valid for the mount lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fatMountSimple(
    name: *const c_char,
    interface: *const DiscInterface,
) -> bool {
    unsafe {
        fatMount(
            name,
            interface,
            0,
            DEFAULT_CACHE_PAGES,
            DEFAULT_SECTORS_PER_PAGE,
        )
    }
}

/// # Safety
/// A non-null `name` must point to a valid NUL-terminated mount name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fatUnmount(name: *const c_char) {
    if name.is_null() {
        return;
    }
    let _ = unmount(unsafe { CStr::from_ptr(name) });
}

fn init_default_interfaces(cache_size: u32, set_default: bool) -> bool {
    let interfaces: [(&CStr, &'static DiscInterface); 5] = [
        (c"sd", &ogc_wiisd::WII_SD_INTERFACE),
        (c"usb", &ogc_usbstorage_sys::USB_STORAGE_INTERFACE),
        (c"carda", &ogc_gcsd::GAMECUBE_SD_A_INTERFACE),
        (c"cardb", &ogc_gcsd::GAMECUBE_SD_B_INTERFACE),
        (c"usb2", &ogc_usbstorage_sys::USB_STORAGE2_INTERFACE),
    ];
    let mut first_device = None;
    for (name, disc) in interfaces {
        if c_mount(name, disc, 0, cache_size, DEFAULT_SECTORS_PER_PAGE).is_ok()
            && first_device.is_none()
        {
            let mut key = name.to_bytes().to_vec();
            key.push(b':');
            key.push(0);
            if let Ok(path) = CStr::from_bytes_with_nul(&key) {
                let index = sysbase::find_device(path);
                if index >= 0 {
                    first_device = Some((index, name));
                }
            }
        }
    }
    let Some((device_index, default_name)) = first_device else {
        return false;
    };
    if set_default {
        sysbase::setDefaultDevice(device_index);
        let mut cwd = default_name.to_bytes().to_vec();
        cwd.extend_from_slice(b":/\0");
        if let Ok(cwd) = CStr::from_bytes_with_nul(&cwd) {
            let _ = set_mount_cwd(cwd);
        }
        apply_argv_working_directory();
    }
    true
}

fn set_mount_cwd(path: &CStr) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let cluster = if path_is_root(relative) {
        mount.root_cluster
    } else {
        let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
        if entry.attr & FAT_ATTR_DIRECTORY == 0 {
            return Err(FatError::PathError);
        }
        entry.start_cluster
    };
    mount.cwd_cluster = cluster;
    Ok(())
}

fn apply_argv_working_directory() {
    // SAFETY: startup owns this descriptor and keeps argv storage alive.
    let argv = unsafe { system::current_system_argv().as_ref() };
    let Some(argv) = argv else { return };
    if argv.argv_magic != ARGV_MAGIC || argv.argc < 1 || argv.argv.is_null() {
        return;
    }
    let argv0 = unsafe { *argv.argv };
    if argv0.is_null() {
        return;
    }
    let bytes = unsafe { CStr::from_ptr(argv0) }.to_bytes();
    let Some(last_slash) = bytes.iter().rposition(|&byte| byte == b'/') else {
        return;
    };
    let dir_end = if last_slash > 0 && bytes[last_slash - 1] == b':' {
        last_slash + 1
    } else {
        last_slash
    };
    let mut path = bytes[..dir_end].to_vec();
    path.push(0);
    let Ok(path) = CStr::from_bytes_with_nul(&path) else {
        return;
    };
    let _ = set_mount_cwd(path);
}

#[unsafe(no_mangle)]
pub extern "C" fn fatInit(cache_size: u32, set_as_default_device: bool) -> bool {
    init_default_interfaces(cache_size, set_as_default_device)
}

#[unsafe(no_mangle)]
pub extern "C" fn fatInitDefault() -> bool {
    init_default_interfaces(DEFAULT_CACHE_PAGES, true)
}

/// # Safety
/// Non-null `name` must point to a NUL-terminated mount name. Non-null `label` must point to at least 12 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fatGetVolumeLabel(name: *const c_char, label: *mut c_char) {
    if name.is_null() || label.is_null() {
        return;
    }
    let Ok(value) = volume_label(unsafe { CStr::from_ptr(name) }) else {
        return;
    };
    let len = value.len().min(11);
    unsafe {
        core::ptr::copy_nonoverlapping(value.as_ptr(), label.cast::<u8>(), len);
        *label.add(len) = 0;
    }
}

/// # Safety
/// A non-null `file` must point to a valid NUL-terminated path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FAT_getAttr(file: *const c_char) -> i32 {
    if file.is_null() {
        return -1;
    }
    match get_attributes(unsafe { CStr::from_ptr(file) }) {
        Ok(attributes) => i32::from(attributes),
        Err(_) => -1,
    }
}

/// # Safety
/// A non-null `file` must point to a valid NUL-terminated path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FAT_setAttr(file: *const c_char, attr: u8) -> i32 {
    if file.is_null() {
        return -1;
    }
    match set_attributes(unsafe { CStr::from_ptr(file) }, attr) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

fn mount_interface(source: MountSource) -> &'static DiscInterface {
    match source {
        MountSource::Sd => &ogc_wiisd::WII_SD_INTERFACE,
        MountSource::Usb => &ogc_usbstorage_sys::USB_STORAGE_INTERFACE,
        MountSource::Usb2 => &ogc_usbstorage_sys::USB_STORAGE2_INTERFACE,
        MountSource::CardA => &ogc_gcsd::GAMECUBE_SD_A_INTERFACE,
        MountSource::CardB => &ogc_gcsd::GAMECUBE_SD_B_INTERFACE,
    }
}

fn mount_name(source: MountSource) -> &'static CStr {
    match source {
        MountSource::Sd => c"sd",
        MountSource::Usb => c"usb",
        MountSource::Usb2 => c"usb2",
        MountSource::CardA => c"carda",
        MountSource::CardB => c"cardb",
    }
}

pub fn unmount(name: &CStr) -> Result<(), FatError> {
    let key = normalize_lookup_name(name).ok_or(FatError::PathError)?;
    let name_with_nul = {
        let mut bytes = key.clone();
        bytes.push(0);
        bytes
    };
    let cstr = CStr::from_bytes_with_nul(&name_with_nul).map_err(|_| FatError::PathError)?;
    let mut mounts = MOUNTS.lock();
    let registered = mounts.get_mut(&key).ok_or(FatError::NotFound)?;
    let (disc, manage_transport) = {
        let mut mount = registered.mount.lock();
        if mount.has_open_handles() {
            return Err(FatError::Busy);
        }
        if mount.is_online() {
            // libfat persists FAT32 FSInfo during partition teardown. Fail
            // loudly if metadata cannot be written rather than silently
            // discarding the error.
            mount.sync_fs_info()?;
            mount.flush_transport()?;
        }
        (mount.disc, registered.manage_transport)
    };
    if registered.registered {
        if sysbase::remove_device(cstr) < 0 {
            return Err(FatError::NotFound);
        }
        registered.registered = false;
    }
    if manage_transport {
        shutdown_disc(disc)?;
    }
    mounts.remove(&key).ok_or(FatError::NotFound)?;
    Ok(())
}

/// Polls mounted block devices and unregisters any volume whose media vanished.
/// Existing file/dir handles retain their allocation but all I/O returns
/// [`FatError::MediaRemoved`] until they are closed and the volume is remounted.
pub fn poll_media() -> usize {
    let mut removed = 0usize;
    let mut mounts = MOUNTS.lock();
    for registered in mounts.values_mut() {
        let became_offline = {
            let mount = registered.mount.lock();
            let was_online = mount.is_online();
            match mount.check_media() {
                Ok(()) | Err(FatError::MediaRemoved) => {}
                Err(_) => continue,
            }
            was_online && !mount.is_online()
        };
        if became_offline {
            removed += 1;
            if registered.registered {
                if let Ok(name) = CStr::from_bytes_with_nul(&registered.dev_name_with_nul) {
                    if sysbase::remove_device(name) < 0 {
                        continue;
                    }
                } else {
                    continue;
                }
                registered.registered = false;
            }
        }
    }
    removed
}

/// Unregisters and shuts down every mounted SD/USB volume.
/// Returns [`FatError::Busy`] without removing anything when a file or
/// directory handle remains open.
pub fn shutdown_all() -> Result<(), FatError> {
    {
        let mut mounts = MOUNTS.lock();
        if mounts
            .values()
            .any(|registered| registered.mount.lock().has_open_handles())
        {
            return Err(FatError::Busy);
        }
        for registered in mounts.values() {
            let mount = registered.mount.lock();
            if mount.is_online() {
                mount.flush_transport()?;
            }
        }
        let mut remove_error = None;
        for registered in mounts.values_mut() {
            if registered.registered {
                if let Ok(name) = CStr::from_bytes_with_nul(&registered.dev_name_with_nul) {
                    if sysbase::remove_device(name) < 0 {
                        remove_error.get_or_insert(FatError::Io);
                        continue;
                    }
                } else {
                    remove_error.get_or_insert(FatError::PathError);
                    continue;
                }
                registered.registered = false;
            }
        }
        if let Some(error) = remove_error {
            return Err(error);
        }
    }

    stop_media_monitor();

    let mut mounts = MOUNTS.lock();
    let keys: Vec<Vec<u8>> = mounts.keys().cloned().collect();
    let mut first_error = None;
    for key in keys {
        let result = mounts
            .get(&key)
            .ok_or(FatError::NotFound)
            .and_then(|registered| shutdown_disc(registered.mount.lock().disc));
        if result.is_ok() {
            mounts.remove(&key);
        } else if first_error.is_none() {
            first_error = result.err();
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub fn media_generation(name: &CStr) -> Option<u32> {
    let key = normalize_lookup_name(name)?;
    let mounts = MOUNTS.lock();
    mounts
        .get(&key)
        .map(|registered| registered.mount.lock().media_generation())
}

pub fn is_mounted(name: &CStr) -> bool {
    let Some(key) = normalize_lookup_name(name) else {
        return false;
    };
    let mounts = MOUNTS.lock();
    mounts
        .get(&key)
        .is_some_and(|registered| registered.registered && registered.mount.lock().is_online())
}

fn shutdown_disc(disc: &'static DiscInterface) -> Result<(), FatError> {
    unsafe {
        if disc.shutdown.is_some_and(|shutdown| !shutdown()) {
            return Err(FatError::Io);
        }
    }
    Ok(())
}

fn empty_devoptab() -> sysbase::Devoptab {
    sysbase::Devoptab {
        name: null_mut(),
        struct_size: 0,
        open_r: None,
        close_r: None,
        write_r: None,
        read_r: None,
        seek_r: None,
        fstat_r: None,
        stat_r: None,
        link_r: None,
        unlink_r: None,
        chdir_r: None,
        rename_r: None,
        mkdir_r: None,
        dir_state_size: 0,
        diropen_r: None,
        dirreset_r: None,
        dirnext_r: None,
        dirclose_r: None,
        statvfs_r: None,
        ftruncate_r: None,
        fsync_r: None,
        device_data: null_mut(),
        chmod_r: None,
        fchmod_r: None,
        rmdir_r: None,
        lstat_r: None,
        utimes_r: None,
        fpathconf_r: None,
        pathconf_r: None,
        symlink_r: None,
        readlink_r: None,
    }
}

fn normalize_mount_name(name: &CStr) -> Option<Vec<u8>> {
    let bytes = name.to_bytes();
    if bytes.is_empty() || bytes.len() > 8 || bytes.iter().any(|&b| b == b':' || b == 0) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.extend_from_slice(bytes);
    out.push(b':');
    out.push(0);
    Some(out)
}

fn normalize_lookup_name(name: &CStr) -> Option<Vec<u8>> {
    let bytes = name.to_bytes();
    if bytes.is_empty() || bytes.len() > 9 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.extend_from_slice(bytes);
    if bytes.last().copied() != Some(b':') {
        out.push(b':');
    }
    Some(out)
}

fn normalize_volume_lookup_name(name: &CStr) -> Option<Vec<u8>> {
    let mut bytes = name.to_bytes();
    while bytes.last().copied() == Some(b'/') {
        bytes = &bytes[..bytes.len() - 1];
    }
    if bytes.is_empty() {
        return None;
    }
    let mut owned = Vec::from(bytes);
    if owned.last().copied() != Some(b':') {
        owned.push(b':');
    }
    if owned.len() > 9 {
        return None;
    }
    Some(owned)
}

fn normalize_volume_label(label: &[u8; 11]) -> Vec<u8> {
    let end = label
        .iter()
        .rposition(|&b| b != b' ')
        .map_or(0, |idx| idx + 1);
    if label[..end].starts_with(b"NO NAME") {
        Vec::new()
    } else {
        label[..end].to_vec()
    }
}

fn root_volume_label(mount: &mut FatMount) -> Result<Option<[u8; 11]>, FatError> {
    let mut walk = DirectoryWalk::new(mount, mount.root_cluster)?;
    while let Some(sector) = walk.next_sector(mount, false)? {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        for entry_idx in 0..(mount.bytes_per_sector as usize / 32) {
            let offset = entry_idx * 32;
            let entry = &buffer[offset..offset + 32];
            if entry[0] == 0 {
                return Ok(None);
            }
            if entry[0] != 0xe5 && entry[11] == FAT_ATTR_VOLUME_ID {
                let mut label = [b' '; 11];
                label.copy_from_slice(&entry[..11]);
                return Ok(Some(label));
            }
        }
    }
    Ok(None)
}

fn parse_fat_mount_at(
    disc: &'static DiscInterface,
    read_only: bool,
    requested_start_sector: u32,
) -> Result<FatMount, FatError> {
    let mut sector0 = [0u8; MAX_SECTOR_BYTES];
    read_disc_sector(disc, requested_start_sector, &mut sector0)?;

    let partition_lba = if requested_start_sector != 0 {
        requested_start_sector
    } else if parse_bpb(&sector0).is_ok() {
        0
    } else {
        find_first_valid_partition(disc, &sector0)?
    };

    let mut bpb = [0u8; MAX_SECTOR_BYTES];
    read_disc_sector(disc, partition_lba, &mut bpb)?;
    let parsed = parse_bpb(&bpb)?;
    partition_lba
        .checked_add(parsed.total_sectors)
        .ok_or(FatError::InvalidBootSector)?;

    let fat_sectors = (parsed.fat_count as u32)
        .checked_mul(parsed.sectors_per_fat)
        .ok_or(FatError::InvalidBootSector)?;
    let metadata_sectors = (parsed.reserved_sectors as u32)
        .checked_add(fat_sectors)
        .and_then(|value| value.checked_add(parsed.root_dir_sectors))
        .ok_or(FatError::InvalidBootSector)?;
    let data_sectors = parsed
        .total_sectors
        .checked_sub(metadata_sectors)
        .ok_or(FatError::InvalidBootSector)?;
    let cluster_count = data_sectors / parsed.sectors_per_cluster as u32;
    let required_fat_entries = cluster_count
        .checked_add(2)
        .ok_or(FatError::InvalidBootSector)?;
    let fat_bytes = parsed
        .sectors_per_fat
        .checked_mul(parsed.bytes_per_sector as u32)
        .ok_or(FatError::InvalidBootSector)?;
    let fat_entry_capacity = parsed
        .fat_type
        .fat_entry_capacity(fat_bytes)
        .ok_or(FatError::InvalidBootSector)?;
    if cluster_count == 0 || required_fat_entries > fat_entry_capacity {
        return Err(FatError::InvalidBootSector);
    }
    if parsed.fat_type == FatType::Fat32
        && (parsed.root_cluster < 2 || parsed.root_cluster >= required_fat_entries)
    {
        return Err(FatError::InvalidBootSector);
    }

    let first_fat_sector = partition_lba
        .checked_add(parsed.reserved_sectors as u32)
        .ok_or(FatError::InvalidBootSector)?;
    let root_dir_start = first_fat_sector
        .checked_add(fat_sectors)
        .ok_or(FatError::InvalidBootSector)?;
    let first_data_sector = root_dir_start
        .checked_add(parsed.root_dir_sectors)
        .ok_or(FatError::InvalidBootSector)?;

    let fs_info_sector = parsed
        .fs_info_relative
        .map(|relative| {
            partition_lba
                .checked_add(u32::from(relative))
                .ok_or(FatError::InvalidBootSector)
        })
        .transpose()?;

    let mut mount = FatMount {
        disc,
        read_only,
        online: AtomicBool::new(true),
        media_generation: AtomicU32::new(1),
        fat_type: parsed.fat_type,
        bytes_per_sector: parsed.bytes_per_sector,
        sectors_per_cluster: parsed.sectors_per_cluster,
        fat_count: parsed.fat_count,
        sectors_per_fat: parsed.sectors_per_fat,
        root_cluster: parsed.root_cluster,
        root_dir_start,
        root_dir_sectors: parsed.root_dir_sectors,
        first_fat_sector,
        active_fat: parsed.active_fat,
        mirror_fats: parsed.mirror_fats,
        first_data_sector,
        cluster_count,
        fat_cache: RefCell::new(sector_cache::SectorCache::default()),
        next_free_hint: 2,
        last_alloc_cluster: 0,
        fs_info_sector,
        boot_label: parsed.boot_label,
        cwd_cluster: parsed.root_cluster,
        open_files: BTreeMap::new(),
        open_dirs: BTreeMap::new(),
    };
    mount.load_or_create_fs_info()?;
    Ok(mount)
}

struct ParsedBpb {
    fat_type: FatType,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
    root_dir_sectors: u32,
    total_sectors: u32,
    active_fat: u8,
    mirror_fats: bool,
    fs_info_relative: Option<u16>,
    boot_label: [u8; 11],
}

fn find_first_valid_partition(disc: &'static DiscInterface, mbr: &[u8]) -> Result<u32, FatError> {
    if mbr[510] != 0x55 || mbr[511] != 0xaa {
        return Err(FatError::InvalidBootSector);
    }

    // Match libfat's FindFirstValidPartition: inspect every non-empty primary
    // entry, validate the candidate boot sector rather than trusting the MBR
    // type byte, and follow an extended-partition chain for up to eight logical
    // partitions.
    for entry_idx in 0..4 {
        let entry = &mbr[446 + entry_idx * 16..446 + (entry_idx + 1) * 16];
        let part_type = entry[4];
        if part_type == 0 {
            continue;
        }
        let base_lba = u32::from_le_bytes(entry[8..12].try_into().unwrap());
        if base_lba == 0 {
            continue;
        }

        if part_type == 0x0f {
            let mut next_ebr = 0u32;
            for _ in 0..8 {
                let ebr_lba = base_lba
                    .checked_add(next_ebr)
                    .ok_or(FatError::InvalidBootSector)?;
                let mut ebr = [0u8; MAX_SECTOR_BYTES];
                read_disc_sector(disc, ebr_lba, &mut ebr)?;
                if ebr[510] != 0x55 || ebr[511] != 0xaa {
                    break;
                }

                let logical_rel = u32::from_le_bytes(ebr[454..458].try_into().unwrap());
                if logical_rel != 0 {
                    let logical_lba = ebr_lba
                        .checked_add(logical_rel)
                        .ok_or(FatError::InvalidBootSector)?;
                    let mut boot = [0u8; MAX_SECTOR_BYTES];
                    read_disc_sector(disc, logical_lba, &mut boot)?;
                    if parse_bpb(&boot).is_ok() {
                        return Ok(logical_lba);
                    }
                }

                next_ebr = u32::from_le_bytes(ebr[470..474].try_into().unwrap());
                if next_ebr == 0 {
                    break;
                }
            }
            continue;
        }

        let mut boot = [0u8; MAX_SECTOR_BYTES];
        read_disc_sector(disc, base_lba, &mut boot)?;
        if parse_bpb(&boot).is_ok() {
            return Ok(base_lba);
        }
    }

    Err(FatError::UnsupportedVariant)
}

fn parse_bpb(sector: &[u8]) -> Result<ParsedBpb, FatError> {
    if sector[510] != 0x55 || sector[511] != 0xaa {
        return Err(FatError::InvalidBootSector);
    }

    let bytes_per_sector = u16::from_le_bytes(sector[11..13].try_into().unwrap());
    let sectors_per_cluster = sector[13];
    let reserved_sectors = u16::from_le_bytes(sector[14..16].try_into().unwrap());
    let fat_count = sector[16];
    let root_entry_count = u16::from_le_bytes(sector[17..19].try_into().unwrap()) as u32;
    let total16 = u16::from_le_bytes(sector[19..21].try_into().unwrap()) as u32;
    let fat16 = u16::from_le_bytes(sector[22..24].try_into().unwrap()) as u32;
    let total32 = u32::from_le_bytes(sector[32..36].try_into().unwrap());
    let fat32 = u32::from_le_bytes(sector[36..40].try_into().unwrap());

    if !(MIN_SECTOR_BYTES as u16..=MAX_SECTOR_BYTES as u16).contains(&bytes_per_sector)
        || !bytes_per_sector.is_power_of_two()
        || sectors_per_cluster == 0
        || !sectors_per_cluster.is_power_of_two()
        || reserved_sectors == 0
        || fat_count == 0
    {
        return Err(FatError::UnsupportedVariant);
    }

    let sectors_per_fat = if fat16 != 0 { fat16 } else { fat32 };
    let total_sectors = if total16 != 0 { total16 } else { total32 };
    if sectors_per_fat == 0 || total_sectors == 0 {
        return Err(FatError::InvalidBootSector);
    }

    let root_bytes = root_entry_count
        .checked_mul(32)
        .ok_or(FatError::InvalidBootSector)?;
    if root_bytes % bytes_per_sector as u32 != 0 {
        // libfat assumes an integral fixed-root sector span. Reject malformed
        // media rather than silently rounding and reading outside the root area.
        return Err(FatError::InvalidBootSector);
    }
    let root_dir_sectors = root_bytes / bytes_per_sector as u32;
    let metadata = (reserved_sectors as u32)
        .checked_add(
            (fat_count as u32)
                .checked_mul(sectors_per_fat)
                .ok_or(FatError::InvalidBootSector)?,
        )
        .and_then(|value| value.checked_add(root_dir_sectors))
        .ok_or(FatError::InvalidBootSector)?;
    let data_sectors = total_sectors
        .checked_sub(metadata)
        .ok_or(FatError::InvalidBootSector)?;
    let cluster_count = data_sectors / sectors_per_cluster as u32;
    let fat_type = if cluster_count < FAT12_CLUSTER_LIMIT {
        FatType::Fat12
    } else if cluster_count < FAT16_CLUSTER_LIMIT {
        FatType::Fat16
    } else {
        FatType::Fat32
    };

    let label_offset = if fat_type == FatType::Fat32 { 71 } else { 43 };
    let mut boot_label = [b' '; 11];
    boot_label.copy_from_slice(&sector[label_offset..label_offset + 11]);

    let (root_cluster, active_fat, mirror_fats, fs_info_relative) = if fat_type == FatType::Fat32 {
        if root_entry_count != 0 || fat16 != 0 || fat32 == 0 {
            return Err(FatError::InvalidBootSector);
        }
        let root_cluster = u32::from_le_bytes(sector[44..48].try_into().unwrap());
        let ext_flags = u16::from_le_bytes(sector[40..42].try_into().unwrap());
        let mirror_fats = ext_flags & 0x0080 == 0;
        let active_fat = (ext_flags & 0x000f) as u8;
        if root_cluster < 2 || (!mirror_fats && active_fat >= fat_count) {
            return Err(FatError::InvalidBootSector);
        }
        let fs_info = u16::from_le_bytes(sector[48..50].try_into().unwrap());
        let fs_info = if fs_info == 0 { 1 } else { fs_info };
        (root_cluster, active_fat, mirror_fats, Some(fs_info))
    } else {
        if root_entry_count == 0 || fat16 == 0 {
            return Err(FatError::InvalidBootSector);
        }
        (0, 0, true, None)
    };

    Ok(ParsedBpb {
        fat_type,
        bytes_per_sector,
        sectors_per_cluster,
        reserved_sectors,
        fat_count,
        sectors_per_fat,
        root_cluster,
        root_dir_sectors,
        total_sectors,
        active_fat,
        mirror_fats,
        fs_info_relative,
        boot_label,
    })
}

fn read_disc_sector(disc: &DiscInterface, lba: u32, buffer: &mut [u8]) -> Result<(), FatError> {
    let Some(read_sectors) = disc.read_sectors else {
        return Err(FatError::Io);
    };
    if read_disc_sector_aligned(read_sectors, lba, MIN_SECTOR_BYTES, buffer) {
        Ok(())
    } else {
        Err(FatError::Io)
    }
}

fn read_disc_sector_aligned(
    read_sectors: unsafe extern "C" fn(u32, u32, *mut c_void) -> bool,
    lba: u32,
    sector_bytes: usize,
    buffer: &mut [u8],
) -> bool {
    if !(MIN_SECTOR_BYTES..=MAX_SECTOR_BYTES).contains(&sector_bytes)
        || !sector_bytes.is_power_of_two()
        || buffer.len() < sector_bytes
    {
        return false;
    }
    let mut sector = AlignedSector([0; MAX_SECTOR_BYTES]);
    // Disc backends may DMA directly into this address. Keep it 32-byte
    // aligned even when the caller supplies a byte-aligned stack buffer.
    if !unsafe { read_sectors(lba, 1, sector.0.as_mut_ptr().cast()) } {
        return false;
    }
    buffer[..sector_bytes].copy_from_slice(&sector.0[..sector_bytes]);
    true
}

unsafe extern "C" fn fat_open_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    path: *const c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_mut() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if path.is_null() {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        }
        let path = { CStr::from_ptr(path) };
        match open_file_from_path(path, flags, file_state) {
            Ok(()) => 0,
            Err(error) => {
                set_fat_errno(r, error);
                -1
            }
        }
    }
}

fn open_file_from_path(
    path: &CStr,
    flags: i32,
    file_state: &mut FatFileState,
) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let writable = match flags & sysbase::O_ACCMODE {
        sysbase::O_RDONLY => false,
        sysbase::O_WRONLY | sysbase::O_RDWR => true,
        _ => return Err(FatError::PermissionDenied),
    };
    if writable && mount.read_only {
        return Err(FatError::ReadOnly);
    }
    let (parent_cluster, leaf_name) = resolve_parent_directory(&mut mount, relative)?;
    let existing = find_entry_in_directory(&mut mount, parent_cluster, leaf_name)?;
    let entry = if let Some(entry) = existing {
        if flags & sysbase::O_CREAT != 0 && flags & sysbase::O_EXCL != 0 {
            return Err(FatError::AlreadyExists);
        }
        if entry.attr & FAT_ATTR_DIRECTORY != 0 {
            return Err(FatError::IsDirectory);
        }
        if writable && entry.attr & ATTR_READONLY != 0 {
            return Err(FatError::ReadOnly);
        }
        if flags & sysbase::O_TRUNC != 0 && writable {
            if mount.file_is_open(entry.short_sector, entry.short_offset) {
                return Err(FatError::Busy);
            }
            mount.update_short_entry(entry.short_sector, entry.short_offset, 0, 0)?;
            free_cluster_chain(&mut mount, entry.start_cluster)?;
            DirEntry {
                start_cluster: 0,
                size: 0,
                ..entry
            }
        } else {
            entry
        }
    } else {
        if flags & sysbase::O_CREAT == 0 {
            return Err(FatError::NotFound);
        }
        create_file_entry(&mut mount, parent_cluster, leaf_name)?
    };

    if flags & (sysbase::O_CREAT | sysbase::O_TRUNC) != 0 {
        mount.flush_transport()?;
    }

    mount.acquire_file(entry.short_sector, entry.short_offset, writable, entry.size)?;
    file_state.magic = FILE_MAGIC;
    file_state.mount = mount.lock as *const SpinLock<FatMount>;
    file_state.dir_sector = entry.short_sector;
    file_state.dir_offset = entry.short_offset;
    file_state.flags = flags;
    file_state.start_cluster = entry.start_cluster;
    file_state.size = entry.size;
    file_state.position = if flags & sysbase::O_APPEND != 0 {
        entry.size
    } else {
        0
    };
    file_state.dirty = 0;
    Ok(())
}

unsafe extern "C" fn fat_close_r(r: *mut sysbase::Reent, file_struct: *mut c_void) -> i32 {
    let Some(state) = (unsafe { file_struct.cast::<FatFileState>().as_mut() }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    match close_file_state(state).inspect_err(|error| record_file_close_failure(*error)) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

/// Flush and release once, including when the medium or transport fails.
fn close_file_state(state: &mut FatFileState) -> Result<(), FatError> {
    if state.magic != FILE_MAGIC {
        return Err(FatError::BadDescriptor);
    }
    file_transfer::close(state, flush_file_state, release_file_state)
}

/// Release ownership without I/O. Invalidate the handle before removing its
/// last mount reference, so subsequent Drop cannot access an unmounted device.
fn release_file_state(state: &mut FatFileState) {
    let lock = unsafe { state.mount.as_ref() };
    state.magic = 0;
    state.mount = core::ptr::null();
    if let Some(lock) = lock {
        lock.lock().release_file(
            state.dir_sector,
            state.dir_offset,
            state.flags & (sysbase::O_WRONLY | sysbase::O_RDWR) != 0,
        );
    }
}

fn flush_file_state(file_state: &mut FatFileState) -> Result<(), FatError> {
    if file_state.magic != FILE_MAGIC {
        return Err(FatError::BadDescriptor);
    }
    let lock = unsafe { file_state.mount.as_ref() }.ok_or(FatError::BadDescriptor)?;
    let mount = lock.lock();
    let (sector, offset, start, size) = (
        file_state.dir_sector,
        file_state.dir_offset,
        file_state.start_cluster,
        file_state.size,
    );
    // Clean files still require the USB cache command. SD sector writes wait
    // for transfer completion; there is no separate SD flush command.
    file_transfer::flush(
        &mut file_state.dirty,
        || mount.ensure_online(),
        || mount.update_short_entry(sector, offset, start, size),
        || mount.flush_transport(),
    )
}

unsafe extern "C" fn fat_fsync_r(r: *mut sysbase::Reent, file_struct: *mut c_void) -> i32 {
    let Some(state) = (unsafe { file_struct.cast::<FatFileState>().as_mut() }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    if state.magic != FILE_MAGIC {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    }
    match flush_file_state(state) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_write_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    ptr: *const c_char,
    len: usize,
) -> isize {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_mut() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if file_state.magic != FILE_MAGIC
            || file_state.flags & (sysbase::O_WRONLY | sysbase::O_RDWR) == 0
        {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        }
        if len == 0 {
            return 0;
        }
        if ptr.is_null() {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        }
        let Some(mount_lock) = file_state.mount.as_ref() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        let mut mount = mount_lock.lock();
        if file_state.flags & sysbase::O_APPEND != 0 {
            file_state.position = file_state.size;
        }
        let max_len = (u32::MAX - file_state.position) as usize;
        let len = min(len, max_len);
        if len == 0 {
            sysbase::set_reent_errno(r, sysbase::EFBIG);
            return -1;
        }
        let bytes = { core::slice::from_raw_parts(ptr.cast::<u8>(), len) };
        match write_file_bytes(&mut mount, file_state, bytes) {
            Ok(written) => written as isize,
            Err(error) => {
                set_fat_errno(r, error);
                -1
            }
        }
    }
}

fn write_file_bytes(
    mount: &mut FatMount,
    file_state: &mut FatFileState,
    bytes: &[u8],
) -> Result<usize, FatError> {
    if file_state.position > file_state.size {
        let mut remaining = file_state.position - file_state.size;
        file_state.position = file_state.size;
        let zeroes = [0u8; MAX_SECTOR_BYTES];
        while remaining != 0 {
            let chunk = min(remaining as usize, zeroes.len());
            let written = write_file_bytes(mount, file_state, &zeroes[..chunk])?;
            remaining -= written as u32;
        }
    }

    let cluster_size = mount.cluster_size();
    file_transfer::transfer(bytes.len(), FatError::Io, |written| {
        let offset = file_state.position;
        let cluster_index = offset / cluster_size;
        let within_cluster = offset % cluster_size;
        let cluster = file_transfer::allocation(
            &mut file_state.start_cluster,
            &mut file_state.dirty,
            |start| mount.nth_cluster(start, cluster_index, true),
        )?;
        let cluster_sector = mount.cluster_to_sector(cluster)?;
        let sector_delta = within_cluster / mount.bytes_per_sector as u32;
        let sector_offset = (within_cluster % mount.bytes_per_sector as u32) as usize;
        let sector = cluster_sector + sector_delta;
        let chunk = min(
            bytes.len() - written,
            mount.bytes_per_sector as usize - sector_offset,
        );

        let mut sector_buf = [0u8; MAX_SECTOR_BYTES];
        if sector_offset != 0 || chunk != mount.bytes_per_sector as usize {
            mount.read_sector(sector, &mut sector_buf)?;
        }
        sector_buf[sector_offset..sector_offset + chunk]
            .copy_from_slice(&bytes[written..written + chunk]);
        mount.write_sector(sector, &sector_buf)?;

        file_state.position += chunk as u32;
        if file_state.position > file_state.size {
            file_state.size = file_state.position;
        }
        file_state.dirty = 1;
        mount.publish_file_size(file_state)?;
        Ok(chunk)
    })
}

unsafe extern "C" fn fat_read_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    ptr: *mut c_char,
    len: usize,
) -> isize {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_mut() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if file_state.magic != FILE_MAGIC || file_state.flags & sysbase::O_WRONLY != 0 {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        }
        if len == 0 {
            return 0;
        }
        if ptr.is_null() {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        }
        let Some(mount_lock) = file_state.mount.as_ref() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        let at_or_past_eof = file_state.position >= file_state.size;
        let remaining = file_state.size.saturating_sub(file_state.position) as usize;
        let truncated_at_eof = len > remaining;
        let mut mount = mount_lock.lock();
        let bytes = { core::slice::from_raw_parts_mut(ptr.cast::<u8>(), len) };
        match read_file_bytes(&mut mount, file_state, bytes) {
            Ok(read) => {
                if at_or_past_eof || truncated_at_eof {
                    // libfat returns the bytes read while reporting that the request crossed EOF.
                    sysbase::set_reent_errno(r, sysbase::EOVERFLOW);
                }
                read as isize
            }
            Err(error) => {
                set_fat_errno(r, error);
                -1
            }
        }
    }
}

fn read_file_bytes(
    mount: &mut FatMount,
    file_state: &mut FatFileState,
    out: &mut [u8],
) -> Result<usize, FatError> {
    if file_state.position >= file_state.size {
        return Ok(0);
    }

    let cluster_size = mount.cluster_size();
    let total = min(out.len(), (file_state.size - file_state.position) as usize);
    file_transfer::transfer(total, FatError::Io, |copied| {
        let offset = file_state.position;
        let cluster_index = offset / cluster_size;
        let within_cluster = offset % cluster_size;
        let mut start_cluster = file_state.start_cluster;
        let cluster = mount.nth_cluster(&mut start_cluster, cluster_index, false)?;
        let cluster_sector = mount.cluster_to_sector(cluster)?;
        let sector_delta = within_cluster / mount.bytes_per_sector as u32;
        let sector_offset = (within_cluster % mount.bytes_per_sector as u32) as usize;
        let sector = cluster_sector + sector_delta;
        let chunk = min(
            total - copied,
            mount.bytes_per_sector as usize - sector_offset,
        );

        let mut sector_buf = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut sector_buf)?;
        out[copied..copied + chunk]
            .copy_from_slice(&sector_buf[sector_offset..sector_offset + chunk]);

        file_state.position += chunk as u32;
        Ok(chunk)
    })
}

unsafe extern "C" fn fat_seek_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    pos: i64,
    dir: i32,
) -> i64 {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_mut() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if file_state.magic != FILE_MAGIC {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        }
        let base = match dir {
            sysbase::SEEK_SET => 0i64,
            sysbase::SEEK_CUR => file_state.position as i64,
            sysbase::SEEK_END => file_state.size as i64,
            _ => {
                sysbase::set_reent_errno(r, sysbase::EINVAL);
                return -1;
            }
        };
        let Some(next) = base.checked_add(pos) else {
            sysbase::set_reent_errno(r, sysbase::EOVERFLOW);
            return -1;
        };
        if next < 0 || next > i64::from(u32::MAX) {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        }
        let next = next as u32;
        file_state.position = next;
        i64::from(file_state.position)
    }
}

unsafe extern "C" fn fat_fstat_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    st: *mut c_void,
) -> i32 {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_ref() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if file_state.magic != FILE_MAGIC {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        }
        let Some(stat) = ({ st.cast::<Stat>().as_mut() }) else {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        };
        let Some(mount_lock) = file_state.mount.as_ref() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        let mut mount = mount_lock.lock();
        let mut sector = [0u8; MAX_SECTOR_BYTES];
        if let Err(error) = mount.read_sector(file_state.dir_sector, &mut sector) {
            set_fat_errno(r, error);
            return -1;
        }
        let attr = sector[file_state.dir_offset as usize + 11];
        let mut mode = S_IFREG | S_IRUSR | S_IRGRP | S_IROTH;
        if attr & ATTR_READONLY == 0 {
            mode |= S_IWUSR | S_IWGRP | S_IWOTH;
        }
        *stat = build_stat(
            mode,
            file_state.size as i64,
            cluster_blocks(&mut mount, file_state.start_cluster) as i64,
            mount.bytes_per_sector,
        );
        0
    }
}

unsafe extern "C" fn fat_stat_r(
    r: *mut sysbase::Reent,
    path: *const c_char,
    st: *mut c_void,
) -> i32 {
    if path.is_null() || st.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    let path = unsafe { CStr::from_ptr(path) };
    let stat = unsafe { &mut *st.cast::<Stat>() };
    match stat_path(path, stat) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

fn strip_mount_prefix(path: &[u8]) -> Option<&[u8]> {
    let colon = path.iter().position(|&b| b == b':')?;
    Some(&path[colon + 1..])
}

fn path_is_root(path: &[u8]) -> bool {
    path.is_empty() || path.iter().all(|&byte| byte == b'/')
}

fn path_start_cluster(mount: &FatMount, path: &[u8]) -> u32 {
    if path.first().copied() == Some(b'/') {
        mount.root_cluster
    } else {
        mount.cwd_cluster
    }
}

fn directory_parent_cluster(mount: &mut FatMount, cluster: u32) -> Result<u32, FatError> {
    if cluster == mount.root_cluster || mount.is_fixed_root(cluster) {
        return Ok(mount.root_cluster);
    }
    let sector = mount.cluster_to_sector(cluster)?;
    let mut buffer = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut buffer)?;
    let dotdot = &buffer[32..64];
    if dotdot[0] != b'.' || dotdot[1] != b'.' || dotdot[11] & FAT_ATTR_DIRECTORY == 0 {
        return Err(FatError::Io);
    }
    let low = u16::from_le_bytes([dotdot[26], dotdot[27]]) as u32;
    let parent = if mount.fat_type == FatType::Fat32 {
        let high = u16::from_le_bytes([dotdot[20], dotdot[21]]) as u32;
        (high << 16) | low
    } else {
        low
    };
    if parent == 0 {
        Ok(mount.root_cluster)
    } else if parent >= 2 && parent <= mount.cluster_count + 1 {
        Ok(parent)
    } else {
        Err(FatError::Io)
    }
}

fn resolve_parent_directory<'a>(
    mount: &mut FatMount,
    relative: &'a [u8],
) -> Result<(u32, &'a [u8]), FatError> {
    let mut current = path_start_cluster(mount, relative);
    let mut parts = relative
        .split(|&b| b == b'/')
        .filter(|part| !part.is_empty())
        .peekable();
    let mut leaf = None;

    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            leaf = Some(part);
            break;
        }
        if part == b"." {
            continue;
        }
        if part == b".." {
            current = directory_parent_cluster(mount, current)?;
            continue;
        }
        let entry = find_entry_in_directory(mount, current, part)?.ok_or(FatError::NotFound)?;
        if entry.attr & FAT_ATTR_DIRECTORY == 0 {
            return Err(FatError::NotDirectory);
        }
        current = entry.start_cluster;
    }

    let leaf = leaf.ok_or(FatError::PathError)?;
    if leaf.is_empty() || leaf == b"." || leaf == b".." {
        return Err(FatError::PathError);
    }
    Ok((current, leaf))
}

fn find_entry_in_directory(
    mount: &mut FatMount,
    start_cluster: u32,
    target_name: &[u8],
) -> Result<Option<DirEntry>, FatError> {
    let target = ascii_fold(target_name);
    let mut lfn_stack: Vec<LfnEntry> = Vec::new();
    let mut lfn_positions: Vec<EntryPos> = Vec::new();
    let mut walk = DirectoryWalk::new(mount, start_cluster)?;

    while let Some(sector) = walk.next_sector(mount, false)? {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        for entry_idx in 0..(mount.bytes_per_sector as usize / 32) {
            let offset = entry_idx * 32;
            let entry = &buffer[offset..offset + 32];
            let marker = entry[0];
            if marker == 0xe5 {
                lfn_stack.clear();
                lfn_positions.clear();
                continue;
            }
            if entry[11] == FAT_ATTR_LFN {
                lfn_stack.push(parse_lfn_entry(entry));
                lfn_positions.push(EntryPos {
                    sector,
                    offset: offset as u16,
                });
                continue;
            }
            if marker == 0 {
                return Ok(None);
            }
            if entry[11] & FAT_ATTR_VOLUME_ID != 0 {
                lfn_stack.clear();
                lfn_positions.clear();
                continue;
            }

            let (name, valid_lfn) = decode_entry_name(entry, &lfn_stack);
            if ascii_fold(&name) == target
                || short_name_display(entry[..11].try_into().map_err(|_| FatError::Io)?)
                    .is_some_and(|short| ascii_fold(short.as_bytes()) == target)
            {
                return Ok(Some(build_dir_entry(
                    mount.fat_type,
                    entry,
                    name,
                    sector,
                    offset as u16,
                    if valid_lfn { &lfn_positions } else { &[] },
                )));
            }
            lfn_stack.clear();
            lfn_positions.clear();
        }
    }
    Ok(None)
}

fn create_file_entry(
    mount: &mut FatMount,
    dir_cluster: u32,
    name: &[u8],
) -> Result<DirEntry, FatError> {
    validate_fat_name(name)?;
    let short_name = build_short_name(mount, dir_cluster, name)?;
    let needs_lfn = short_name_display(&short_name)
        .map(|display| ascii_fold(display.as_bytes()) != ascii_fold(name))
        .unwrap_or(true);
    let lfn_entries = if needs_lfn {
        build_lfn_entries(name, &short_name)?
    } else {
        Vec::new()
    };
    let needed = lfn_entries.len() + 1;
    let slots = find_free_slots(mount, dir_cluster, needed)?.ok_or(FatError::NoSpace)?;
    let short_sector = slots[lfn_entries.len()].sector;
    let short_offset = slots[lfn_entries.len()].offset;

    let mut written = Vec::new();
    for (idx, lfn) in lfn_entries.iter().enumerate() {
        if let Err(error) = write_entry_bytes(mount, slots[idx].sector, slots[idx].offset, lfn) {
            let _ = mark_entry_deleted(mount, &written);
            return Err(error);
        }
        written.push(slots[idx].clone());
    }

    let short_entry = build_short_entry(&short_name, 0, 0);
    if let Err(error) = write_entry_bytes(
        mount,
        slots[lfn_entries.len()].sector,
        slots[lfn_entries.len()].offset,
        &short_entry,
    ) {
        let _ = mark_entry_deleted(mount, &written);
        return Err(error);
    }

    Ok(DirEntry {
        name: name.to_vec(),
        attr: FAT_ATTR_ARCHIVE,
        start_cluster: 0,
        size: 0,
        entry_span: slots.clone(),
        short_sector,
        short_offset,
    })
}

fn find_free_slots(
    mount: &mut FatMount,
    start_cluster: u32,
    needed: usize,
) -> Result<Option<Vec<EntryPos>>, FatError> {
    let mut run = Vec::new();
    let mut walk = DirectoryWalk::new(mount, start_cluster)?;
    while let Some(sector) = walk.next_sector(mount, true)? {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        for entry_idx in 0..(mount.bytes_per_sector as usize / 32) {
            let offset = entry_idx * 32;
            let marker = buffer[offset];
            if marker == 0 || marker == 0xe5 {
                run.push(EntryPos {
                    sector,
                    offset: offset as u16,
                });
                if run.len() == needed {
                    return Ok(Some(run));
                }
            } else {
                run.clear();
            }
        }
        if walk.fixed_root && walk.sector_index >= mount.root_dir_sectors {
            return Ok(None);
        }
    }
    Ok(None)
}

fn write_entry_bytes(
    mount: &FatMount,
    sector: u32,
    offset: u16,
    entry: &[u8; 32],
) -> Result<(), FatError> {
    let mut buffer = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut buffer)?;
    let offset = offset as usize;
    buffer[offset..offset + 32].copy_from_slice(entry);
    mount.write_sector(sector, &buffer)
}

fn build_dir_entry(
    fat_type: FatType,
    entry: &[u8],
    name: Vec<u8>,
    sector: u32,
    offset: u16,
    lfn_positions: &[EntryPos],
) -> DirEntry {
    let low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
    let start_cluster = if fat_type == FatType::Fat32 {
        let high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
        (high << 16) | low
    } else {
        low
    };
    let size = u32::from_le_bytes(entry[28..32].try_into().unwrap());
    let mut entry_span = lfn_positions.to_vec();
    entry_span.push(EntryPos { sector, offset });
    DirEntry {
        name,
        attr: entry[11],
        start_cluster,
        size,
        entry_span,
        short_sector: sector,
        short_offset: offset,
    }
}

fn decode_entry_name(entry: &[u8], lfn_stack: &[LfnEntry]) -> (Vec<u8>, bool) {
    if valid_lfn_stack(entry, lfn_stack) {
        let mut out = Vec::new();
        for part in lfn_stack.iter().rev() {
            for &ch in &part.chars {
                if ch == 0 || ch == 0xffff {
                    return (out, true);
                }
                if ch > 0x7f {
                    return (short_name_bytes(&entry[..11]), false);
                }
                out.push(ch as u8);
            }
        }
        return (out, true);
    }
    (short_name_bytes(&entry[..11]), false)
}

fn valid_lfn_stack(short_entry: &[u8], lfn_stack: &[LfnEntry]) -> bool {
    let Some(first) = lfn_stack.first() else {
        return false;
    };
    let count = lfn_stack.len();
    if count > 20 || first.ordinal & 0x40 == 0 || usize::from(first.ordinal & 0x1f) != count {
        return false;
    }
    let checksum = short_name_checksum(short_entry[..11].try_into().unwrap());
    lfn_stack.iter().enumerate().all(|(index, part)| {
        let expected = (count - index) as u8;
        part.ordinal & 0x1f == expected
            && part.ordinal & 0x40 == if index == 0 { 0x40 } else { 0 }
            && part.ordinal & 0xa0 == 0
            && part.checksum == checksum
            && part.format_valid
    })
}

fn parse_lfn_entry(entry: &[u8]) -> LfnEntry {
    let mut chars = [0u16; 13];
    let positions = [(1, 5), (14, 6), (28, 2)];
    let mut idx = 0;
    for (start, count) in positions {
        for n in 0..count {
            let pos = start + n * 2;
            chars[idx] = u16::from_le_bytes([entry[pos], entry[pos + 1]]);
            idx += 1;
        }
    }
    LfnEntry {
        ordinal: entry[0],
        checksum: entry[13],
        format_valid: entry[12] == 0 && entry[26] == 0 && entry[27] == 0,
        chars,
    }
}

fn build_lfn_entries(name: &[u8], short_name: &[u8; 11]) -> Result<Vec<[u8; 32]>, FatError> {
    validate_fat_name(name)?;
    let checksum = short_name_checksum(short_name);
    let chars: Vec<u16> = name.iter().map(|&b| b as u16).collect();
    let count = chars.len().div_ceil(13);
    let mut out = Vec::with_capacity(count);

    for idx in (0..count).rev() {
        let start = idx * 13;
        let end = min(chars.len(), start + 13);
        let mut entry = [0xffu8; 32];
        entry[11] = FAT_ATTR_LFN;
        entry[12] = 0;
        entry[13] = checksum;
        entry[26] = 0;
        entry[27] = 0;

        let ord = (idx + 1) as u8;
        entry[0] = if idx == count - 1 { ord | 0x40 } else { ord };

        write_lfn_chars(
            &mut entry,
            &chars[start..end],
            idx == count - 1 && end == chars.len(),
        );
        out.push(entry);
    }

    Ok(out)
}

fn write_lfn_chars(entry: &mut [u8; 32], chars: &[u16], is_last_chunk: bool) {
    let positions = [(1, 5), (14, 6), (28, 2)];
    let mut idx = 0;
    for (start, count) in positions {
        for n in 0..count {
            let pos = start + n * 2;
            let value = if idx < chars.len() {
                chars[idx]
            } else if idx == chars.len() && is_last_chunk {
                0
            } else {
                0xffff
            };
            entry[pos..pos + 2].copy_from_slice(&value.to_le_bytes());
            idx += 1;
        }
    }
}

fn build_short_entry(short_name: &[u8; 11], start_cluster: u32, size: u32) -> [u8; 32] {
    let mut entry = [0u8; 32];
    entry[..11].copy_from_slice(short_name);
    entry[11] = FAT_ATTR_ARCHIVE;
    entry[20..22].copy_from_slice(&((start_cluster >> 16) as u16).to_le_bytes());
    entry[26..28].copy_from_slice(&(start_cluster as u16).to_le_bytes());
    entry[28..32].copy_from_slice(&size.to_le_bytes());
    entry
}

fn build_short_name(
    mount: &mut FatMount,
    dir_cluster: u32,
    name: &[u8],
) -> Result<[u8; 11], FatError> {
    let (base, ext) = split_name(name);
    let base_clean = sanitize_short_component(base, 8);
    let ext_clean = sanitize_short_component(ext, 3);
    let exact = is_short_name_compatible(base, ext, &base_clean, &ext_clean);

    if exact {
        let mut short = [b' '; 11];
        short[..base_clean.len()].copy_from_slice(&base_clean);
        short[8..8 + ext_clean.len()].copy_from_slice(&ext_clean);
        if !short_name_exists(mount, dir_cluster, &short)? {
            return Ok(short);
        }
    }

    let mut base_part = sanitize_short_component(base, 8);
    if base_part.is_empty() {
        base_part.push(b'_');
    }
    for suffix in 1..=999_999u32 {
        let mut digits = [0u8; 6];
        let mut cursor = digits.len();
        let mut value = suffix;
        while value != 0 {
            cursor -= 1;
            digits[cursor] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        let digits = &digits[cursor..];
        let prefix_len = min(base_part.len(), 8 - 1 - digits.len());
        let mut short = [b' '; 11];
        short[..prefix_len].copy_from_slice(&base_part[..prefix_len]);
        short[prefix_len] = b'~';
        short[prefix_len + 1..prefix_len + 1 + digits.len()].copy_from_slice(digits);
        short[8..8 + ext_clean.len()].copy_from_slice(&ext_clean);
        if !short_name_exists(mount, dir_cluster, &short)? {
            return Ok(short);
        }
    }

    Err(FatError::NoSpace)
}

fn validate_fat_name(name: &[u8]) -> Result<(), FatError> {
    if name.is_empty()
        || name.len() > NAME_MAX
        || name.last().is_some_and(|byte| matches!(byte, b' ' | b'.'))
        || name.iter().any(|byte| {
            *byte < 0x20
                || *byte >= 0x80
                || matches!(
                    *byte,
                    b'"' | b'*' | b'/' | b':' | b'<' | b'>' | b'?' | b'\\' | b'|'
                )
        })
    {
        return Err(FatError::PathError);
    }
    Ok(())
}

fn split_name(name: &[u8]) -> (&[u8], &[u8]) {
    if let Some(idx) = name.iter().rposition(|&b| b == b'.') {
        (&name[..idx], &name[idx + 1..])
    } else {
        (name, &[])
    }
}

fn sanitize_short_component(component: &[u8], max_len: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for &byte in component {
        if out.len() == max_len {
            break;
        }
        let upper = byte.to_ascii_uppercase();
        if matches!(
            upper,
            b'A'..=b'Z'
                | b'0'..=b'9'
                | b'$'
                | b'%'
                | b'\''
                | b'-'
                | b'_'
                | b'@'
                | b'~'
                | b'`'
                | b'!'
                | b'('
                | b')'
                | b'{'
                | b'}'
                | b'^'
                | b'#'
                | b'&'
        ) {
            out.push(upper);
        }
    }
    out
}

fn is_short_name_compatible(base: &[u8], ext: &[u8], base_clean: &[u8], ext_clean: &[u8]) -> bool {
    !base.is_empty()
        && base.len() <= 8
        && ext.len() <= 3
        && base_clean.len() == base.len()
        && ext_clean.len() == ext.len()
}

fn short_name_exists(
    mount: &mut FatMount,
    dir_cluster: u32,
    target: &[u8; 11],
) -> Result<bool, FatError> {
    let mut walk = DirectoryWalk::new(mount, dir_cluster)?;
    while let Some(sector) = walk.next_sector(mount, false)? {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        for entry_idx in 0..(mount.bytes_per_sector as usize / 32) {
            let offset = entry_idx * 32;
            let entry = &buffer[offset..offset + 32];
            if entry[0] == 0 {
                return Ok(false);
            }
            if entry[0] != 0xe5 && entry[11] != FAT_ATTR_LFN && &entry[..11] == target {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn short_name_checksum(short_name: &[u8; 11]) -> u8 {
    let mut sum = 0u8;
    for &byte in short_name {
        sum = ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(byte);
    }
    sum
}

fn short_name_display(short_name: &[u8; 11]) -> Option<alloc::string::String> {
    let base_end = short_name[..8]
        .iter()
        .rposition(|&b| b != b' ')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let ext_end = short_name[8..]
        .iter()
        .rposition(|&b| b != b' ')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let base = core::str::from_utf8(&short_name[..base_end]).ok()?;
    let ext = core::str::from_utf8(&short_name[8..8 + ext_end]).ok()?;
    let mut out = alloc::string::String::from(base);
    if !ext.is_empty() {
        out.push('.');
        out.push_str(ext);
    }
    Some(out)
}

fn short_name_bytes(short_name: &[u8]) -> Vec<u8> {
    let base_end = short_name[..8]
        .iter()
        .rposition(|&b| b != b' ')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let ext_end = short_name[8..11]
        .iter()
        .rposition(|&b| b != b' ')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let mut out = Vec::new();
    out.extend(
        short_name[..base_end]
            .iter()
            .map(|b| b.to_ascii_lowercase()),
    );
    if ext_end > 0 {
        out.push(b'.');
        out.extend(
            short_name[8..8 + ext_end]
                .iter()
                .map(|b| b.to_ascii_lowercase()),
        );
    }
    out
}

fn ascii_fold(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(|b| b.to_ascii_lowercase()).collect()
}

fn split_mount_path(path: &CStr) -> Result<(Vec<u8>, &[u8]), FatError> {
    let bytes = path.to_bytes();
    let colon = bytes
        .iter()
        .position(|&byte| byte == b':')
        .ok_or(FatError::PathError)?;
    let key = bytes[..=colon].to_vec();
    let relative = strip_mount_prefix(bytes).ok_or(FatError::PathError)?;
    Ok((key, relative))
}

fn lock_mount_by_key(key: &[u8]) -> Result<SpinGuard<'static, FatMount>, FatError> {
    let mounts = MOUNTS.lock();
    let registered = mounts.get(key).ok_or(FatError::NotFound)?;
    let lock_ptr = &registered.mount as *const SpinLock<FatMount>;
    // MOUNTS remains locked until the per-mount lock is acquired. Unmount uses
    // the same order, so the boxed lock cannot be removed during acquisition.
    let mount_lock: &'static SpinLock<FatMount> = unsafe { &*lock_ptr };
    let guard = mount_lock.lock();
    drop(mounts);
    Ok(guard)
}

fn mount_and_relative_from_path(
    path: &CStr,
) -> Result<(SpinGuard<'static, FatMount>, &[u8]), FatError> {
    let (key, relative) = split_mount_path(path)?;
    Ok((lock_mount_by_key(&key)?, relative))
}

fn resolve_path(mount: &mut FatMount, relative: &[u8]) -> Result<Option<DirEntry>, FatError> {
    if path_is_root(relative) {
        return Ok(None);
    }
    let mut current = path_start_cluster(mount, relative);
    let mut parts = relative
        .split(|&b| b == b'/')
        .filter(|part| !part.is_empty())
        .peekable();
    while let Some(part) = parts.next() {
        if part == b"." {
            if parts.peek().is_none() {
                return Ok(Some(synthetic_directory_entry(current)));
            }
            continue;
        }
        if part == b".." {
            current = directory_parent_cluster(mount, current)?;
            if parts.peek().is_none() {
                return Ok(Some(synthetic_directory_entry(current)));
            }
            continue;
        }
        let entry = find_entry_in_directory(mount, current, part)?.ok_or(FatError::NotFound)?;
        if parts.peek().is_none() {
            return Ok(Some(entry));
        }
        if entry.attr & FAT_ATTR_DIRECTORY == 0 {
            return Err(FatError::NotDirectory);
        }
        current = entry.start_cluster;
    }
    Ok(None)
}

fn synthetic_directory_entry(cluster: u32) -> DirEntry {
    DirEntry {
        name: Vec::new(),
        attr: FAT_ATTR_DIRECTORY,
        start_cluster: cluster,
        size: 0,
        entry_span: Vec::new(),
        short_sector: 0,
        short_offset: 0,
    }
}

fn build_stat(mode: u32, size: i64, blocks: i64, block_size: u16) -> Stat {
    Stat {
        st_mode: mode,
        st_nlink: 1,
        st_size: size,
        st_blksize: block_size as i32,
        st_blocks: blocks as i32,
        ..Stat::default()
    }
}

fn stat_path(path: &CStr, stat: &mut Stat) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    mount.ensure_online()?;
    if path_is_root(relative) {
        *stat = build_stat(
            S_IFDIR
                | S_IRUSR
                | S_IRGRP
                | S_IROTH
                | if mount.read_only {
                    0
                } else {
                    S_IWUSR | S_IWGRP | S_IWOTH
                },
            0,
            1,
            mount.bytes_per_sector,
        );
        return Ok(());
    }
    let mut entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
    if let Some(open) = mount
        .open_files
        .get(&(entry.short_sector, entry.short_offset))
    {
        entry.size = open.size;
    }
    let mut mode = if entry.attr & FAT_ATTR_DIRECTORY != 0 {
        S_IFDIR | S_IRUSR | S_IRGRP | S_IROTH
    } else {
        S_IFREG | S_IRUSR | S_IRGRP | S_IROTH
    };
    if entry.attr & ATTR_READONLY == 0 && !mount.read_only {
        mode |= S_IWUSR | S_IWGRP | S_IWOTH;
    }
    let blocks = if entry.size == 0 {
        0
    } else {
        entry.size.div_ceil(mount.bytes_per_sector as u32)
    };
    *stat = build_stat(
        mode,
        entry.size as i64,
        blocks as i64,
        mount.bytes_per_sector,
    );
    Ok(())
}

fn free_cluster_chain(mount: &mut FatMount, start_cluster: u32) -> Result<(), FatError> {
    if start_cluster < 2 {
        return Ok(());
    }
    if start_cluster > mount.cluster_count + 1 {
        return Err(FatError::Io);
    }
    let mut cluster = start_cluster;
    for _ in 0..mount.cluster_count {
        let next = mount.read_fat_entry(cluster)?;
        mount.write_fat_entry(cluster, 0)?;
        if mount.is_eoc(next) || next == 0 || mount.is_bad_cluster(next) {
            return Ok(());
        }
        if next < 2 || next > mount.cluster_count + 1 {
            return Err(FatError::Io);
        }
        cluster = next;
    }
    Err(FatError::Io)
}

fn mark_entry_deleted(mount: &FatMount, positions: &[EntryPos]) -> Result<(), FatError> {
    for pos in positions {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(pos.sector, &mut buffer)?;
        buffer[pos.offset as usize] = 0xe5;
        mount.write_sector(pos.sector, &buffer)?;
    }
    Ok(())
}

fn unlink_path(path: &CStr) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
    if entry.attr & FAT_ATTR_DIRECTORY != 0 {
        return Err(FatError::IsDirectory);
    }
    if mount.file_is_open(entry.short_sector, entry.short_offset) {
        return Err(FatError::Busy);
    }
    mark_entry_deleted(&mount, &entry.entry_span)?;
    free_cluster_chain(&mut mount, entry.start_cluster)?;
    mount.flush_transport()
}

fn rename_path(old_name: &CStr, new_name: &CStr) -> Result<(), FatError> {
    rename_path_impl(old_name, new_name, false)
}

fn rename_path_impl(old_name: &CStr, new_name: &CStr, replace: bool) -> Result<(), FatError> {
    let (old_key, old_relative) = split_mount_path(old_name)?;
    let (new_key, new_relative) = split_mount_path(new_name)?;
    if old_key != new_key {
        return Err(FatError::CrossDevice);
    }
    let mut mount = lock_mount_by_key(&old_key)?;
    mount.ensure_online()?;
    if mount.read_only {
        return Err(FatError::ReadOnly);
    }
    let old_entry = resolve_path(&mut mount, old_relative)?.ok_or(FatError::Busy)?;
    if old_entry.entry_span.is_empty() {
        return Err(FatError::Busy);
    }
    let target = resolve_path(&mut mount, new_relative);
    let target = match target {
        Ok(value) => value,
        Err(FatError::NotFound) => None,
        Err(error) => return Err(error),
    };
    if replace
        && target.as_ref().is_some_and(|entry| {
            (entry.short_sector == old_entry.short_sector
                && entry.short_offset == old_entry.short_offset)
                || (entry.start_cluster >= 2 && entry.start_cluster == old_entry.start_cluster)
        })
    {
        return Ok(());
    }
    if mount.file_is_open(old_entry.short_sector, old_entry.short_offset)
        || (old_entry.attr & FAT_ATTR_DIRECTORY != 0 && mount.dir_is_open(old_entry.start_cluster))
    {
        return Err(FatError::Busy);
    }
    let (old_parent_cluster, _) = resolve_parent_directory(&mut mount, old_relative)?;
    let (parent_cluster, leaf_name) = resolve_parent_directory(&mut mount, new_relative)?;
    if old_entry.attr & FAT_ATTR_DIRECTORY != 0
        && directory_is_ancestor_of(&mut mount, old_entry.start_cluster, parent_cluster)?
    {
        return Err(FatError::PathError);
    }
    let target = find_entry_in_directory(&mut mount, parent_cluster, leaf_name)?;
    if let Some(target) = &target {
        if !replace {
            return Err(FatError::AlreadyExists);
        }
        if mount.file_is_open(target.short_sector, target.short_offset)
            || (target.attr & FAT_ATTR_DIRECTORY != 0 && mount.dir_is_open(target.start_cluster))
        {
            return Err(FatError::Busy);
        }
        if target.attr & ATTR_READONLY != 0 {
            return Err(FatError::ReadOnly);
        }
        match (
            old_entry.attr & FAT_ATTR_DIRECTORY != 0,
            target.attr & FAT_ATTR_DIRECTORY != 0,
        ) {
            (false, true) => return Err(FatError::IsDirectory),
            (true, false) => return Err(FatError::NotDirectory),
            (true, true) if !directory_is_empty(&mut mount, target.start_cluster)? => {
                return Err(FatError::DirectoryNotEmpty);
            }
            _ => {}
        }
    }
    let new_entry = if let Some(target) = &target {
        target.clone()
    } else if old_entry.attr & FAT_ATTR_DIRECTORY != 0 {
        create_directory_entry(
            &mut mount,
            parent_cluster,
            leaf_name,
            old_entry.start_cluster,
        )?
    } else {
        create_file_entry(&mut mount, parent_cluster, leaf_name)?
    };
    // Preserve the target name and copy all source metadata. Read both sectors
    // before mutation. A replacement reuses the target entry without freeing
    // its cluster chain until the source name has been removed.
    let prepared = (|| {
        let mut old_sector = [0u8; MAX_SECTOR_BYTES];
        let mut new_sector = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(old_entry.short_sector, &mut old_sector)?;
        mount.read_sector(new_entry.short_sector, &mut new_sector)?;
        let original = new_sector;
        let old_offset = old_entry.short_offset as usize;
        let new_offset = new_entry.short_offset as usize;
        new_sector[new_offset + 11..new_offset + 32]
            .copy_from_slice(&old_sector[old_offset + 11..old_offset + 32]);
        Ok((original, new_sector))
    })();
    let source_entries = (|| {
        let mut saved = Vec::new();
        for position in &old_entry.entry_span {
            let mut sector = [0u8; MAX_SECTOR_BYTES];
            mount.read_sector(position.sector, &mut sector)?;
            let offset = position.offset as usize;
            let mut entry = [0u8; 32];
            entry.copy_from_slice(&sector[offset..offset + 32]);
            saved.push((position.clone(), entry));
        }
        Ok::<_, FatError>(saved)
    })();
    let prepared = prepared
        .and_then(|(original, sector)| source_entries.map(|entries| (original, sector, entries)));
    let (original, new_sector, source_entries) = match prepared {
        Ok(value) => value,
        Err(error) => {
            if target.is_none() {
                mark_entry_deleted(&mount, &new_entry.entry_span)?;
            }
            return Err(error);
        }
    };
    let parent_changed =
        old_entry.attr & FAT_ATTR_DIRECTORY != 0 && old_parent_cluster != parent_cluster;
    use rename_transaction::Step;
    rename_transaction::commit(parent_changed, |step| match step {
        Step::PublishDestination => mount.write_sector(new_entry.short_sector, &new_sector),
        Step::ChangeParent => {
            update_directory_parent(&mount, old_entry.start_cluster, parent_cluster)
        }
        Step::RemoveSource => mark_entry_deleted(&mount, &old_entry.entry_span),
        Step::RestoreSource => {
            for (position, entry) in &source_entries {
                write_entry_bytes(&mount, position.sector, position.offset, entry)?;
            }
            Ok(())
        }
        Step::RestoreParent => {
            update_directory_parent(&mount, old_entry.start_cluster, old_parent_cluster)
        }
        Step::RestoreDestination => {
            mount.write_sector(new_entry.short_sector, &original)?;
            if target.is_none() {
                mark_entry_deleted(&mount, &new_entry.entry_span)?;
            }
            Ok(())
        }
        Step::FlushRecovery => mount.flush_transport(),
    })?;
    if let Some(target) = target {
        free_cluster_chain(&mut mount, target.start_cluster)?;
    }
    mount.flush_transport()
}

fn directory_is_ancestor_of(
    mount: &mut FatMount,
    ancestor_cluster: u32,
    mut directory_cluster: u32,
) -> Result<bool, FatError> {
    for _ in 0..=mount.cluster_count {
        if directory_cluster == ancestor_cluster {
            return Ok(true);
        }
        if directory_cluster == mount.root_cluster {
            return Ok(false);
        }

        let sector = mount.cluster_to_sector(directory_cluster)?;
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        let dotdot = &buffer[32..64];
        if dotdot[0] != b'.' || dotdot[1] != b'.' || dotdot[11] & FAT_ATTR_DIRECTORY == 0 {
            return Err(FatError::Io);
        }
        let low = u16::from_le_bytes([dotdot[26], dotdot[27]]) as u32;
        directory_cluster = if mount.fat_type == FatType::Fat32 {
            let high = u16::from_le_bytes([dotdot[20], dotdot[21]]) as u32;
            (high << 16) | low
        } else {
            low
        };
        if directory_cluster == 0 {
            directory_cluster = mount.root_cluster;
        }
    }
    Err(FatError::Io)
}

fn mkdir_path(path: &CStr) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let (parent_cluster, leaf_name) = resolve_parent_directory(&mut mount, relative)?;
    if find_entry_in_directory(&mut mount, parent_cluster, leaf_name)?.is_some() {
        return Err(FatError::AlreadyExists);
    }
    let cluster = mount.allocate_cluster(None)?;
    let entry = match create_directory_entry(&mut mount, parent_cluster, leaf_name, cluster) {
        Ok(entry) => entry,
        Err(error) => {
            let _ = free_cluster_chain(&mut mount, cluster);
            return Err(error);
        }
    };
    if let Err(error) = write_directory_dots(&mut mount, entry.start_cluster, parent_cluster) {
        let _ = mark_entry_deleted(&mount, &entry.entry_span);
        let _ = free_cluster_chain(&mut mount, cluster);
        return Err(error);
    }
    mount.flush_transport()
}

fn rmdir_path(path: &CStr) -> Result<(), FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
    if entry.attr & FAT_ATTR_DIRECTORY == 0 {
        return Err(FatError::NotDirectory);
    }
    if mount.dir_is_open(entry.start_cluster) {
        return Err(FatError::Busy);
    }
    if !directory_is_empty(&mut mount, entry.start_cluster)? {
        return Err(FatError::DirectoryNotEmpty);
    }
    mark_entry_deleted(&mount, &entry.entry_span)?;
    free_cluster_chain(&mut mount, entry.start_cluster)?;
    mount.flush_transport()
}

fn directory_is_empty(mount: &mut FatMount, start_cluster: u32) -> Result<bool, FatError> {
    let mut lfn_stack = Vec::new();
    let mut walk = DirectoryWalk::new(mount, start_cluster)?;
    while let Some(sector) = walk.next_sector(mount, false)? {
        let mut buffer = [0u8; MAX_SECTOR_BYTES];
        mount.read_sector(sector, &mut buffer)?;
        for entry_idx in 0..(mount.bytes_per_sector as usize / 32) {
            let offset = entry_idx * 32;
            let entry = &buffer[offset..offset + 32];
            if entry[0] == 0 {
                return Ok(true);
            }
            if entry[0] == 0xe5 {
                lfn_stack.clear();
                continue;
            }
            if entry[11] == FAT_ATTR_LFN {
                lfn_stack.push(parse_lfn_entry(entry));
                continue;
            }
            if entry[11] & FAT_ATTR_VOLUME_ID != 0 {
                lfn_stack.clear();
                continue;
            }
            let (name, _) = decode_entry_name(entry, &lfn_stack);
            lfn_stack.clear();
            if name != b".".to_vec() && name != b"..".to_vec() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn create_directory_entry(
    mount: &mut FatMount,
    dir_cluster: u32,
    name: &[u8],
    start_cluster: u32,
) -> Result<DirEntry, FatError> {
    let mut entry = create_file_entry(mount, dir_cluster, name)?;
    let mut sector = [0u8; MAX_SECTOR_BYTES];
    if let Err(error) = mount.read_sector(entry.short_sector, &mut sector) {
        let _ = mark_entry_deleted(mount, &entry.entry_span);
        return Err(error);
    }
    sector[entry.short_offset as usize + 11] = FAT_ATTR_DIRECTORY;
    if let Err(error) = mount.write_sector(entry.short_sector, &sector) {
        let _ = mark_entry_deleted(mount, &entry.entry_span);
        return Err(error);
    }
    if let Err(error) =
        mount.update_short_entry(entry.short_sector, entry.short_offset, start_cluster, 0)
    {
        let _ = mark_entry_deleted(mount, &entry.entry_span);
        return Err(error);
    }
    entry.attr = FAT_ATTR_DIRECTORY;
    entry.start_cluster = start_cluster;
    Ok(entry)
}

fn write_directory_dots(
    mount: &mut FatMount,
    cluster: u32,
    parent_cluster: u32,
) -> Result<(), FatError> {
    let mut sector = [0u8; MAX_SECTOR_BYTES];
    let dot = build_dot_entry(b".          ", cluster);
    let dotdot = build_dot_entry(b"..         ", parent_cluster);
    sector[..32].copy_from_slice(&dot);
    sector[32..64].copy_from_slice(&dotdot);
    let first = mount.cluster_to_sector(cluster)?;
    mount.write_sector(first, &sector)
}

fn update_directory_parent(
    mount: &FatMount,
    cluster: u32,
    parent_cluster: u32,
) -> Result<(), FatError> {
    let sector = mount.cluster_to_sector(cluster)?;
    let mut buffer = [0u8; MAX_SECTOR_BYTES];
    mount.read_sector(sector, &mut buffer)?;
    let offset = 32 + 20;
    buffer[offset..offset + 2].copy_from_slice(&((parent_cluster >> 16) as u16).to_le_bytes());
    buffer[offset + 6..offset + 8].copy_from_slice(&(parent_cluster as u16).to_le_bytes());
    mount.write_sector(sector, &buffer)
}

fn build_dot_entry(name: &[u8; 11], cluster: u32) -> [u8; 32] {
    let mut entry = build_short_entry(name, cluster, 0);
    entry[11] = FAT_ATTR_DIRECTORY;
    entry
}

fn diropen_path(
    dir_state: *mut sysbase::DirIter,
    path: &CStr,
) -> Result<*mut sysbase::DirIter, FatError> {
    let (mut mount, relative) = mount_and_relative_from_path(path)?;
    let cluster = if relative.is_empty() {
        mount.root_cluster
    } else {
        let entry = resolve_path(&mut mount, relative)?.ok_or(FatError::NotFound)?;
        if entry.attr & FAT_ATTR_DIRECTORY == 0 {
            return Err(FatError::PathError);
        }
        entry.start_cluster
    };
    let iter = unsafe { dir_state.as_mut() }.ok_or(FatError::Io)?;
    let state = unsafe { iter.dir_struct.cast::<DirState>().as_mut() }.ok_or(FatError::Io)?;
    mount.acquire_dir(cluster)?;
    *state = DirState {
        magic: DIR_MAGIC,
        mount: mount.lock as *const SpinLock<FatMount>,
        initial_cluster: cluster,
        cluster,
        sector_index: 0,
        entry_index: 0,
    };
    Ok(dir_state)
}

unsafe fn dir_state_mut(dir_state: *mut sysbase::DirIter) -> Option<&'static mut DirState> {
    let iter = unsafe { dir_state.as_mut()? };
    unsafe { iter.dir_struct.cast::<DirState>().as_mut() }
}

fn dirnext_entry(
    state: &mut DirState,
    mount: &mut FatMount,
    filename: *mut u8,
    mut stat: Option<&mut Stat>,
) -> Result<bool, FatError> {
    let mut cluster = state.cluster;
    let mut sector_index = state.sector_index;
    let mut entry_index = state.entry_index;
    let mut lfn_stack: Vec<LfnEntry> = Vec::new();

    loop {
        let sector_limit = if mount.is_fixed_root(cluster) {
            mount.root_dir_sectors
        } else {
            mount.sectors_per_cluster as u32
        };
        while sector_index < sector_limit {
            let sector = if mount.is_fixed_root(cluster) {
                mount
                    .root_dir_start
                    .checked_add(sector_index)
                    .ok_or(FatError::Io)?
            } else {
                mount
                    .cluster_to_sector(cluster)?
                    .checked_add(sector_index)
                    .ok_or(FatError::Io)?
            };
            let mut buffer = [0u8; MAX_SECTOR_BYTES];
            mount.read_sector(sector, &mut buffer)?;
            while entry_index < (mount.bytes_per_sector as usize / 32) as u32 {
                let offset = entry_index as usize * 32;
                let entry = &buffer[offset..offset + 32];
                entry_index += 1;
                if entry[0] == 0 {
                    state.sector_index = sector_index;
                    state.entry_index = entry_index;
                    return Ok(false);
                }
                if entry[0] == 0xe5 {
                    lfn_stack.clear();
                    continue;
                }
                if entry[11] == FAT_ATTR_LFN {
                    lfn_stack.push(parse_lfn_entry(entry));
                    continue;
                }
                if entry[11] & FAT_ATTR_VOLUME_ID != 0 {
                    lfn_stack.clear();
                    continue;
                }
                let (name, _) = decode_entry_name(entry, &lfn_stack);
                lfn_stack.clear();
                copy_name_out(filename, &name)?;
                let built =
                    build_dir_entry(mount.fat_type, entry, name, sector, offset as u16, &[]);
                if let Some(stat) = stat.as_deref_mut() {
                    let mut mode = if built.attr & FAT_ATTR_DIRECTORY != 0 {
                        S_IFDIR | S_IRUSR | S_IRGRP | S_IROTH
                    } else {
                        S_IFREG | S_IRUSR | S_IRGRP | S_IROTH
                    };
                    if built.attr & ATTR_READONLY == 0 {
                        mode |= S_IWUSR | S_IWGRP | S_IWOTH;
                    }
                    *stat = build_stat(
                        mode,
                        built.size as i64,
                        built.cluster_blocks(mount) as i64,
                        mount.bytes_per_sector,
                    );
                }
                state.cluster = cluster;
                state.sector_index = sector_index;
                state.entry_index = entry_index;
                return Ok(true);
            }
            sector_index += 1;
            entry_index = 0;
        }

        if mount.is_fixed_root(cluster) {
            return Ok(false);
        }
        let next = mount.read_fat_entry(cluster)?;
        if mount.is_bad_cluster(next) || next == 0 {
            return Err(FatError::Io);
        }
        if mount.is_eoc(next) {
            return Ok(false);
        }
        if next < 2 || next > mount.cluster_count + 1 {
            return Err(FatError::Io);
        }
        cluster = next;
        state.cluster = cluster;
        sector_index = 0;
        entry_index = 0;
    }
}

fn copy_name_out(filename: *mut u8, name: &[u8]) -> Result<(), FatError> {
    if name.len() > NAME_MAX {
        return Err(FatError::PathError);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(name.as_ptr(), filename, name.len());
        *filename.add(name.len()) = 0;
    }
    Ok(())
}

fn statvfs_path(path: &CStr, statvfs: &mut StatVfs) -> Result<(), FatError> {
    let (mut mount, _) = mount_and_relative_from_path(path)?;
    let free_clusters = count_free_clusters(&mut mount)?;
    let cluster_count = mount.cluster_count;
    *statvfs = StatVfs {
        // libfat reports FAT clusters as POSIX blocks.
        f_bsize: mount.cluster_size(),
        f_frsize: mount.cluster_size(),
        f_blocks: cluster_count as u64,
        f_bfree: free_clusters as u64,
        f_bavail: free_clusters as u64,
        f_files: cluster_count,
        f_ffree: free_clusters,
        f_favail: free_clusters,
        f_fsid: mount.disc.io_type,
        f_flag: ST_NOSUID | if mount.read_only { ST_RDONLY } else { 0 },
        f_namemax: NAME_MAX as u32,
    };
    Ok(())
}

fn count_free_clusters(mount: &mut FatMount) -> Result<u32, FatError> {
    let mut free = 0u32;
    for cluster in 2..=mount.cluster_count + 1 {
        if mount.read_fat_entry(cluster)? == 0 {
            free += 1;
        }
    }
    Ok(free)
}

fn truncate_file(
    mount: &mut FatMount,
    file_state: &mut FatFileState,
    new_len: u32,
) -> Result<(), FatError> {
    let result = truncate_file_inner(mount, file_state, new_len);
    // Publish any committed size change, including a partial extension on error.
    let published = mount.publish_file_size(file_state);
    result.and(published)
}

fn truncate_file_inner(
    mount: &mut FatMount,
    file_state: &mut FatFileState,
    new_len: u32,
) -> Result<(), FatError> {
    if new_len == file_state.size {
        return Ok(());
    }
    if new_len > file_state.size {
        let original_position = file_state.position;
        file_state.position = file_state.size;
        let zeroes = [0u8; MAX_SECTOR_BYTES];
        let mut remaining = new_len - file_state.size;
        while remaining != 0 {
            let chunk = min(remaining as usize, zeroes.len());
            match write_file_bytes(mount, file_state, &zeroes[..chunk]) {
                Ok(written) => remaining -= written as u32,
                Err(error) => {
                    file_state.position = original_position;
                    return Err(error);
                }
            }
        }
        file_state.position = original_position;
        mount.update_short_entry(
            file_state.dir_sector,
            file_state.dir_offset,
            file_state.start_cluster,
            file_state.size,
        )?;
        file_state.dirty = 0;
        return Ok(());
    }

    if new_len == 0 {
        let old_start = file_state.start_cluster;
        mount.update_short_entry(file_state.dir_sector, file_state.dir_offset, 0, 0)?;
        file_state.start_cluster = 0;
        file_state.size = 0;
        file_state.dirty = 0;
        return free_cluster_chain(mount, old_start);
    } else {
        let last_cluster_index = (new_len - 1) / mount.cluster_size();
        let last_cluster =
            mount.nth_cluster(&mut file_state.start_cluster, last_cluster_index, false)?;
        let next_cluster = mount.read_fat_entry(last_cluster)?;
        mount.update_short_entry(
            file_state.dir_sector,
            file_state.dir_offset,
            file_state.start_cluster,
            new_len,
        )?;
        file_state.size = new_len;
        file_state.dirty = 0;
        if !mount.is_eoc(next_cluster) {
            mount.write_fat_entry(last_cluster, mount.fat_type.eoc_marker())?;
            if next_cluster >= 2 && !mount.is_bad_cluster(next_cluster) {
                free_cluster_chain(mount, next_cluster)?;
            }
        }
    }
    Ok(())
}

fn set_fat_errno(r: *mut sysbase::Reent, error: FatError) {
    unsafe { sysbase::set_reent_errno(r, fat_errno(error)) };
}

unsafe extern "C" fn fat_link_r(
    r: *mut sysbase::Reent,
    _existing: *const c_char,
    _new_link: *const c_char,
) -> i32 {
    // libfat explicitly does not support hard links.
    unsafe { sysbase::set_reent_errno(r, sysbase::ENOTSUP) };
    -1
}

unsafe extern "C" fn fat_chdir_r(r: *mut sysbase::Reent, name: *const c_char) -> i32 {
    if name.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    let path = unsafe { CStr::from_ptr(name) };
    let (mut mount, relative) = match mount_and_relative_from_path(path) {
        Ok(value) => value,
        Err(error) => {
            set_fat_errno(r, error);
            return -1;
        }
    };
    let cluster = if path_is_root(relative) {
        mount.root_cluster
    } else {
        match resolve_path(&mut mount, relative) {
            Ok(Some(entry)) if entry.attr & FAT_ATTR_DIRECTORY != 0 => entry.start_cluster,
            Ok(None) => mount.root_cluster,
            Ok(Some(_)) => {
                unsafe { sysbase::set_reent_errno(r, sysbase::ENOTDIR) };
                return -1;
            }
            Err(error) => {
                set_fat_errno(r, error);
                return -1;
            }
        }
    };
    mount.cwd_cluster = cluster;
    0
}

unsafe extern "C" fn fat_unlink_r(r: *mut sysbase::Reent, name: *const c_char) -> i32 {
    if name.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    match unlink_path(unsafe { CStr::from_ptr(name) }) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_rename_r(
    r: *mut sysbase::Reent,
    old_name: *const c_char,
    new_name: *const c_char,
) -> i32 {
    if old_name.is_null() || new_name.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    match rename_path(unsafe { CStr::from_ptr(old_name) }, unsafe {
        CStr::from_ptr(new_name)
    }) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_mkdir_r(r: *mut sysbase::Reent, path: *const c_char, _mode: i32) -> i32 {
    if path.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    match mkdir_path(unsafe { CStr::from_ptr(path) }) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_rmdir_r(r: *mut sysbase::Reent, name: *const c_char) -> i32 {
    if name.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    match rmdir_path(unsafe { CStr::from_ptr(name) }) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_diropen_r(
    r: *mut sysbase::Reent,
    dir_state: *mut sysbase::DirIter,
    path: *const c_char,
) -> *mut sysbase::DirIter {
    if dir_state.is_null() || path.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return core::ptr::null_mut();
    }
    match diropen_path(dir_state, unsafe { CStr::from_ptr(path) }) {
        Ok(result) => result,
        Err(error) => {
            set_fat_errno(r, error);
            core::ptr::null_mut()
        }
    }
}

unsafe extern "C" fn fat_dirreset_r(
    r: *mut sysbase::Reent,
    dir_state: *mut sysbase::DirIter,
) -> i32 {
    let Some(state) = (unsafe { dir_state_mut(dir_state) }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    if state.magic != DIR_MAGIC {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    }
    state.cluster = state.initial_cluster;
    state.sector_index = 0;
    state.entry_index = 0;
    0
}

unsafe extern "C" fn fat_dirnext_r(
    r: *mut sysbase::Reent,
    dir_state: *mut sysbase::DirIter,
    filename: *mut c_char,
    file_stat: *mut c_void,
) -> i32 {
    let Some(state) = (unsafe { dir_state_mut(dir_state) }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    if state.magic != DIR_MAGIC {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    }
    if filename.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    let Some(mount_lock) = (unsafe { state.mount.as_ref() }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    let mut mount = mount_lock.lock();
    let stat = if file_stat.is_null() {
        None
    } else {
        Some(unsafe { &mut *file_stat.cast::<Stat>() })
    };
    match dirnext_entry(state, &mut mount, filename.cast::<u8>(), stat) {
        Ok(true) => 0,
        // libfat uses -1 for end-of-directory without setting a new errno.
        Ok(false) => -1,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

unsafe extern "C" fn fat_dirclose_r(
    r: *mut sysbase::Reent,
    dir_state: *mut sysbase::DirIter,
) -> i32 {
    let Some(state) = (unsafe { dir_state_mut(dir_state) }) else {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    };
    if state.magic != DIR_MAGIC {
        unsafe { sysbase::set_reent_errno(r, sysbase::EBADF) };
        return -1;
    }
    if let Some(mount_lock) = unsafe { state.mount.as_ref() } {
        mount_lock.lock().release_dir(state.initial_cluster);
    }
    state.magic = 0;
    0
}

unsafe extern "C" fn fat_statvfs_r(
    r: *mut sysbase::Reent,
    path: *const c_char,
    buf: *mut c_void,
) -> i32 {
    if path.is_null() || buf.is_null() {
        unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
        return -1;
    }
    let statvfs = unsafe { &mut *buf.cast::<StatVfs>() };
    match statvfs_path(unsafe { CStr::from_ptr(path) }, statvfs) {
        Ok(()) => 0,
        Err(error) => {
            set_fat_errno(r, error);
            -1
        }
    }
}

fn fat_position_from_off_t(position: i64) -> Option<u32> {
    u32::try_from(position).ok()
}

fn truncate_len_from_off_t(len: i64) -> Option<u32> {
    fat_position_from_off_t(len)
}

unsafe extern "C" fn fat_ftruncate_r(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    len: i64,
) -> i32 {
    unsafe {
        let Some(file_state) = (file_struct as *mut FatFileState).as_mut() else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        if file_state.magic != FILE_MAGIC {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        }
        let Some(len) = truncate_len_from_off_t(len) else {
            sysbase::set_reent_errno(r, sysbase::EFBIG);
            return -1;
        };
        if file_state.flags & (sysbase::O_WRONLY | sysbase::O_RDWR) == 0 {
            sysbase::set_reent_errno(r, sysbase::EINVAL);
            return -1;
        }
        let Some(mount_lock) = ({ file_state.mount.as_ref() }) else {
            sysbase::set_reent_errno(r, sysbase::EBADF);
            return -1;
        };
        let mut mount = mount_lock.lock();
        match truncate_file(&mut mount, file_state, len) {
            Ok(()) => 0,
            Err(error) => {
                set_fat_errno(r, error);
                -1
            }
        }
    }
}

unsafe extern "C" fn fat_fpathconf_r(
    r: *mut sysbase::Reent,
    _file_struct: *mut c_void,
    name: i32,
) -> i32 {
    match name {
        PATHCONF_NAME_MAX => NAME_MAX as i32,
        PATHCONF_PATH_MAX => 1024,
        _ => {
            unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
            -1
        }
    }
}

unsafe extern "C" fn fat_pathconf_r(
    r: *mut sysbase::Reent,
    _path: *const c_char,
    name: i32,
) -> i32 {
    match name {
        PATHCONF_NAME_MAX => NAME_MAX as i32,
        PATHCONF_PATH_MAX => 1024,
        _ => {
            unsafe { sysbase::set_reent_errno(r, sysbase::EINVAL) };
            -1
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod ftruncate_tests {
    use super::*;

    #[test]
    fn ftruncate_callback_matches_off_t_and_checks_u32_backend_range() {
        let _: sysbase::FtruncateFn = fat_ftruncate_r;
        let _: sysbase::SeekFn = fat_seek_r;
        assert_eq!(core::mem::size_of::<i64>(), 8);
        assert_eq!(
            truncate_len_from_off_t(i64::from(i32::MAX) + 1),
            Some(0x8000_0000)
        );
        assert_eq!(truncate_len_from_off_t(i64::from(u32::MAX)), Some(u32::MAX));
        assert_eq!(truncate_len_from_off_t(i64::from(u32::MAX) + 1), None);
        assert_eq!(truncate_len_from_off_t(-1), None);
        assert_eq!(
            fat_position_from_off_t(i64::from(i32::MAX) + 1),
            Some(0x8000_0000)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SECTOR_FIXTURE: [u8; MIN_SECTOR_BYTES] = [0xa5; MIN_SECTOR_BYTES];

    unsafe extern "C" fn read_aligned_sector(lba: u32, count: u32, output: *mut c_void) -> bool {
        if lba != 7 || count != 1 || output as usize % 32 != 0 {
            return false;
        }
        // SAFETY: the production helper provides a writable 4096-byte sector
        // buffer, and the independent fixture contains exactly 512 bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(
                SECTOR_FIXTURE.as_ptr(),
                output.cast::<u8>(),
                SECTOR_FIXTURE.len(),
            );
        }
        true
    }

    #[test]
    fn sector_dma_buffer_has_disc_interface_alignment() {
        assert_eq!(core::mem::align_of::<AlignedSector>(), 32);
        assert_eq!(core::mem::size_of::<AlignedSector>(), MAX_SECTOR_BYTES);
    }

    #[test]
    fn sector_read_uses_aligned_dma_storage_and_copies_to_unaligned_callers() {
        #[repr(align(32))]
        struct AlignedOutput([u8; MIN_SECTOR_BYTES + 1]);
        let mut storage = AlignedOutput([0u8; MIN_SECTOR_BYTES + 1]);
        let output = &mut storage.0[1..];
        assert_ne!((output.as_ptr() as usize) % 32, 0);
        assert!(read_disc_sector_aligned(
            read_aligned_sector,
            7,
            MIN_SECTOR_BYTES,
            output,
        ));
        assert_eq!(output, SECTOR_FIXTURE);
    }

    fn boot_sector() -> [u8; MAX_SECTOR_BYTES] {
        let mut sector = [0u8; MAX_SECTOR_BYTES];
        sector[510] = 0x55;
        sector[511] = 0xaa;
        sector[11..13].copy_from_slice(&(MIN_SECTOR_BYTES as u16).to_le_bytes());
        sector[13] = 1;
        sector[14..16].copy_from_slice(&1u16.to_le_bytes());
        sector[16] = 2;
        sector
    }

    #[test]
    fn fat12_entry_codec_preserves_neighbor_nibbles() {
        let (low, high) = encode_fat12_entry(0xaa, 0xb0, 2, 0x123);
        assert_eq!((low, high), (0x23, 0xb1));
        assert_eq!(decode_fat12_entry(low, high, 2), 0x123);

        let (low, high) = encode_fat12_entry(0x0a, 0xbb, 3, 0x456);
        assert_eq!((low, high), (0x6a, 0x45));
        assert_eq!(decode_fat12_entry(low, high, 3), 0x456);
    }

    #[test]
    fn bpb_cluster_count_selects_fat12() {
        let mut sector = boot_sector();
        sector[17..19].copy_from_slice(&224u16.to_le_bytes());
        sector[19..21].copy_from_slice(&2880u16.to_le_bytes());
        sector[22..24].copy_from_slice(&9u16.to_le_bytes());
        let parsed = parse_bpb(&sector).expect("FAT12 BPB should parse");
        assert_eq!(parsed.fat_type, FatType::Fat12);
        assert_eq!(parsed.root_cluster, 0);
        assert_eq!(parsed.root_dir_sectors, 14);
    }

    #[test]
    fn bpb_cluster_count_selects_fat16() {
        let mut sector = boot_sector();
        sector[13] = 2;
        sector[17..19].copy_from_slice(&512u16.to_le_bytes());
        sector[19..21].copy_from_slice(&32768u16.to_le_bytes());
        sector[22..24].copy_from_slice(&64u16.to_le_bytes());
        let parsed = parse_bpb(&sector).expect("FAT16 BPB should parse");
        assert_eq!(parsed.fat_type, FatType::Fat16);
        assert_eq!(parsed.root_cluster, 0);
        assert_eq!(parsed.root_dir_sectors, 32);
    }

    #[test]
    fn bpb_cluster_count_selects_fat32() {
        let mut sector = boot_sector();
        sector[13] = 8;
        sector[14..16].copy_from_slice(&32u16.to_le_bytes());
        sector[17..19].copy_from_slice(&0u16.to_le_bytes());
        sector[19..21].copy_from_slice(&0u16.to_le_bytes());
        sector[22..24].copy_from_slice(&0u16.to_le_bytes());
        sector[32..36].copy_from_slice(&1_000_000u32.to_le_bytes());
        sector[36..40].copy_from_slice(&1000u32.to_le_bytes());
        sector[44..48].copy_from_slice(&2u32.to_le_bytes());
        let parsed = parse_bpb(&sector).expect("FAT32 BPB should parse");
        assert_eq!(parsed.fat_type, FatType::Fat32);
        assert_eq!(parsed.root_cluster, 2);
        assert_eq!(parsed.root_dir_sectors, 0);
    }
    #[test]
    fn bpb_accepts_supported_logical_sector_sizes() {
        for bytes in [512u16, 1024, 2048, 4096] {
            let mut sector = boot_sector();
            sector[11..13].copy_from_slice(&bytes.to_le_bytes());
            sector[17..19].copy_from_slice(&512u16.to_le_bytes());
            sector[19..21].copy_from_slice(&32768u16.to_le_bytes());
            sector[22..24].copy_from_slice(&64u16.to_le_bytes());
            let parsed = parse_bpb(&sector).expect("supported logical sector size should parse");
            assert_eq!(parsed.bytes_per_sector, bytes);
        }
    }

    #[test]
    fn failed_device_startup_reports_absent_without_calling_cleanup() {
        let mut insertion_checked = false;
        let mut shutdown_called = false;

        let result: Result<bool, ()> = check_device_presence(
            false,
            || false,
            || {
                insertion_checked = true;
                true
            },
            || {
                shutdown_called = true;
                Ok(())
            },
        );

        assert_eq!(result, Ok(false));
        assert!(!insertion_checked);
        assert!(!shutdown_called);
    }

    #[test]
    fn successful_device_startup_checks_media_and_propagates_cleanup_failure() {
        let mut shutdown_called = false;
        let result = check_device_presence(
            false,
            || true,
            || false,
            || {
                shutdown_called = true;
                Err(FatError::Io)
            },
        );

        assert_eq!(result, Err(FatError::Io));
        assert!(shutdown_called);
    }
}
