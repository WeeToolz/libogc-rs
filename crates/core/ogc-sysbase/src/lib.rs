#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

mod device_io;
pub mod error;
pub use device_io::{device_close_failures, last_device_close_errno};
pub use error::{
    EACCES, EBADF, EBADMSG, EBUSY, ECANCELED, EEXIST, EFBIG, EINVAL, EIO, EISDIR, ENODEV, ENOENT,
    ENOMEM, ENOSPC, ENOTDIR, ENOTEMPTY, ENOTSUP, EOVERFLOW, ERANGE, EROFS, EXDEV,
};

#[cfg(target_arch = "powerpc")]
use alloc::alloc::{Layout, alloc_zeroed, dealloc};

use core::ffi::{CStr, c_char, c_void};

#[cfg(target_arch = "powerpc")]
use core::ptr::null_mut;

pub const O_WRONLY: i32 = 0x0001;
pub const O_RDONLY: i32 = 0x0000;
pub const O_RDWR: i32 = 0x0002;
pub const O_ACCMODE: i32 = 0x0003;
pub const O_APPEND: i32 = 0x0008;
pub const O_CREAT: i32 = 0x0200;
pub const O_TRUNC: i32 = 0x0400;
pub const O_EXCL: i32 = 0x0800;
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

// newlib mode bits shared by devoptab stat implementations.
pub const S_IFREG: u32 = 0o100000;
pub const S_IRUSR: u32 = 0o0400;
pub const S_IRGRP: u32 = 0o0040;

#[repr(C)]
pub struct Reent {
    /// First field of newlib's `_reent`; devoptab callbacks write errno here.
    pub _errno: i32,
    _private: [u8; 0],
}

#[inline]
/// # Safety
/// A non-null `reent` must point to a writable Reent with exclusive access.
pub unsafe fn set_reent_errno(reent: *mut Reent, errno: i32) {
    if !reent.is_null() {
        unsafe { (*reent)._errno = errno };
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DirIter {
    pub device: i32,
    pub dir_struct: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TimeVal {
    pub tv_sec: i64,
    pub tv_usec: i32,
}

#[cfg(all(test, target_arch = "powerpc"))]
mod timeval_layout_tests {
    use super::TimeVal;
    use core::mem::{offset_of, size_of};

    #[test]
    fn timeval_matches_devkitppc_time_t_and_suseconds_t_layout() {
        assert_eq!(size_of::<TimeVal>(), 16);
        assert_eq!(offset_of!(TimeVal, tv_sec), 0);
        assert_eq!(offset_of!(TimeVal, tv_usec), 8);
    }
}

/// newlib PowerPC `struct timespec` layout used by devoptab callbacks.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i32,
    pub _pad: i32,
}

/// newlib PowerPC `struct stat` layout used by devoptab callbacks.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Stat {
    pub st_dev: u32,
    pub st_ino: u32,
    pub st_mode: u32,
    pub st_nlink: u16,
    pub st_uid: u16,
    pub st_gid: u16,
    pub _pad0: u16,
    pub st_rdev: u32,
    pub st_size: i64,
    pub st_atim: Timespec,
    pub st_mtim: Timespec,
    pub st_ctim: Timespec,
    pub st_blksize: i32,
    pub st_blocks: i32,
    pub st_spare4: [i32; 2],
}

/// newlib PowerPC `struct statvfs` layout used by devoptab callbacks.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct StatVfs {
    pub f_bsize: u32,
    pub f_frsize: u32,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u32,
    pub f_ffree: u32,
    pub f_favail: u32,
    pub f_fsid: u32,
    pub f_flag: u32,
    pub f_namemax: u32,
}

#[cfg(all(test, target_arch = "powerpc"))]
mod statvfs_layout_tests {
    use super::StatVfs;
    use core::mem::{offset_of, size_of};

