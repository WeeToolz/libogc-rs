#![no_std]

//! Read-only mount routing for block-backed filesystems.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use alloc::string::String;
use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char, c_void};
use core::mem::{align_of, size_of};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use ogc_filesystem_sys::DiscInterface;
use ogc_lwp_sys as lwp;

use ogc_exfat::{
    DirectoryHandle as ExfatDirectoryHandle, Error as ExfatError, FileInfo, ReadHandle, Volume,
};

/// Filesystem format selected by [`mount_sd`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// The existing FAT12/16/32 backend owns the mount.
    Fat,
    /// The read-only exFAT backend owns the mount.
    Exfat,
}

/// Mount and handle lifecycle errors.
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// The SD device did not start or has no inserted card.
    DeviceUnavailable,
    /// The device could not read the boot records.
    Io,
    /// The exFAT boot records do not provide valid bounds.
    InvalidGeometry,
    /// The mount already exists.
    AlreadyMounted,
    /// Open files or directories still use the mount.
    Busy,
    /// No mount exists.
    NotMounted,
    /// The handle belongs to an old or different mount.
    InvalidHandle,
    /// The medium was removed after it was mounted.
    MediaRemoved,
    /// exFAT rejected its on-disk metadata.
    Exfat(ExfatError),
}

struct Lock<T> {
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Lock<T> {}

impl<T> Lock<T> {
    const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> LockGuard<'_, T> {
        let mutex = blockfs_mutex();
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            panic!("blockfs scheduler mutex lock failed");
        }
        LockGuard { lock: self, mutex }
    }
}

struct LockGuard<'a, T> {
    lock: &'a Lock<T>,
    mutex: lwp::Mutex,
}

impl<T> core::ops::Deref for LockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: this guard holds the unique lock.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: this guard holds the unique lock.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for LockGuard<'_, T> {
    fn drop(&mut self) {
        if lwp::mutex_unlock(self.mutex) != lwp::LWP_SUCCESSFUL {
            panic!("blockfs scheduler mutex unlock failed");
        }
    }
}

static MUTEX_READY: AtomicBool = AtomicBool::new(false);
static MUTEX_STARTING: AtomicBool = AtomicBool::new(false);
static mut BLOCKFS_MUTEX: lwp::Mutex = lwp::LWP_MUTEX_NULL;

fn blockfs_mutex() -> lwp::Mutex {
    if MUTEX_READY.load(Ordering::Acquire) {
        // SAFETY: the ready flag publishes the initialized mutex handle.
        return unsafe { BLOCKFS_MUTEX };
    }
    if MUTEX_STARTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let mut mutex = lwp::LWP_MUTEX_NULL;
        // SAFETY: `mutex` is a valid unique output reference.
        if unsafe { lwp::mutex_init(&mut mutex, 1) } != lwp::LWP_SUCCESSFUL {
            MUTEX_STARTING.store(false, Ordering::Release);
            panic!("blockfs scheduler mutex initialization failed");
        }
        // SAFETY: this thread owns initialization; READY publishes the value.
        unsafe { BLOCKFS_MUTEX = mutex };
        MUTEX_READY.store(true, Ordering::Release);
        MUTEX_STARTING.store(false, Ordering::Release);
        mutex
    } else {
        while !MUTEX_READY.load(Ordering::Acquire) {
            lwp::yield_thread();
        }
        // SAFETY: the ready flag publishes the initialized mutex handle.
        unsafe { BLOCKFS_MUTEX }
    }
}

struct ExfatMount {
    volume: Volume,
    generation: u32,
    open_handles: u32,
    started_device: bool,
    online: bool,
    devoptab: alloc::boxed::Box<ogc_sysbase::Devoptab>,
    registered: bool,
}

// SAFETY: The Devoptab points to a static name and static callbacks. The
// registration remains in this mount and all access uses the mount lock.
unsafe impl Send for ExfatMount {}

static MOUNT: Lock<Option<ExfatMount>> = Lock::new(None);
static NEXT_GENERATION: AtomicU32 = AtomicU32::new(1);

fn ensure_online(mount: &mut ExfatMount) -> Result<(), Error> {
    if mount.online {
        let inserted = ogc_wiisd::WII_SD_INTERFACE
            .is_inserted
            .ok_or(Error::DeviceUnavailable)?;
        // SAFETY: Wii SD has been started while this mount is live.
        mount.online = unsafe { inserted() };
    }
    if !mount.online {
        if mount.registered {
            if ogc_sysbase::remove_device(c"sd") < 0 {
                return Err(Error::Io);
            }
            mount.registered = false;
        }
        return Err(Error::MediaRemoved);
    }
    Ok(())
}

fn next_generation() -> u32 {
    loop {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        if generation != 0 {
            return generation;
        }
    }
}

#[repr(C, align(32))]
struct Sector([u8; 512]);

fn read_sector(disc: &'static DiscInterface, sector: u32, out: &mut Sector) -> Result<(), Error> {
    let read = disc.read_sectors.ok_or(Error::Io)?;
    // SAFETY: the output buffer is aligned and holds one Wii SD sector.
    if unsafe { read(sector, 1, out.0.as_mut_ptr().cast()) } {
        Ok(())
    } else {
        Err(Error::Io)
    }
}

fn stop_started_device(disc: &DiscInterface, started: bool) -> Result<(), Error> {
    if !started {
        return Ok(());
    }
    let shutdown = disc.shutdown.ok_or(Error::Io)?;
    // SAFETY: this call pairs with this mount attempt's successful startup.
    if unsafe { shutdown() } {
        Ok(())
    } else {
        Err(Error::Io)
    }
}

fn exfat_extent(disc: &'static DiscInterface) -> Result<Option<u64>, Error> {
    let mut boot = Sector([0; 512]);
    read_sector(disc, 0, &mut boot)?;
    if boot.0[3..11] == *b"EXFAT   " {
        let sectors = u64::from_le_bytes(boot.0[72..80].try_into().expect("8-byte field"));
        if sectors == 0 || sectors > u64::from(u32::MAX) + 1 {
            return Err(Error::InvalidGeometry);
        }
        return Ok(Some(sectors));
    }
    if boot.0[510..512] != [0x55, 0xaa] {
        return Ok(None);
    }
    for index in 0..4 {
        let offset = 446 + index * 16;
        let entry = &boot.0[offset..offset + 16];
        if entry[4] != 0x07 {
            continue;
        }
        let start = u64::from(u32::from_le_bytes(
            entry[8..12].try_into().expect("4-byte LBA"),
        ));
        let length = u64::from(u32::from_le_bytes(
            entry[12..16].try_into().expect("4-byte size"),
        ));
        let end = start.checked_add(length).ok_or(Error::InvalidGeometry)?;
        if start == 0 || length == 0 || end > u64::from(u32::MAX) + 1 {
            return Err(Error::InvalidGeometry);
        }
        let start_lba = u32::try_from(start).map_err(|_| Error::InvalidGeometry)?;
        read_sector(disc, start_lba, &mut boot)?;
        if boot.0[3..11] == *b"EXFAT   " {
            let volume_length =
                u64::from_le_bytes(boot.0[72..80].try_into().expect("8-byte field"));
            if volume_length == 0 || volume_length > length {
                return Err(Error::InvalidGeometry);
            }
            return Ok(Some(end));
        }
    }
    Ok(None)
}