    #[test]
    fn statvfs_matches_devkitppc_field_widths_and_offsets() {
        assert_eq!(size_of::<StatVfs>(), 56);
        assert_eq!(offset_of!(StatVfs, f_bsize), 0);
        assert_eq!(offset_of!(StatVfs, f_frsize), 4);
        assert_eq!(offset_of!(StatVfs, f_blocks), 8);
        assert_eq!(offset_of!(StatVfs, f_bfree), 16);
        assert_eq!(offset_of!(StatVfs, f_bavail), 24);
        assert_eq!(offset_of!(StatVfs, f_files), 32);
        assert_eq!(offset_of!(StatVfs, f_ffree), 36);
        assert_eq!(offset_of!(StatVfs, f_favail), 40);
        assert_eq!(offset_of!(StatVfs, f_fsid), 44);
        assert_eq!(offset_of!(StatVfs, f_flag), 48);
        assert_eq!(offset_of!(StatVfs, f_namemax), 52);
    }
}

pub type OpenFn = unsafe extern "C" fn(
    r: *mut Reent,
    file_struct: *mut c_void,
    path: *const c_char,
    flags: i32,
    mode: i32,
) -> i32;

pub type CloseFn = unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void) -> i32;

pub type WriteFn = unsafe extern "C" fn(
    r: *mut Reent,
    file_struct: *mut c_void,
    ptr: *const c_char,
    len: usize,
) -> isize;

pub type ReadFn = unsafe extern "C" fn(
    r: *mut Reent,
    file_struct: *mut c_void,
    ptr: *mut c_char,
    len: usize,
) -> isize;

pub type SeekFn =
    unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void, pos: i64, dir: i32) -> i64;

pub type FstatFn =
    unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void, st: *mut c_void) -> i32;

pub type StatFn = unsafe extern "C" fn(r: *mut Reent, file: *const c_char, st: *mut c_void) -> i32;

pub type LinkFn =
    unsafe extern "C" fn(r: *mut Reent, existing: *const c_char, new_link: *const c_char) -> i32;

pub type UnlinkFn = unsafe extern "C" fn(r: *mut Reent, name: *const c_char) -> i32;
pub type ChdirFn = unsafe extern "C" fn(r: *mut Reent, name: *const c_char) -> i32;
pub type RenameFn =
    unsafe extern "C" fn(r: *mut Reent, old_name: *const c_char, new_name: *const c_char) -> i32;
pub type MkdirFn = unsafe extern "C" fn(r: *mut Reent, path: *const c_char, mode: i32) -> i32;

pub type DiropenFn = unsafe extern "C" fn(
    r: *mut Reent,
    dir_state: *mut DirIter,
    path: *const c_char,
) -> *mut DirIter;
pub type DirresetFn = unsafe extern "C" fn(r: *mut Reent, dir_state: *mut DirIter) -> i32;
pub type DirnextFn = unsafe extern "C" fn(
    r: *mut Reent,
    dir_state: *mut DirIter,
    filename: *mut c_char,
    file_stat: *mut c_void,
) -> i32;
pub type DircloseFn = unsafe extern "C" fn(r: *mut Reent, dir_state: *mut DirIter) -> i32;
pub type StatvfsFn =
    unsafe extern "C" fn(r: *mut Reent, path: *const c_char, buf: *mut c_void) -> i32;
pub type FtruncateFn =
    unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void, len: i64) -> i32;
pub type FsyncFn = unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void) -> i32;
pub type ChmodFn = unsafe extern "C" fn(r: *mut Reent, path: *const c_char, mode: u32) -> i32;
pub type FchmodFn = unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void, mode: u32) -> i32;
pub type RmdirFn = unsafe extern "C" fn(r: *mut Reent, name: *const c_char) -> i32;
pub type LstatFn = unsafe extern "C" fn(r: *mut Reent, file: *const c_char, st: *mut c_void) -> i32;
pub type UtimesFn =
    unsafe extern "C" fn(r: *mut Reent, filename: *const c_char, times: *const TimeVal) -> i32;
pub type FpathconfFn =
    unsafe extern "C" fn(r: *mut Reent, file_struct: *mut c_void, name: i32) -> i32;
pub type PathconfFn = unsafe extern "C" fn(r: *mut Reent, path: *const c_char, name: i32) -> i32;
pub type SymlinkFn =
    unsafe extern "C" fn(r: *mut Reent, target: *const c_char, linkpath: *const c_char) -> i32;
pub type ReadlinkFn = unsafe extern "C" fn(
    r: *mut Reent,
    path: *const c_char,
    buf: *mut c_char,
    bufsiz: usize,
) -> isize;

#[repr(C)]
pub struct Devoptab {
    pub name: *const c_char,
    pub struct_size: usize,
    pub open_r: Option<OpenFn>,
    pub close_r: Option<CloseFn>,
    pub write_r: Option<WriteFn>,
    pub read_r: Option<ReadFn>,
    pub seek_r: Option<SeekFn>,
    pub fstat_r: Option<FstatFn>,
    pub stat_r: Option<StatFn>,
    pub link_r: Option<LinkFn>,
    pub unlink_r: Option<UnlinkFn>,
    pub chdir_r: Option<ChdirFn>,
    pub rename_r: Option<RenameFn>,
    pub mkdir_r: Option<MkdirFn>,
    pub dir_state_size: usize,
    pub diropen_r: Option<DiropenFn>,
    pub dirreset_r: Option<DirresetFn>,
    pub dirnext_r: Option<DirnextFn>,
    pub dirclose_r: Option<DircloseFn>,
    pub statvfs_r: Option<StatvfsFn>,
    pub ftruncate_r: Option<FtruncateFn>,
    pub fsync_r: Option<FsyncFn>,
    pub device_data: *mut c_void,
    pub chmod_r: Option<ChmodFn>,
    pub fchmod_r: Option<FchmodFn>,
    pub rmdir_r: Option<RmdirFn>,
    pub lstat_r: Option<LstatFn>,
    pub utimes_r: Option<UtimesFn>,
    pub fpathconf_r: Option<FpathconfFn>,
    pub pathconf_r: Option<PathconfFn>,
    pub symlink_r: Option<SymlinkFn>,
    pub readlink_r: Option<ReadlinkFn>,
}

#[cfg(target_arch = "powerpc")]
const DEVICE_SLOTS: usize = 35;
#[cfg(target_arch = "powerpc")]
const FIRST_DYNAMIC_DEVICE: usize = 3;

#[cfg(target_arch = "powerpc")]
static DEFAULT_DEVICE: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);

#[cfg(target_arch = "powerpc")]
static mut DOTAB_STDNULL: Devoptab = Devoptab {
    name: c"stdnull".as_ptr(),
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
    device_data: core::ptr::null_mut(),
    chmod_r: None,
    fchmod_r: None,
    rmdir_r: None,
    lstat_r: None,
    utimes_r: None,
    fpathconf_r: None,
    pathconf_r: None,
    symlink_r: None,
    readlink_r: None,
};

/// Low-level mounted-device file handle opened through the active devoptab.
///
/// This is the expert path. Higher-level code should prefer a guarded wrapper
/// that constrains paths, access mode, and write volume.
pub struct DeviceFile {
    #[cfg(target_arch = "powerpc")]
    devoptab: *const Devoptab,

    #[cfg(target_arch = "powerpc")]
    file_struct: *mut c_void,

    #[cfg(target_arch = "powerpc")]
    layout: Layout,

    #[cfg(not(target_arch = "powerpc"))]
    _private: (),
}

impl DeviceFile {
    /// Opens a device-backed file for reading through the active devoptab.
    pub fn open_read(path: &CStr) -> Option<Self> {
        Self::open_with_flags(path, &[O_RDONLY])
    }

    /// Opens a device-backed file for append-oriented writes.
    ///
    /// This is intentionally low level: callers are responsible for choosing a
    /// safe path and for deciding whether direct mounted-device access is
    /// appropriate for their use case.
    pub fn open_append(path: &CStr) -> Option<Self> {
        Self::open_with_flags(
            path,
            &[O_WRONLY | O_APPEND | O_CREAT, O_RDWR | O_APPEND | O_CREAT],
        )
    }