/// Start Wii SD and mount either the existing FAT backend or the exFAT reader.
///
/// exFAT uses the validated volume extent as its read bound. If the card CSD
/// reports a capacity, the extent must also fit inside that capacity.
pub fn mount_sd() -> Result<Format, Error> {
    let mut mount_slot = MOUNT.lock();
    if let Some(mount) = mount_slot.as_mut() {
        ensure_online(mount)?;
        if !mount.registered {
            // SAFETY: the boxed Devoptab remains alive in `MOUNT`.
            if unsafe { ogc_sysbase::add_device(&*mount.devoptab) } < 0 {
                return Err(Error::Io);
            }
            mount.registered = true;
        }
        return Ok(Format::Exfat);
    }
    if ogc_fat::is_mounted(c"sd") {
        return Ok(Format::Fat);
    }

    let disc = &ogc_wiisd::WII_SD_INTERFACE;
    let startup = disc.startup.ok_or(Error::DeviceUnavailable)?;
    let inserted = disc.is_inserted.ok_or(Error::DeviceUnavailable)?;
    // Detect whether another caller already owns the initialized transport.
    // `is_inserted` is false until startup has initialized the card.
    let was_inserted = unsafe { inserted() };
    if !unsafe { startup() } {
        return Err(Error::DeviceUnavailable);
    }
    let started_device = !was_inserted;
    if !unsafe { inserted() } {
        stop_started_device(disc, started_device)?;
        return Err(Error::DeviceUnavailable);
    }

    let extent = match exfat_extent(disc) {
        Ok(extent) => extent,
        Err(error) => {
            stop_started_device(disc, started_device)?;
            return Err(error);
        }
    };
    let Some(sectors) = extent else {
        stop_started_device(disc, started_device)?;
        drop(mount_slot);
        ogc_fat::mount_sd().map_err(|_| Error::Io)?;
        return Ok(Format::Fat);
    };
    if ogc_wiisd::sector_count().is_some_and(|capacity| sectors > capacity) {
        stop_started_device(disc, started_device)?;
        return Err(Error::InvalidGeometry);
    }

    // Use a second MBR read as exFAT::Volume expects whole-device LBA bounds.
    // The extent includes the partition start and volume length.
    // SAFETY: this function starts the Wii SD device and the count was bounded
    // by the CSD when available, otherwise by the validated volume extent.
    let volume = match unsafe { Volume::mount(disc, sectors, 512) } {
        Ok(volume) => volume,
        Err(error) => {
            stop_started_device(disc, started_device)?;
            return Err(Error::Exfat(error));
        }
    };
    *mount_slot = Some(ExfatMount {
        volume,
        generation: next_generation(),
        open_handles: 0,
        started_device,
        online: true,
        devoptab: make_devoptab(),
        registered: false,
    });
    let mount = mount_slot.as_mut().expect("mount was inserted");
    // SAFETY: the boxed Devoptab remains alive until RemoveDevice succeeds.
    if unsafe { ogc_sysbase::add_device(&*mount.devoptab) } < 0 {
        let failed = mount_slot.take().expect("mount was inserted");
        drop(mount_slot);
        stop_started_device(disc, failed.started_device)?;
        return Err(Error::Io);
    }
    mount.registered = true;
    Ok(Format::Exfat)
}

/// Return `true` when the exFAT SD mount is online.
pub fn exfat_is_mounted() -> bool {
    MOUNT.lock().is_some()
}

/// Unmount SD after all file and directory handles have closed.
pub fn unmount_sd() -> Result<(), Error> {
    if ogc_fat::is_mounted(c"sd") {
        return ogc_fat::unmount(c"sd").map_err(|_| Error::Io);
    }
    let mut slot = MOUNT.lock();
    let mounted = slot.as_ref().ok_or(Error::NotMounted)?;
    if mounted.open_handles != 0 {
        return Err(Error::Busy);
    }
    if mounted.registered && ogc_sysbase::remove_device(c"sd") < 0 {
        return Err(Error::Io);
    }
    if let Some(mounted) = slot.as_mut() {
        mounted.registered = false;
    }
    let mount = slot.take().ok_or(Error::NotMounted)?;
    if mount.started_device
        && let Some(shutdown) = ogc_wiisd::WII_SD_INTERFACE.shutdown
    {
        if !unsafe { shutdown() } {
            let mut mount = mount;
            // SAFETY: the boxed Devoptab remains alive in this mount.
            if unsafe { ogc_sysbase::add_device(&*mount.devoptab) } >= 0 {
                mount.registered = true;
            }
            *slot = Some(mount);
            return Err(Error::Io);
        }
    }
    Ok(())
}

/// Return metadata from the mounted exFAT volume.
pub fn stat(path: &str) -> Result<FileInfo, Error> {
    let mut slot = MOUNT.lock();
    let mount = slot.as_mut().ok_or(Error::NotMounted)?;
    ensure_online(mount)?;
    mount.volume.stat(path).map_err(Error::Exfat)
}

/// Open an exFAT file. Dropping the handle releases one mount reference.
pub fn open(path: &str) -> Result<File, Error> {
    let mut slot = MOUNT.lock();
    let mount = slot.as_mut().ok_or(Error::NotMounted)?;
    ensure_online(mount)?;
    let handle = mount.volume.open_handle(path).map_err(Error::Exfat)?;
    mount.open_handles = mount.open_handles.checked_add(1).ok_or(Error::Busy)?;
    Ok(File {
        generation: mount.generation,
        handle: Some(handle),
        position: 0,
    })
}