    /// Opens a device-backed file and replaces an older probe log.
    pub fn open_truncate(path: &CStr) -> Option<Self> {
        Self::open_with_flags(
            path,
            &[O_WRONLY | O_CREAT | O_TRUNC, O_RDWR | O_CREAT | O_TRUNC],
        )
    }

    /// Open with the exact flags. Failures return a negative Newlib errno.
    pub fn open_checked(path: &CStr, flags: i32) -> Result<Self, i32> {
        let devoptab = device_op_tab(path).ok_or(-ENODEV)?;
        let open_r = unsafe { (*devoptab).open_r }.ok_or(-ENOTSUP)?;
        // A successful open must have a real resource release operation.
        if unsafe { (*devoptab).close_r }.is_none() {
            return Err(-ENOTSUP);
        }
        let layout = Layout::from_size_align(unsafe { (*devoptab).struct_size }.max(1), 8)
            .map_err(|_| -EINVAL)?;
        let file_struct = unsafe { alloc_zeroed(layout) }.cast::<c_void>();
        if file_struct.is_null() {
            return Err(-ENOMEM);
        }
        // Clear stale errno so a broken callback cannot reuse an old failure.
        unsafe { set_reent_errno(current_reent(), 0) };
        let result = unsafe { open_r(current_reent(), file_struct, path.as_ptr(), flags, 0o666) };
        if result < 0 {
            let error = device_io::callback_failure(errno);
            unsafe { dealloc(file_struct.cast::<u8>(), layout) };
            return Err(error);
        }
        Ok(Self {
            devoptab,
            file_struct,
            layout,
        })
    }

    fn open_with_flags(path: &CStr, flags_to_try: &[i32]) -> Option<Self> {
        for &flags in flags_to_try {
            match Self::open_checked(path, flags) {
                Ok(file) => return Some(file),
                Err(error) => unsafe { set_reent_errno(current_reent(), -error) },
            }
        }
        None
    }

    /// Write every byte or return the first negative Newlib errno.
    pub fn write_all(&self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.is_empty() {
            return Ok(());
        }
        let write_r = unsafe { (*self.devoptab).write_r }.ok_or(-ENOTSUP)?;
        device_io::write_all(bytes, |remaining| {
            device_io::count(
                remaining.len(),
                || {
                    unsafe { set_reent_errno(current_reent(), 0) };
                    unsafe {
                        write_r(
                            current_reent(),
                            self.file_struct,
                            remaining.as_ptr().cast(),
                            remaining.len(),
                        )
                    }
                },
                errno,
            )
        })
    }

    /// Read bytes, returning EOF as zero and failures as negative Newlib errno.
    pub fn read(&self, output: &mut [u8]) -> Result<usize, i32> {
        if output.is_empty() {
            return Ok(0);
        }
        let read_r = unsafe { (*self.devoptab).read_r }.ok_or(-ENOTSUP)?;
        device_io::count(
            output.len(),
            || {
                unsafe { set_reent_errno(current_reent(), 0) };
                unsafe {
                    read_r(
                        current_reent(),
                        self.file_struct,
                        output.as_mut_ptr().cast(),
                        output.len(),
                    )
                }
            },
            errno,
        )
    }

    pub fn seek(&self, position: i32, direction: i32) -> Result<i32, i32> {
        self.seek_off_t(i64::from(position), direction)
            .and_then(|position| i32::try_from(position).map_err(|_| -EOVERFLOW))
    }

    /// Seek using target-width off_t. Failures return negative Newlib errno.
    pub fn seek_off_t(&self, position: i64, direction: i32) -> Result<i64, i32> {
        let seek_r = unsafe { (*self.devoptab).seek_r }.ok_or(-ENOTSUP)?;
        device_io::seek(
            || {
                unsafe { set_reent_errno(current_reent(), 0) };
                unsafe { seek_r(current_reent(), self.file_struct, position, direction) }
            },
            errno,
        )
    }

    /// Flush metadata and transport buffers; return the actual callback errno.
    pub fn sync(&self) -> Result<(), i32> {
        let fsync_r = unsafe { (*self.devoptab).fsync_r }.ok_or(-ENOTSUP)?;
        device_io::status(
            || {
                unsafe { set_reent_errno(current_reent(), 0) };
                unsafe { fsync_r(current_reent(), self.file_struct) }
            },
            errno,
        )
    }