/// Open an exFAT directory. Dropping the handle releases one mount reference.
pub fn open_dir(path: &str) -> Result<Directory, Error> {
    let mut slot = MOUNT.lock();
    let mount = slot.as_mut().ok_or(Error::NotMounted)?;
    ensure_online(mount)?;
    let handle = mount.volume.open_dir_handle(path).map_err(Error::Exfat)?;
    mount.open_handles = mount.open_handles.checked_add(1).ok_or(Error::Busy)?;
    Ok(Directory {
        generation: mount.generation,
        handle: Some(handle),
    })
}

fn release(generation: u32) {
    if let Some(mount) = MOUNT.lock().as_mut()
        && mount.generation == generation
    {
        mount.open_handles = mount.open_handles.saturating_sub(1);
    }
}

/// An open read-only exFAT file.
pub struct File {
    generation: u32,
    handle: Option<ReadHandle>,
    position: u64,
}

impl File {
    /// Return the file size in bytes.
    pub fn len(&self) -> Result<u64, Error> {
        let mut slot = MOUNT.lock();
        let mount = slot.as_mut().ok_or(Error::MediaRemoved)?;
        if mount.generation != self.generation {
            return Err(Error::InvalidHandle);
        }
        ensure_online(mount)?;
        mount
            .volume
            .handle_len(self.handle.as_ref().ok_or(Error::InvalidHandle)?)
            .map_err(Error::Exfat)
    }

    /// Return the current file offset.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Seek to an absolute byte offset.
    pub fn seek(&mut self, position: u64) -> Result<(), Error> {
        let mut slot = MOUNT.lock();
        let mount = slot.as_mut().ok_or(Error::MediaRemoved)?;
        if mount.generation != self.generation {
            return Err(Error::InvalidHandle);
        }
        ensure_online(mount)?;
        mount
            .volume
            .seek_handle(self.handle.as_mut().ok_or(Error::InvalidHandle)?, position)
            .map_err(Error::Exfat)?;
        self.position = position;
        Ok(())
    }

    /// Read from the current byte offset.
    pub fn read(&mut self, out: &mut [u8]) -> Result<usize, Error> {
        let mut slot = MOUNT.lock();
        let mount = slot.as_mut().ok_or(Error::MediaRemoved)?;
        if mount.generation != self.generation {
            return Err(Error::InvalidHandle);
        }
        ensure_online(mount)?;
        let count = mount
            .volume
            .read_handle(self.handle.as_mut().ok_or(Error::InvalidHandle)?, out)
            .map_err(Error::Exfat)?;
        self.position = self
            .position
            .checked_add(count as u64)
            .ok_or(Error::InvalidHandle)?;
        Ok(count)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.handle.take();
        release(self.generation);
    }
}

/// An open read-only exFAT directory.
pub struct Directory {
    generation: u32,
    handle: Option<ExfatDirectoryHandle>,
}

impl Directory {
    /// Return the next entry or `None` at end of directory.
    pub fn next(&mut self) -> Result<Option<FileInfo>, Error> {
        let mut slot = MOUNT.lock();
        let mount = slot.as_mut().ok_or(Error::MediaRemoved)?;
        if mount.generation != self.generation {
            return Err(Error::InvalidHandle);
        }
        ensure_online(mount)?;
        mount
            .volume
            .next_dir_handle(self.handle.as_mut().ok_or(Error::InvalidHandle)?)
            .map_err(Error::Exfat)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        self.handle.take();
        release(self.generation);
    }
}

/// Map the read-only mount error to a Newlib errno value.
pub fn errno(error: &Error) -> i32 {
    match error {
        Error::DeviceUnavailable | Error::MediaRemoved => ogc_sysbase::ENODEV,
        Error::Io => ogc_sysbase::EIO,
        Error::InvalidGeometry | Error::Exfat(ExfatError::InvalidBoot) => ogc_sysbase::EBADMSG,
        Error::AlreadyMounted | Error::Busy => ogc_sysbase::EBUSY,
        Error::NotMounted => ogc_sysbase::ENODEV,
        Error::InvalidHandle => ogc_sysbase::EBADF,
        Error::Exfat(ExfatError::NotFound) => ogc_sysbase::ENOENT,
        Error::Exfat(ExfatError::Io) => ogc_sysbase::EIO,
        Error::Exfat(ExfatError::InvalidHandle) => ogc_sysbase::EBADF,
        Error::Exfat(ExfatError::NotDirectory) => ogc_sysbase::ENOTDIR,
        Error::Exfat(ExfatError::IsDirectory) => ogc_sysbase::EISDIR,
        Error::Exfat(ExfatError::InvalidPath) => ogc_sysbase::EINVAL,
        Error::Exfat(ExfatError::Unsupported | ExfatError::Corrupt) => ogc_sysbase::EIO,
    }
}

const FILE_MAGIC: u32 = 0x4246_5331;
const DIR_MAGIC: u32 = 0x4244_5331;

struct FileState {
    magic: u32,
    file: Option<File>,
}

struct DirState {
    magic: u32,
    directory: Option<Directory>,
}

fn make_devoptab() -> alloc::boxed::Box<ogc_sysbase::Devoptab> {
    alloc::boxed::Box::new(ogc_sysbase::Devoptab {
        name: c"sd".as_ptr(),
        struct_size: size_of::<FileState>(),
        open_r: Some(open_r),
        close_r: Some(close_r),
        write_r: Some(write_r),
        read_r: Some(read_r),
        seek_r: Some(seek_r),
        fstat_r: Some(fstat_r),
        stat_r: Some(stat_r),
        link_r: None,
        unlink_r: None,
        chdir_r: None,
        rename_r: None,
        mkdir_r: None,
        dir_state_size: size_of::<DirState>(),
        diropen_r: Some(diropen_r),
        dirreset_r: None,
        dirnext_r: Some(dirnext_r),
        dirclose_r: Some(dirclose_r),
        statvfs_r: None,
        ftruncate_r: None,
        fsync_r: None,
        device_data: null_mut(),
        chmod_r: None,
        fchmod_r: None,
        rmdir_r: None,
        lstat_r: Some(stat_r),
        utimes_r: None,
        fpathconf_r: None,
        pathconf_r: None,
        symlink_r: None,
        readlink_r: None,
    })
}

fn set_errno(r: *mut ogc_sysbase::Reent, value: i32) {
    // SAFETY: Newlib passes its writable per-thread reent structure.
    unsafe { ogc_sysbase::set_reent_errno(r, value) };
}

unsafe fn path_str<'a>(path: *const c_char) -> Result<&'a str, i32> {
    if path.is_null() {
        return Err(ogc_sysbase::EINVAL);
    }
    // SAFETY: Devoptab passes a NUL-terminated path for the callback duration.
    unsafe { CStr::from_ptr(path) }
        .to_str()
        .map_err(|_| ogc_sysbase::EINVAL)
}

fn relative_path(path: &str) -> &str {
    path.strip_prefix("sd:").unwrap_or(path)
}

unsafe fn file_state<'a>(ptr: *mut c_void) -> Result<&'a mut FileState, i32> {
    if ptr.is_null() || !(ptr as usize).is_multiple_of(align_of::<FileState>()) {
        return Err(ogc_sysbase::EBADF);
    }
    // SAFETY: Newlib allocated `struct_size` bytes for this Devoptab handle.
    let state = unsafe { &mut *ptr.cast::<FileState>() };
    if state.magic != FILE_MAGIC || state.file.is_none() {
        return Err(ogc_sysbase::EBADF);
    }
    Ok(state)
}

unsafe fn dir_state<'a>(iter: *mut ogc_sysbase::DirIter) -> Result<&'a mut DirState, i32> {
    if iter.is_null() || !iter.is_aligned() {
        return Err(ogc_sysbase::EBADF);
    }
    // SAFETY: Newlib provides a DirIter and storage sized by dir_state_size.
    let ptr = unsafe { (*iter).dir_struct };
    if ptr.is_null() || !(ptr as usize).is_multiple_of(align_of::<DirState>()) {
        return Err(ogc_sysbase::EBADF);
    }
    let state = unsafe { &mut *ptr.cast::<DirState>() };
    if state.magic != DIR_MAGIC || state.directory.is_none() {
        return Err(ogc_sysbase::EBADF);
    }
    Ok(state)
}