    /// Consume this handle after one close attempt. A failure is returned and
    /// recorded; Drop will not retry a possibly invalidated backend resource.
    pub fn close(mut self) -> Result<(), i32> {
        self.close_once()
            .inspect_err(|error| device_io::record_close_failure(*error))
    }

    fn close_once(&mut self) -> Result<(), i32> {
        let state = core::mem::take(&mut self.file_struct);
        if state.is_null() {
            return Ok(());
        }
        let callback = unsafe { (*self.devoptab).close_r };
        device_io::close(
            || {
                let close_r = callback.ok_or(-ENOTSUP)?;
                device_io::status(
                    || {
                        unsafe { set_reent_errno(current_reent(), 0) };
                        unsafe { close_r(current_reent(), state) }
                    },
                    errno,
                )
            },
            || unsafe { dealloc(state.cast::<u8>(), self.layout) },
        )
    }
}

impl Drop for DeviceFile {
    fn drop(&mut self) {
        self.close_once()
            .unwrap_or_else(device_io::record_close_failure);
    }
}

#[cfg(target_arch = "powerpc")]
pub fn device_op_tab(path: &CStr) -> Option<*const Devoptab> {
    let device = unsafe { GetDeviceOpTab(path.as_ptr()) };
    (!device.is_null()).then_some(device)
}

#[cfg(target_arch = "powerpc")]
/// Registers a devoptab entry in the Rust-owned newlib-compatible device table.
///
/// # Safety
/// `device` and every pointer reachable from it must remain valid and stable
/// until the device is removed.
pub unsafe fn add_device(device: *const Devoptab) -> i32 {
    unsafe { AddDevice(device) }
}

#[cfg(target_arch = "powerpc")]
pub fn find_device(name: &CStr) -> i32 {
    unsafe { FindDevice(name.as_ptr()) }
}

#[cfg(target_arch = "powerpc")]
pub fn remove_device(name: &CStr) -> i32 {
    unsafe { RemoveDevice(name.as_ptr()) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn device_name_bytes(device: *const Devoptab) -> Option<&'static [u8]> {
    if device.is_null() {
        return None;
    }
    let name = unsafe { (*device).name };
    if name.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(name) }.to_bytes())
}