unsafe extern "C" fn open_r(
    r: *mut ogc_sysbase::Reent,
    ptr: *mut c_void,
    path: *const c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    let path = match unsafe { path_str(path) } {
        Ok(path) => path,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    if ptr.is_null() || !(ptr as usize).is_multiple_of(align_of::<FileState>()) {
        set_errno(r, ogc_sysbase::EBADF);
        return -1;
    }
    if flags
        & (ogc_sysbase::O_ACCMODE
            | ogc_sysbase::O_CREAT
            | ogc_sysbase::O_TRUNC
            | ogc_sysbase::O_APPEND)
        != ogc_sysbase::O_RDONLY
    {
        set_errno(r, ogc_sysbase::EROFS);
        return -1;
    }
    match open(relative_path(path)) {
        Ok(file) => {
            // SAFETY: Newlib provided uninitialized storage of `struct_size`.
            unsafe {
                ptr.cast::<FileState>().write(FileState {
                    magic: FILE_MAGIC,
                    file: Some(file),
                })
            };
            0
        }
        Err(error) => {
            set_errno(r, errno(&error));
            -1
        }
    }
}

unsafe extern "C" fn close_r(r: *mut ogc_sysbase::Reent, ptr: *mut c_void) -> i32 {
    match unsafe { file_state(ptr) } {
        Ok(state) => {
            state.magic = 0;
            drop(state.file.take());
            0
        }
        Err(errno) => {
            set_errno(r, errno);
            -1
        }
    }
}

unsafe extern "C" fn read_r(
    r: *mut ogc_sysbase::Reent,
    ptr: *mut c_void,
    out: *mut c_char,
    length: usize,
) -> isize {
    if length > isize::MAX as usize || (length != 0 && out.is_null()) {
        set_errno(r, ogc_sysbase::EINVAL);
        return -1;
    }
    let state = match unsafe { file_state(ptr) } {
        Ok(state) => state,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    let data = if length == 0 {
        core::ptr::NonNull::<u8>::dangling().as_ptr()
    } else {
        out.cast::<u8>()
    };
    // SAFETY: Newlib provides a writable buffer with the requested length.
    let buffer = unsafe { core::slice::from_raw_parts_mut(data, length) };
    match state
        .file
        .as_mut()
        .expect("validated file state")
        .read(buffer)
    {
        Ok(count) => count as isize,
        Err(error) => {
            set_errno(r, errno(&error));
            -1
        }
    }
}

unsafe extern "C" fn write_r(
    r: *mut ogc_sysbase::Reent,
    ptr: *mut c_void,
    _data: *const c_char,
    _length: usize,
) -> isize {
    if unsafe { file_state(ptr) }.is_err() {
        set_errno(r, ogc_sysbase::EBADF);
    } else {
        set_errno(r, ogc_sysbase::EROFS);
    }
    -1
}

unsafe extern "C" fn seek_r(
    r: *mut ogc_sysbase::Reent,
    ptr: *mut c_void,
    offset: i64,
    whence: i32,
) -> i64 {
    let state = match unsafe { file_state(ptr) } {
        Ok(state) => state,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    let file = state.file.as_mut().expect("validated file state");
    let base = match whence {
        ogc_sysbase::SEEK_SET => 0,
        ogc_sysbase::SEEK_CUR => file.position(),
        ogc_sysbase::SEEK_END => match file.len() {
            Ok(length) => length,
            Err(error) => {
                set_errno(r, errno(&error));
                return -1;
            }
        },
        _ => {
            set_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
    };
    let target = i128::from(base) + i128::from(offset);
    if target < 0 || target > i128::from(i64::MAX) {
        set_errno(r, ogc_sysbase::EINVAL);
        return -1;
    }
    let target = target as u64;
    match file.seek(target) {
        Ok(()) => target as i64,
        Err(error) => {
            set_errno(r, errno(&error));
            -1
        }
    }
}

fn fill_stat(stat: &mut ogc_sysbase::Stat, info: &FileInfo) -> Result<(), i32> {
    stat.st_size = i64::try_from(info.size).map_err(|_| ogc_sysbase::EOVERFLOW)?;
    stat.st_mode = if info.is_dir { 0o040555 } else { 0o100444 };
    stat.st_nlink = 1;
    stat.st_blksize = 512;
    stat.st_blocks = i32::try_from(info.size.div_ceil(512)).unwrap_or(i32::MAX);
    Ok(())
}

unsafe extern "C" fn fstat_r(
    r: *mut ogc_sysbase::Reent,
    ptr: *mut c_void,
    out: *mut c_void,
) -> i32 {
    if out.is_null() || !(out as usize).is_multiple_of(align_of::<ogc_sysbase::Stat>()) {
        set_errno(r, ogc_sysbase::EINVAL);
        return -1;
    }
    let state = match unsafe { file_state(ptr) } {
        Ok(state) => state,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    let file = state.file.as_ref().expect("validated file state");
    let info = match file.len() {
        Ok(size) => FileInfo {
            name: String::new(),
            size,
            is_dir: false,
        },
        Err(error) => {
            set_errno(r, errno(&error));
            return -1;
        }
    };
    let stat = unsafe { &mut *out.cast::<ogc_sysbase::Stat>() };
    *stat = ogc_sysbase::Stat::default();
    match fill_stat(stat, &info) {
        Ok(()) => 0,
        Err(errno) => {
            set_errno(r, errno);
            -1
        }
    }
}

unsafe extern "C" fn stat_r(
    r: *mut ogc_sysbase::Reent,
    path: *const c_char,
    out: *mut c_void,
) -> i32 {
    if out.is_null() || !(out as usize).is_multiple_of(align_of::<ogc_sysbase::Stat>()) {
        set_errno(r, ogc_sysbase::EINVAL);
        return -1;
    }
    let path = match unsafe { path_str(path) } {
        Ok(path) => path,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    let info = match stat(relative_path(path)) {
        Ok(info) => info,
        Err(error) => {
            set_errno(r, errno(&error));
            return -1;
        }
    };
    let stat = unsafe { &mut *out.cast::<ogc_sysbase::Stat>() };
    *stat = ogc_sysbase::Stat::default();
    match fill_stat(stat, &info) {
        Ok(()) => 0,
        Err(errno) => {
            set_errno(r, errno);
            -1
        }
    }
}

unsafe extern "C" fn diropen_r(
    r: *mut ogc_sysbase::Reent,
    iter: *mut ogc_sysbase::DirIter,
    path: *const c_char,
) -> *mut ogc_sysbase::DirIter {
    if iter.is_null() || !iter.is_aligned() {
        set_errno(r, ogc_sysbase::EINVAL);
        return null_mut();
    }
    let dir_struct = unsafe { (*iter).dir_struct };
    if dir_struct.is_null() || !(dir_struct as usize).is_multiple_of(align_of::<DirState>()) {
        set_errno(r, ogc_sysbase::EBADF);
        return null_mut();
    }
    let path = match unsafe { path_str(path) } {
        Ok(path) => path,
        Err(errno) => {
            set_errno(r, errno);
            return null_mut();
        }
    };
    match open_dir(relative_path(path)) {
        Ok(directory) => {
            unsafe {
                dir_struct.cast::<DirState>().write(DirState {
                    magic: DIR_MAGIC,
                    directory: Some(directory),
                })
            };
            iter
        }
        Err(error) => {
            set_errno(r, errno(&error));
            null_mut()
        }
    }
}

unsafe extern "C" fn dirnext_r(
    r: *mut ogc_sysbase::Reent,
    iter: *mut ogc_sysbase::DirIter,
    filename: *mut c_char,
    out: *mut c_void,
) -> i32 {
    if filename.is_null() {
        set_errno(r, ogc_sysbase::EINVAL);
        return -1;
    }
    let state = match unsafe { dir_state(iter) } {
        Ok(state) => state,
        Err(errno) => {
            set_errno(r, errno);
            return -1;
        }
    };
    let next = match state
        .directory
        .as_mut()
        .expect("validated directory state")
        .next()
    {
        Ok(Some(entry)) => entry,
        Ok(None) => return -1,
        Err(error) => {
            set_errno(r, errno(&error));
            return -1;
        }
    };
    if next.name.len() > 255 {
        set_errno(r, ogc_sysbase::EOVERFLOW);
        return -1;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(next.name.as_ptr(), filename.cast::<u8>(), next.name.len());
        *filename.cast::<u8>().add(next.name.len()) = 0;
    }
    if !out.is_null() && (out as usize).is_multiple_of(align_of::<ogc_sysbase::Stat>()) {
        let stat = unsafe { &mut *out.cast::<ogc_sysbase::Stat>() };
        *stat = ogc_sysbase::Stat::default();
        if let Err(errno) = fill_stat(stat, &next) {
            set_errno(r, errno);
            return -1;
        }
    }
    0
}

unsafe extern "C" fn dirclose_r(
    r: *mut ogc_sysbase::Reent,
    iter: *mut ogc_sysbase::DirIter,
) -> i32 {
    match unsafe { dir_state(iter) } {
        Ok(state) => {
            state.magic = 0;
            drop(state.directory.take());
            0
        }
        Err(errno) => {
            set_errno(r, errno);
            -1
        }
    }
}