#[cfg(target_arch = "powerpc")]
fn name_matches_path(device_name: &[u8], path: &[u8]) -> bool {
    if !path.starts_with(device_name) {
        return false;
    }
    matches!(path.get(device_name.len()..), Some([b':', ..]))
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
/// A non-null name must be a valid NUL-terminated path. Serialize access to the device table.
pub unsafe extern "C" fn FindDevice(name: *const c_char) -> i32 {
    if name.is_null() {
        return -1;
    }
    let path = unsafe { CStr::from_ptr(name) }.to_bytes();
    if !path.contains(&b':') {
        return DEFAULT_DEVICE.load(core::sync::atomic::Ordering::Acquire);
    }
    for index in 0..DEVICE_SLOTS {
        let device = unsafe {
            core::ptr::addr_of!(devoptab_list)
                .cast::<*const Devoptab>()
                .add(index)
                .read()
        };
        let Some(device_name) = (unsafe { device_name_bytes(device) }) else {
            continue;
        };
        if name_matches_path(device_name, path) {
            return index as i32;
        }
    }
    -1
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
/// A non-null name must be NUL-terminated. Serialize table access and stop users of the removed device first.
pub unsafe extern "C" fn RemoveDevice(name: *const c_char) -> i32 {
    if name.is_null() {
        return -1;
    }
    let requested = unsafe { CStr::from_ptr(name) }.to_bytes();
    for index in FIRST_DYNAMIC_DEVICE..DEVICE_SLOTS {
        let device = unsafe {
            core::ptr::addr_of!(devoptab_list)
                .cast::<*const Devoptab>()
                .add(index)
                .read()
        };
        let Some(device_name) = (unsafe { device_name_bytes(device) }) else {
            continue;
        };
        if requested == device_name || name_matches_path(device_name, requested) {
            unsafe {
                core::ptr::addr_of_mut!(devoptab_list)
                    .cast::<*const Devoptab>()
                    .add(index)
                    .write(core::ptr::null())
            };
            return 0;
        }
    }
    -1
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
/// A non-null device and its name and callbacks must stay valid while registered. Serialize device table access.
pub unsafe extern "C" fn AddDevice(device: *const Devoptab) -> i32 {
    let Some(name) = (unsafe { device_name_bytes(device) }) else {
        return -1;
    };
    if name.is_empty() {
        return -1;
    }
    for index in FIRST_DYNAMIC_DEVICE..DEVICE_SLOTS {
        let current = unsafe {
            core::ptr::addr_of!(devoptab_list)
                .cast::<*const Devoptab>()
                .add(index)
                .read()
        };
        if current.is_null() {
            unsafe {
                core::ptr::addr_of_mut!(devoptab_list)
                    .cast::<*const Devoptab>()
                    .add(index)
                    .write(device)
            };
            return index as i32;
        }
        let Some(current_name) = (unsafe { device_name_bytes(current) }) else {
            continue;
        };
        if current_name == name || current_name == b"stdnull" {
            unsafe {
                core::ptr::addr_of_mut!(devoptab_list)
                    .cast::<*const Devoptab>()
                    .add(index)
                    .write(device)
            };
            return index as i32;
        }
    }
    -1
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
/// A non-null name must be NUL-terminated. Serialize table access and preserve the lifetime of the returned device.
pub unsafe extern "C" fn GetDeviceOpTab(name: *const c_char) -> *const Devoptab {
    let index = unsafe { FindDevice(name) };
    if index < 0 || index >= DEVICE_SLOTS as i32 {
        return core::ptr::null();
    }
    unsafe { devoptab_list[index as usize] }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn setDefaultDevice(device: i32) {
    // The reference accepts STD_MAX itself, which can create an out-of-range
    // default index. Reject that value as a Rust memory-safety deviation.
    if device >= FIRST_DYNAMIC_DEVICE as i32 && device < DEVICE_SLOTS as i32 {
        DEFAULT_DEVICE.store(device, core::sync::atomic::Ordering::Release);
    }
}

#[cfg(target_arch = "powerpc")]
pub fn current_reent() -> *mut Reent {
    ogc_lwp_driver::current_reent().cast::<Reent>()
}

/// Return the current thread's newlib errno storage, including early startup.
#[cfg(target_arch = "powerpc")]
pub fn errno_location() -> *mut i32 {
    // SAFETY: current_reent always supplies initialized reentrancy storage.
    unsafe { core::ptr::addr_of_mut!((*current_reent())._errno) }
}

/// Read the current thread's newlib errno without changing it or allocating.
#[cfg(target_arch = "powerpc")]
pub fn errno() -> i32 {
    // SAFETY: The calling thread owns this field for the duration of the read.
    unsafe { errno_location().read() }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub static mut devoptab_list: [*const Devoptab; DEVICE_SLOTS] =
    [core::ptr::addr_of!(DOTAB_STDNULL); DEVICE_SLOTS];

/// Replaces the newlib stdin device with `device`.
///
/// This matches libogc's `devoptab_list[STD_IN]` hook. The device must remain
/// valid for as long as stdin can use it.
#[cfg(target_arch = "powerpc")]
/// # Safety
/// The device and its callbacks must stay valid while installed. Serialize stdin device table access.
pub unsafe fn set_stdin_device(device: *const Devoptab) -> *const Devoptab {
    unsafe { core::mem::replace(&mut devoptab_list[0], device) }
}

/// Restores stdin if the expected device is still active.
#[cfg(target_arch = "powerpc")]
/// # Safety
/// The previous device must remain valid after restoration. Serialize stdin device table access.
pub unsafe fn restore_stdin_device(expected: *const Devoptab, previous: *const Devoptab) -> bool {
    unsafe {
        if devoptab_list[0] == expected {
            devoptab_list[0] = previous;
            true
        } else {
            false
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod devoptab_tests {
    use super::*;

    static NAME: &[u8] = b"es\0";
    static PATH: &[u8] = b"es:/0000000100000002/00000004\0";
    static RELATIVE_PATH: &[u8] = b"relative/path\0";

    fn test_devoptab() -> Devoptab {
        Devoptab {
            name: NAME.as_ptr().cast(),
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
            device_data: core::ptr::null_mut(),
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

    #[test]
    fn rust_devoptab_registry_matches_reference_behavior() {
        let previous_default = DEFAULT_DEVICE.load(core::sync::atomic::Ordering::Acquire);
        let device = test_devoptab();
        let index = unsafe { AddDevice(&device) };
        assert!(index >= FIRST_DYNAMIC_DEVICE as i32);
        assert_eq!(unsafe { FindDevice(PATH.as_ptr().cast()) }, index);
        assert!(core::ptr::eq(
            unsafe { GetDeviceOpTab(PATH.as_ptr().cast()) },
            &device
        ));

        setDefaultDevice(index);
        assert_eq!(unsafe { FindDevice(RELATIVE_PATH.as_ptr().cast()) }, index);
        assert!(core::ptr::eq(
            unsafe { GetDeviceOpTab(RELATIVE_PATH.as_ptr().cast()) },
            &device
        ));

        assert_eq!(unsafe { RemoveDevice(PATH.as_ptr().cast()) }, 0);
        assert!(unsafe { devoptab_list[index as usize] }.is_null());
        assert_eq!(unsafe { FindDevice(PATH.as_ptr().cast()) }, -1);

        let replacement = test_devoptab();
        assert_eq!(unsafe { AddDevice(&replacement) }, index);
        setDefaultDevice(0);
        let default = unsafe { devoptab_list[0] };
        assert_eq!(unsafe { RemoveDevice(NAME.as_ptr().cast()) }, 0);
        assert!(unsafe { devoptab_list[index as usize] }.is_null());
        assert_eq!(unsafe { devoptab_list[0] }, default);
        assert_eq!(unsafe { RemoveDevice(NAME.as_ptr().cast()) }, -1);
        DEFAULT_DEVICE.store(previous_default, core::sync::atomic::Ordering::Release);
    }

    #[test]
    fn device_lookup_matches_reference_name_rules() {
        assert!(name_matches_path(b"sd", b"sd:/boot.dol"));
        assert!(name_matches_path(b"es", b"es:/0000000100000002/00000004"));
        assert!(name_matches_path(b"es", b"es:"));
        assert!(!name_matches_path(b"usb", b"usb1:/apps"));
        assert!(name_matches_path(b"usb1", b"usb1:/apps"));
        assert!(!name_matches_path(b"sd:", b"sd:/boot.dol"));
        assert!(!name_matches_path(b"sd", b"sda:/boot.dol"));
    }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
unsafe extern "C" fn strncmp(lhs: *const c_char, rhs: *const c_char, n: usize) -> i32 {
    for idx in 0..n {
        let a = unsafe { *lhs.add(idx) };
        let b = unsafe { *rhs.add(idx) };

        if a != b {
            return a as i32 - b as i32;
        }

        if a == 0 {
            return 0;
        }
    }

    0
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
unsafe extern "C" fn strcmp(lhs: *const c_char, rhs: *const c_char) -> i32 {
    let mut idx = 0usize;
    loop {
        let a = unsafe { *lhs.add(idx) };
        let b = unsafe { *rhs.add(idx) };
        if a != b {
            return a as i32 - b as i32;
        }
        if a == 0 {
            return 0;
        }
        idx += 1;
    }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
unsafe extern "C" fn strchr(s: *const c_char, c: i32) -> *mut c_char {
    let needle = c as u8;
    let mut ptr = s;

    loop {
        let byte = unsafe { *ptr };

        if byte == needle {
            return ptr.cast_mut();
        }

        if byte == 0 {
            return if needle == 0 {
                ptr.cast_mut()
            } else {
                null_mut()
            };
        }

        ptr = unsafe { ptr.add(1) };
    }
}
