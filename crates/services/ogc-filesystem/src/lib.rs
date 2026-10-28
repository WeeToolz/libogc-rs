#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_char, c_void};
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use iso9660 as iso9660_rs;
use ogc_ipc::{
    FileDescriptor, HeapId, IPC_OPEN_NONE, IoctlBuffers, Ioctlv, IoctlvEntry, Ipccallback,
    open_file, open_file_async,
};

pub use ogc_filesystem_sys as sys;
pub use sys::{DiscInterface, ISO_MAXPATHLEN};

const ISFS_MAXPATH: usize = 64;
const ISFS_IOCTL_FORMAT: i32 = 1;
const ISFS_IOCTL_GETSTATS: i32 = 2;
const ISFS_IOCTL_CREATEDIR: i32 = 3;
const ISFS_IOCTL_READDIR: i32 = 4;
const ISFS_IOCTL_SETATTR: i32 = 5;
const ISFS_IOCTL_GETATTR: i32 = 6;
const ISFS_IOCTL_DELETE: i32 = 7;
const ISFS_IOCTL_RENAME: i32 = 8;
const ISFS_IOCTL_CREATEFILE: i32 = 9;
const ISFS_IOCTL_GETFILESTATS: i32 = 11;
const ISFS_IOCTL_GETUSAGE: i32 = 12;
const ISFS_ASYNC_PAYLOAD_SIZE: usize = ISFS_MAXPATH * 2;

pub const ISFS_OPEN_READ: u8 = 0x01;
pub const ISFS_OPEN_WRITE: u8 = 0x02;
pub const ISFS_OPEN_RW: u8 = ISFS_OPEN_READ | ISFS_OPEN_WRITE;

pub const ISFS_OK: i32 = 0;
pub const ISFS_ENOMEM: i32 = -22;
pub const ISFS_EINVAL: i32 = -101;
/// `/dev/fs` result for a path that does not exist.
///
/// This is distinct from `ogc_ipc::IPC_ENOENT` (`-6`), which belongs to the
/// IOS IPC layer rather than the filesystem service's error namespace.
pub const ISFS_ENOENT: i32 = -106;

fn close_if_valid(fd: i32, close: impl FnOnce() -> i32) -> i32 {
    if fd < 0 { ISFS_OK } else { close() }
}

const DEV_FS: &CStr = c"/dev/fs";

static ISFS_INITIALIZED: AtomicBool = AtomicBool::new(false);
static FS_SERVICE_FD: AtomicI32 = AtomicI32::new(-1);

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Fstats {
    pub file_length: u32,
    pub file_pos: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct IsfsStats {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub e: u32,
    pub f: u32,
    pub g: u32,
}

#[repr(C, align(32))]
struct AlignedPath {
    bytes: [u8; ISFS_MAXPATH],
}

impl AlignedPath {
    fn from_cstr(path: &CStr) -> Result<Self, i32> {
        let bytes = path.to_bytes_with_nul();
        if path.to_bytes().len() >= ISFS_MAXPATH || bytes.len() > ISFS_MAXPATH {
            return Err(ISFS_EINVAL);
        }

        let mut out = Self {
            bytes: [0; ISFS_MAXPATH],
        };
        out.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(out)
    }

    fn as_mut_ptr(&mut self) -> *mut c_void {
        self.bytes.as_mut_ptr().cast()
    }
}

#[repr(C, align(32))]
struct AlignedRenamePaths {
    old_path: [u8; ISFS_MAXPATH],
    new_path: [u8; ISFS_MAXPATH],
}

impl AlignedRenamePaths {
    fn new(old_path: &CStr, new_path: &CStr) -> Result<Self, i32> {
        let old_path_bytes = old_path.to_bytes_with_nul();
        let new_path_bytes = new_path.to_bytes_with_nul();
        if old_path.to_bytes().len() >= ISFS_MAXPATH || new_path.to_bytes().len() >= ISFS_MAXPATH {
            return Err(ISFS_EINVAL);
        }

        let mut paths = Self {
            old_path: [0; ISFS_MAXPATH],
            new_path: [0; ISFS_MAXPATH],
        };
        paths.old_path[..old_path_bytes.len()].copy_from_slice(old_path_bytes);
        paths.new_path[..new_path_bytes.len()].copy_from_slice(new_path_bytes);
        Ok(paths)
    }

    fn bytes(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(self.old_path).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

#[repr(C)]
struct FsAttrPayload {
    owner_id: u32,
    group_id: u16,
    filepath: [u8; ISFS_MAXPATH],
    ownerperm: u8,
    groupperm: u8,
    otherperm: u8,
    attributes: u8,
    pad0: [u8; 2],
}

#[repr(C, align(32))]
struct AlignedFsAttr {
    payload: FsAttrPayload,
}

impl AlignedFsAttr {
    fn new(
        path: &CStr,
        attributes: u8,
        owner_perm: u8,
        group_perm: u8,
        other_perm: u8,
    ) -> Result<Self, i32> {
        let path_bytes = path.to_bytes_with_nul();
        if path.to_bytes().len() >= ISFS_MAXPATH || path_bytes.len() > ISFS_MAXPATH {
            return Err(ISFS_EINVAL);
        }

        let mut out = Self {
            payload: FsAttrPayload {
                owner_id: 0,
                group_id: 0,
                filepath: [0; ISFS_MAXPATH],
                ownerperm: owner_perm,
                groupperm: group_perm,
                otherperm: other_perm,
                attributes,
                pad0: [0; 2],
            },
        };
        out.payload.filepath[..path_bytes.len()].copy_from_slice(path_bytes);
        Ok(out)
    }

    fn bytes(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(self.payload).cast::<u8>(),
                core::mem::size_of::<FsAttrPayload>(),
            )
        }
    }
}

pub struct IsfsFile {
    fd: FileDescriptor,
}

impl IsfsFile {
    pub fn from_raw(fd: i32) -> Self {
        Self {
            fd: FileDescriptor::new(fd),
        }
    }

    pub fn raw(&self) -> i32 {
        self.fd.raw()
    }

    pub fn write(&self, buffer: &[u8]) -> Result<usize, i32> {
        if buffer.is_empty() {
            return Err(ISFS_EINVAL);
        }
        self.fd.write(buffer)
    }

    pub fn write_all(&self, buffer: &[u8]) -> Result<(), i32> {
        if buffer.is_empty() {
            return Err(ISFS_EINVAL);
        }
        self.fd.write_exact(buffer)
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, i32> {
        validate_isfs_read_buffer(buffer)?;
        self.fd.read(buffer)
    }

    pub fn seek(&self, where_: i32, whence: i32) -> Result<i32, i32> {
        self.fd.seek(where_, whence)
    }

    /// Starts a read and calls `callback` when IOS completes the request.
    ///
    /// # Safety
    ///
    /// `buffer` must remain writable and valid until `callback` returns. The
    /// callback and `userdata` must remain valid for the same duration.
    pub unsafe fn read_async(
        &self,
        buffer: &mut [u8],
        callback: Ipccallback,
        userdata: *mut c_void,
    ) -> i32 {
        if let Err(rc) = validate_isfs_read_buffer(buffer) {
            return rc;
        }
        unsafe { self.fd.read_async(buffer, callback, userdata) }
    }

    /// Starts a write and calls `callback` when IOS completes the request.
    ///
    /// # Safety
    ///
    /// `buffer` must remain readable and valid until `callback` returns. The
    /// callback and `userdata` must remain valid for the same duration.
    pub unsafe fn write_async(
        &self,
        buffer: &[u8],
        callback: Ipccallback,
        userdata: *mut c_void,
    ) -> i32 {
        if buffer.is_empty() {
            return ISFS_EINVAL;
        }
        unsafe { self.fd.write_async(buffer, callback, userdata) }
    }

    /// Starts a seek and calls `callback` when IOS completes the request.
    ///
    /// # Safety
    ///
    /// The callback and `userdata` must remain valid until the callback returns.
    pub unsafe fn seek_async(
        &self,
        where_: i32,
        whence: i32,
        callback: Ipccallback,
        userdata: *mut c_void,
    ) -> i32 {
        unsafe { self.fd.seek_async(where_, whence, callback, userdata) }
    }

    pub fn close(self) -> i32 {
        close_if_valid(self.fd.raw(), || {
            self.fd.close().map_or_else(|rc| rc, |_| ISFS_OK)
        })
    }

    /// Closes the file and calls `callback` when IOS completes the request.
    ///
    /// # Safety
    ///
    /// The callback and `userdata` must remain valid until the callback returns.
    pub unsafe fn close_async(self, callback: Ipccallback, userdata: *mut c_void) -> i32 {
        close_if_valid(self.fd.raw(), || unsafe {
            self.fd.close_async(callback, userdata)
        })
    }

    /// Starts a file-stats query and copies the result before calling `callback`.
    ///
    /// # Safety
    ///
    /// `status` must remain writable and valid until `callback` returns. The
    /// callback and `userdata` must remain valid for the same duration.
    pub unsafe fn get_stats_async(
        &self,
        status: &mut Fstats,
        callback: Ipccallback,
        userdata: *mut c_void,
    ) -> i32 {
        if !is_32_byte_aligned((status as *mut Fstats).cast()) {
            return ISFS_EINVAL;
        }
        unsafe {
            submit_async_output_ioctl(
                &self.fd,
                ISFS_IOCTL_GETFILESTATS,
                (status as *mut Fstats).cast(),
                core::mem::size_of::<Fstats>(),
                false,
                callback,
                userdata,
            )
        }
    }
}

fn open_fs_service() -> Result<FileDescriptor, i32> {
    let fd = FS_SERVICE_FD.load(Ordering::Acquire);
    if !ISFS_INITIALIZED.load(Ordering::Acquire) || fd < 0 {
        return Err(ISFS_EINVAL);
    }
    Ok(FileDescriptor::new(fd))
}

fn run_fs_ioctl(ioctl: i32, buffers: IoctlBuffers<'_>) -> i32 {
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    fd.ioctl(ioctl, buffers)
}

fn run_fs_ioctlv<'a, const N: usize>(ioctl: i32, cnt_in: usize, argv: [IoctlvEntry<'a>; N]) -> i32 {
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    fd.ioctlv(ioctl, cnt_in, argv)
}

pub fn initialize() -> i32 {
    if ISFS_INITIALIZED.load(Ordering::Acquire) && FS_SERVICE_FD.load(Ordering::Acquire) >= 0 {
        return ISFS_OK;
    }

    match open_file(DEV_FS, IPC_OPEN_NONE) {
        Ok(fd) => {
            FS_SERVICE_FD.store(fd.raw(), Ordering::Release);
            ISFS_INITIALIZED.store(true, Ordering::Release);
            ISFS_OK
        }
        Err(rc) => {
            FS_SERVICE_FD.store(-1, Ordering::Release);
            rc
        }
    }
}

pub fn deinitialize() -> i32 {
    if ISFS_INITIALIZED.swap(false, Ordering::AcqRel) {
        let fd = FS_SERVICE_FD.swap(-1, Ordering::AcqRel);
        if fd >= 0 {
            FileDescriptor::new(fd)
                .close()
                .map_or_else(|rc| rc, |_| ISFS_OK)
        } else {
            ISFS_OK
        }
    } else {
        ISFS_EINVAL
    }
}

pub fn format() -> i32 {
    run_fs_ioctl(ISFS_IOCTL_FORMAT, IoctlBuffers::empty())
}

/// Starts ISFS formatting and calls `callback` when IOS completes the request.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn format_async(callback: Ipccallback, userdata: *mut c_void) -> i32 {
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    unsafe {
        fd.ioctl_async(
            ISFS_IOCTL_FORMAT,
            IoctlBuffers::empty(),
            Some(callback.unwrap_or(isfs_noop_callback)),
            userdata,
        )
    }
}

/// Reads a directory into packed, NUL-terminated entry names.
///
/// When `name_list` is `Some`, `num` gives the entry capacity on input and the
/// number of entries returned on success. Reserve 13 bytes per entry for the
/// maximum name size, but advance between returned names by their actual
/// length plus the NUL terminator. The buffer must be 32-byte aligned,
/// as required by ISFS. When `name_list` is `None`, ISFS returns the number of
/// entries in the directory without copying entry names.
pub fn read_dir(path: &CStr, name_list: Option<&mut [u8]>, num: &mut u32) -> i32 {
    let Ok(path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };

    let Some(name_list) = name_list else {
        let mut count = 0u32;
        let rc = run_fs_ioctlv(
            ISFS_IOCTL_READDIR,
            1,
            [
                IoctlvEntry::In(&path_buf.bytes),
                IoctlvEntry::Out(unsafe {
                    core::slice::from_raw_parts_mut(
                        &mut count as *mut u32 as *mut u8,
                        core::mem::size_of::<u32>(),
                    )
                }),
            ],
        );
        if rc == ISFS_OK {
            *num = count;
        }
        return rc;
    };

    let capacity = *num;
    let Some(list_len) = (capacity as usize).checked_mul(13) else {
        return ISFS_EINVAL;
    };
    if name_list.len() < list_len || (name_list.as_ptr() as usize) & 31 != 0 {
        return ISFS_EINVAL;
    }

    let input_count = capacity;
    let mut output_count = 0u32;
    let rc = run_fs_ioctlv(
        ISFS_IOCTL_READDIR,
        2,
        [
            IoctlvEntry::In(&path_buf.bytes),
            IoctlvEntry::In(unsafe {
                core::slice::from_raw_parts(
                    &input_count as *const u32 as *const u8,
                    core::mem::size_of::<u32>(),
                )
            }),
            IoctlvEntry::Out(&mut name_list[..list_len]),
            IoctlvEntry::Out(unsafe {
                core::slice::from_raw_parts_mut(
                    &mut output_count as *mut u32 as *mut u8,
                    core::mem::size_of::<u32>(),
                )
            }),
        ],
    );
    if is_isfs_success(rc) {
        *num = output_count;
    }
    rc
}

#[repr(C, align(32))]
struct ReadDirAsyncContext {
    path: [u8; ISFS_MAXPATH],
    vectors: [Ioctlv; 4],
    count: u32,
    num: *mut u32,
    callback: Ipccallback,
    userdata: *mut c_void,
}

const _: () = {
    assert!(core::mem::offset_of!(ReadDirAsyncContext, vectors) == 64);
    assert!(core::mem::offset_of!(ReadDirAsyncContext, count) == 96);
};

unsafe extern "C" fn read_dir_async_callback(result: i32, userdata: *mut c_void) -> i32 {
    let Some(context) = (unsafe { userdata.cast::<ReadDirAsyncContext>().as_mut() }) else {
        return ISFS_EINVAL;
    };
    unsafe { copy_read_dir_count_on_success(result, context.num, context.count) };
    if let Some(callback) = context.callback {
        unsafe { callback(result, context.userdata) };
    }
    if let Some(context_ptr) = core::ptr::NonNull::new(userdata) {
        unsafe { HeapId::new(0).free(context_ptr) };
    }
    ISFS_OK
}

unsafe fn copy_read_dir_count_on_success(result: i32, destination: *mut u32, count: u32) {
    if result == ISFS_OK {
        unsafe { *destination = count };
    }
}

/// Starts a directory read and calls `callback` when IOS completes the request.
///
/// `num` supplies the entry capacity when `name_list` is present and receives
/// the resulting entry count on success. The name buffer must be 32-byte
/// aligned. The path, name buffer, count, callback, and userdata must remain
/// valid until the callback completes.
///
/// # Safety
///
/// The caller must keep all borrowed buffers and callback state valid until
/// the completion callback returns.
pub unsafe fn read_dir_async(
    path: &CStr,
    name_list: Option<&mut [u8]>,
    num: &mut u32,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    let (initial_count, list_ptr, list_len, input_count, output_count) =
        if let Some(name_list) = name_list {
            let Some(list_len) = (*num as usize).checked_mul(13) else {
                return ISFS_EINVAL;
            };
            if name_list.len() < list_len || (name_list.as_ptr() as usize) & 31 != 0 {
                return ISFS_EINVAL;
            }
            (*num, name_list.as_mut_ptr(), list_len, 2usize, 2usize)
        } else {
            (0, core::ptr::null_mut(), 0, 1usize, 1usize)
        };

    let fd_raw = FS_SERVICE_FD.load(Ordering::Acquire);
    if fd_raw < 0 || !ISFS_INITIALIZED.load(Ordering::Acquire) {
        return ISFS_EINVAL;
    }

    let Ok(heap) = HeapId::default_heap() else {
        return ISFS_ENOMEM;
    };
    let Some(context_ptr) = heap.alloc(core::mem::size_of::<ReadDirAsyncContext>() as i32) else {
        return ISFS_ENOMEM;
    };
    let context = context_ptr.as_ptr().cast::<ReadDirAsyncContext>();
    unsafe {
        core::ptr::write(
            context,
            ReadDirAsyncContext {
                path: path_buf.bytes,
                vectors: [Ioctlv::default(); 4],
                count: initial_count,
                num,
                callback,
                userdata,
            },
        );
        let context = &mut *context;
        context.vectors[0] = Ioctlv {
            data: context.path.as_mut_ptr().cast(),
            len: ISFS_MAXPATH as u32,
        };
        if output_count == 1 {
            context.vectors[1] = Ioctlv {
                data: (&mut context.count as *mut u32).cast(),
                len: core::mem::size_of::<u32>() as u32,
            };
        } else {
            context.vectors[1] = Ioctlv {
                data: (&mut context.count as *mut u32).cast(),
                len: core::mem::size_of::<u32>() as u32,
            };
            context.vectors[2] = Ioctlv {
                data: list_ptr.cast(),
                len: list_len as u32,
            };
            context.vectors[3] = Ioctlv {
                data: (&mut context.count as *mut u32).cast(),
                len: core::mem::size_of::<u32>() as u32,
            };
        }
    }

    let fd = FileDescriptor::new(fd_raw);
    let result = unsafe {
        fd.ioctlv_async_raw(
            ISFS_IOCTL_READDIR,
            input_count,
            output_count,
            core::ptr::addr_of_mut!((*context).vectors).cast::<Ioctlv>(),
            Some(read_dir_async_callback),
            context.cast(),
        )
    };
    if result < 0 {
        unsafe { heap.free(context_ptr) };
    }
    result
}

pub fn get_stats(stats: &mut IsfsStats) -> i32 {
    let mut output = [0u8; core::mem::size_of::<IsfsStats>()];
    let rc = run_fs_ioctl(
        ISFS_IOCTL_GETSTATS,
        IoctlBuffers::from_slices(None, Some(&mut output)),
    );
    if rc == ISFS_OK {
        unsafe {
            core::ptr::copy_nonoverlapping(
                output.as_ptr(),
                (stats as *mut IsfsStats).cast(),
                output.len(),
            );
        }
    }
    rc
}

/// Starts an ISFS stats query and copies the result before calling `callback`.
///
/// # Safety
///
/// `stats` must remain writable and valid until `callback` returns. The
/// callback and `userdata` must remain valid for the same duration.
pub unsafe fn get_stats_async(
    stats: &mut IsfsStats,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    unsafe {
        submit_async_output_ioctl(
            &fd,
            ISFS_IOCTL_GETSTATS,
            (stats as *mut IsfsStats).cast(),
            core::mem::size_of::<IsfsStats>(),
            true,
            callback,
            userdata,
        )
    }
}

pub fn create_dir(
    path: &CStr,
    attributes: u8,
    owner_perm: u8,
    group_perm: u8,
    other_perm: u8,
) -> i32 {
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    run_fs_ioctl(
        ISFS_IOCTL_CREATEDIR,
        IoctlBuffers::from_slices(Some(attr.bytes()), None),
    )
}

/// Starts directory creation and calls `callback` when IOS completes it.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn create_dir_async(
    path: &CStr,
    attributes: u8,
    owner_perm: u8,
    group_perm: u8,
    other_perm: u8,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    unsafe { submit_async_input_ioctl(ISFS_IOCTL_CREATEDIR, attr.bytes(), callback, userdata) }
}

pub fn create_file(
    path: &CStr,
    attributes: u8,
    owner_perm: u8,
    group_perm: u8,
    other_perm: u8,
) -> i32 {
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    run_fs_ioctl(
        ISFS_IOCTL_CREATEFILE,
        IoctlBuffers::from_slices(Some(attr.bytes()), None),
    )
}

/// Starts file creation and calls `callback` when IOS completes it.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn create_file_async(
    path: &CStr,
    attributes: u8,
    owner_perm: u8,
    group_perm: u8,
    other_perm: u8,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    unsafe { submit_async_input_ioctl(ISFS_IOCTL_CREATEFILE, attr.bytes(), callback, userdata) }
}

pub fn get_file_stats(fd: &IsfsFile, status: &mut Fstats) -> i32 {
    if !is_32_byte_aligned((status as *mut Fstats).cast()) {
        return ISFS_EINVAL;
    }
    fd.fd.ioctl(
        ISFS_IOCTL_GETFILESTATS,
        IoctlBuffers::from_slices(
            None,
            Some(unsafe {
                core::slice::from_raw_parts_mut(
                    status as *mut Fstats as *mut u8,
                    core::mem::size_of::<Fstats>(),
                )
            }),
        ),
    )
}

/// Filesystem ownership, flags, and access permissions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FsAttributes {
    pub owner_id: u32,
    pub group_id: u16,
    pub attributes: u8,
    pub owner_perm: u8,
    pub group_perm: u8,
    pub other_perm: u8,
}

/// Writable outputs retained until an asynchronous attribute query completes.
pub struct FsAttributeOutputs<'a> {
    pub owner_id: &'a mut u32,
    pub group_id: &'a mut u16,
    pub attributes: &'a mut u8,
    pub owner_perm: &'a mut u8,
    pub group_perm: &'a mut u8,
    pub other_perm: &'a mut u8,
}

pub fn get_attr(
    path: &CStr,
    owner_id: &mut u32,
    group_id: &mut u16,
    attributes: &mut u8,
    owner_perm: &mut u8,
    group_perm: &mut u8,
    other_perm: &mut u8,
) -> i32 {
    let Ok(mut path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    let Ok(mut attr) = AlignedFsAttr::new(path, 0, 0, 0, 0) else {
        return ISFS_EINVAL;
    };
    let rc = run_fs_ioctl(
        ISFS_IOCTL_GETATTR,
        IoctlBuffers::from_slices(
            Some(unsafe {
                core::slice::from_raw_parts_mut(path_buf.as_mut_ptr().cast::<u8>(), ISFS_MAXPATH)
            }),
            Some(attr.bytes()),
        ),
    );
    if rc == ISFS_OK {
        *owner_id = attr.payload.owner_id;
        *group_id = attr.payload.group_id;
        *attributes = attr.payload.attributes;
        *owner_perm = attr.payload.ownerperm;
        *group_perm = attr.payload.groupperm;
        *other_perm = attr.payload.otherperm;
    }
    rc
}

/// Starts an ISFS attribute query and writes its outputs before `callback`.
///
/// # Safety
///
/// All output references, the callback, and `userdata` must remain valid until
/// the callback returns.
pub unsafe fn get_attr_async(
    path: &CStr,
    params: FsAttributeOutputs<'_>,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let FsAttributeOutputs {
        owner_id,
        group_id,
        attributes,
        owner_perm,
        group_perm,
        other_perm,
    } = params;
    let Ok(path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    let Ok(heap) = HeapId::default_heap() else {
        return ISFS_ENOMEM;
    };
    let Some(context_ptr) = heap.alloc(core::mem::size_of::<AsyncGetAttrContext>() as i32) else {
        return ISFS_ENOMEM;
    };
    let context = context_ptr.as_ptr().cast::<AsyncGetAttrContext>();
    unsafe {
        core::ptr::write(
            context,
            AsyncGetAttrContext {
                path: path_buf.bytes,
                output: FsAttrPayload {
                    owner_id: 0,
                    group_id: 0,
                    filepath: [0; ISFS_MAXPATH],
                    ownerperm: 0,
                    groupperm: 0,
                    otherperm: 0,
                    attributes: 0,
                    pad0: [0; 2],
                },
                owner_id,
                group_id,
                attributes,
                owner_perm,
                group_perm,
                other_perm,
                callback,
                userdata,
            },
        );
    }
    let result = unsafe {
        let context_ref = &mut *context;
        let output_bytes = core::slice::from_raw_parts_mut(
            (&mut context_ref.output as *mut FsAttrPayload).cast::<u8>(),
            core::mem::size_of::<FsAttrPayload>(),
        );
        fd.ioctl_async(
            ISFS_IOCTL_GETATTR,
            IoctlBuffers::from_slices(Some(&mut context_ref.path), Some(output_bytes)),
            Some(async_get_attr_callback),
            context.cast(),
        )
    };
    if result < 0 {
        unsafe { heap.free(context_ptr) };
    }
    result
}

pub fn set_attr(
    path: &CStr,
    owner_id: u32,
    group_id: u16,
    attributes: u8,
    owner_perm: u8,
    group_perm: u8,
    other_perm: u8,
) -> i32 {
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    attr.payload.owner_id = owner_id;
    attr.payload.group_id = group_id;
    run_fs_ioctl(
        ISFS_IOCTL_SETATTR,
        IoctlBuffers::from_slices(Some(attr.bytes()), None),
    )
}

/// Starts an ISFS attribute update and calls `callback` when IOS completes it.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn set_attr_async(
    path: &CStr,
    params: FsAttributes,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let FsAttributes {
        owner_id,
        group_id,
        attributes,
        owner_perm,
        group_perm,
        other_perm,
    } = params;
    let Ok(mut attr) = AlignedFsAttr::new(path, attributes, owner_perm, group_perm, other_perm)
    else {
        return ISFS_EINVAL;
    };
    attr.payload.owner_id = owner_id;
    attr.payload.group_id = group_id;
    unsafe { submit_async_input_ioctl(ISFS_IOCTL_SETATTR, attr.bytes(), callback, userdata) }
}

pub fn get_usage(path: &CStr, usage1: &mut u32, usage2: &mut u32) -> i32 {
    let Ok(path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    let mut out_usage1 = 0u32;
    let mut out_usage2 = 0u32;
    let rc = run_fs_ioctlv(
        ISFS_IOCTL_GETUSAGE,
        1,
        [
            IoctlvEntry::In(unsafe {
                core::slice::from_raw_parts(path_buf.bytes.as_ptr(), ISFS_MAXPATH)
            }),
            IoctlvEntry::Out(unsafe {
                core::slice::from_raw_parts_mut(
                    &mut out_usage1 as *mut u32 as *mut u8,
                    core::mem::size_of::<u32>(),
                )
            }),
            IoctlvEntry::Out(unsafe {
                core::slice::from_raw_parts_mut(
                    &mut out_usage2 as *mut u32 as *mut u8,
                    core::mem::size_of::<u32>(),
                )
            }),
        ],
    );
    if rc == ISFS_OK {
        *usage1 = out_usage1;
        *usage2 = out_usage2;
    }
    rc
}

/// Starts an ISFS usage query and writes its outputs before `callback`.
///
/// # Safety
///
/// Both output references, the callback, and `userdata` must remain valid until
/// the callback returns.
pub unsafe fn get_usage_async(
    path: &CStr,
    usage1: &mut u32,
    usage2: &mut u32,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    let fd_raw = FS_SERVICE_FD.load(Ordering::Acquire);
    if fd_raw < 0 || !ISFS_INITIALIZED.load(Ordering::Acquire) {
        return ISFS_EINVAL;
    }
    let Ok(heap) = HeapId::default_heap() else {
        return ISFS_ENOMEM;
    };
    let Some(context_ptr) = heap.alloc(core::mem::size_of::<AsyncGetUsageContext>() as i32) else {
        return ISFS_ENOMEM;
    };
    let context = context_ptr.as_ptr().cast::<AsyncGetUsageContext>();
    unsafe {
        core::ptr::write(
            context,
            AsyncGetUsageContext {
                path: path_buf.bytes,
                vectors: [Ioctlv::default(); 4],
                usage1: 0,
                pad0: [0; 28],
                usage2: 0,
                output_usage1: usage1,
                output_usage2: usage2,
                callback,
                userdata,
            },
        );
        let context_ref = &mut *context;
        context_ref.vectors[0] = Ioctlv {
            data: context_ref.path.as_mut_ptr().cast(),
            len: ISFS_MAXPATH as u32,
        };
        context_ref.vectors[1] = Ioctlv {
            data: (&mut context_ref.usage1 as *mut u32).cast(),
            len: core::mem::size_of::<u32>() as u32,
        };
        context_ref.vectors[2] = Ioctlv {
            data: (&mut context_ref.usage2 as *mut u32).cast(),
            len: core::mem::size_of::<u32>() as u32,
        };
    }
    let fd = FileDescriptor::new(fd_raw);
    let result = unsafe {
        fd.ioctlv_async_raw(
            ISFS_IOCTL_GETUSAGE,
            1,
            2,
            core::ptr::addr_of_mut!((*context).vectors).cast::<Ioctlv>(),
            Some(async_get_usage_callback),
            context.cast(),
        )
    };
    if result < 0 {
        unsafe { heap.free(context_ptr) };
    }
    result
}

pub fn delete(path: &CStr) -> i32 {
    let Ok(mut path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    run_fs_ioctl(
        ISFS_IOCTL_DELETE,
        IoctlBuffers::from_slices(
            Some(unsafe {
                core::slice::from_raw_parts_mut(path_buf.as_mut_ptr().cast::<u8>(), ISFS_MAXPATH)
            }),
            None,
        ),
    )
}

/// Starts deletion and calls `callback` when IOS completes it.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn delete_async(path: &CStr, callback: Ipccallback, userdata: *mut c_void) -> i32 {
    let Ok(mut path_buf) = AlignedPath::from_cstr(path) else {
        return ISFS_EINVAL;
    };
    unsafe {
        submit_async_input_ioctl(
            ISFS_IOCTL_DELETE,
            core::slice::from_raw_parts_mut(path_buf.bytes.as_mut_ptr(), ISFS_MAXPATH),
            callback,
            userdata,
        )
    }
}

/// Rename an ISFS path using the native `/dev/fs` rename ioctl.
pub fn rename(old_path: &CStr, new_path: &CStr) -> i32 {
    let Ok(mut paths) = AlignedRenamePaths::new(old_path, new_path) else {
        return ISFS_EINVAL;
    };
    run_fs_ioctl(
        ISFS_IOCTL_RENAME,
        IoctlBuffers::from_slices(Some(paths.bytes()), None),
    )
}

/// Starts a rename and calls `callback` when IOS completes it.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn rename_async(
    old_path: &CStr,
    new_path: &CStr,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    let Ok(mut paths) = AlignedRenamePaths::new(old_path, new_path) else {
        return ISFS_EINVAL;
    };
    unsafe { submit_async_input_ioctl(ISFS_IOCTL_RENAME, paths.bytes(), callback, userdata) }
}

pub fn open(path: &CStr, mode: u8) -> Result<IsfsFile, i32> {
    if !ISFS_INITIALIZED.load(Ordering::Acquire) {
        return Err(ISFS_EINVAL);
    }
    validate_isfs_path(path)?;
    open_file(path, mode as u32).map(|fd| IsfsFile { fd })
}

/// Starts opening an ISFS path and passes the resulting IOS descriptor to
/// `callback` when the request completes.
///
/// # Safety
///
/// The callback and `userdata` must remain valid until the callback returns.
pub unsafe fn open_async(
    path: &CStr,
    mode: u8,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    if !ISFS_INITIALIZED.load(Ordering::Acquire) {
        return ISFS_EINVAL;
    }
    if let Err(rc) = validate_isfs_path(path) {
        return rc;
    }
    open_file_async(
        path,
        mode as u32,
        Some(callback.unwrap_or(isfs_noop_callback)),
        userdata,
    )
}

unsafe extern "C" fn isfs_noop_callback(_result: i32, _userdata: *mut c_void) -> i32 {
    ISFS_OK
}

#[repr(C, align(32))]
struct AsyncIoctlContext {
    payload: [u8; ISFS_ASYNC_PAYLOAD_SIZE],
    payload_len: usize,
    callback: Ipccallback,
    userdata: *mut c_void,
}

#[repr(C, align(32))]
struct AsyncOutputContext {
    output: [u8; core::mem::size_of::<IsfsStats>()],
    output_len: usize,
    destination: *mut u8,
    copy_only_on_success: bool,
    callback: Ipccallback,
    userdata: *mut c_void,
}

#[repr(C, align(32))]
struct AsyncGetAttrContext {
    path: [u8; ISFS_MAXPATH],
    output: FsAttrPayload,
    owner_id: *mut u32,
    group_id: *mut u16,
    attributes: *mut u8,
    owner_perm: *mut u8,
    group_perm: *mut u8,
    other_perm: *mut u8,
    callback: Ipccallback,
    userdata: *mut c_void,
}

#[repr(C, align(32))]
struct AsyncGetUsageContext {
    path: [u8; ISFS_MAXPATH],
    vectors: [Ioctlv; 4],
    usage1: u32,
    pad0: [u8; 28],
    usage2: u32,
    output_usage1: *mut u32,
    output_usage2: *mut u32,
    callback: Ipccallback,
    userdata: *mut c_void,
}

const _: () = {
    assert!(core::mem::offset_of!(AsyncGetUsageContext, vectors) == 64);
    assert!(core::mem::offset_of!(AsyncGetUsageContext, usage1) == 96);
    assert!(core::mem::offset_of!(AsyncGetUsageContext, usage2) == 128);
};

unsafe extern "C" fn async_ioctl_callback(result: i32, userdata: *mut c_void) -> i32 {
    let Some(context) = (unsafe { userdata.cast::<AsyncIoctlContext>().as_mut() }) else {
        return ISFS_EINVAL;
    };
    if let Some(callback) = context.callback {
        unsafe { callback(result, context.userdata) };
    }
    if let Some(context_ptr) = core::ptr::NonNull::new(userdata) {
        unsafe { HeapId::new(0).free(context_ptr) };
    }
    result
}

unsafe extern "C" fn async_output_ioctl_callback(result: i32, userdata: *mut c_void) -> i32 {
    let Some(context) = (unsafe { userdata.cast::<AsyncOutputContext>().as_mut() }) else {
        return ISFS_EINVAL;
    };
    if is_isfs_success(result) || !context.copy_only_on_success {
        unsafe {
            core::ptr::copy_nonoverlapping(
                context.output.as_ptr(),
                context.destination,
                context.output_len,
            );
        }
    }
    if let Some(callback) = context.callback {
        unsafe { callback(result, context.userdata) };
    }
    if let Some(context_ptr) = core::ptr::NonNull::new(userdata) {
        unsafe { HeapId::new(0).free(context_ptr) };
    }
    result
}

unsafe extern "C" fn async_get_attr_callback(result: i32, userdata: *mut c_void) -> i32 {
    let Some(context) = (unsafe { userdata.cast::<AsyncGetAttrContext>().as_mut() }) else {
        return ISFS_EINVAL;
    };
    if is_isfs_success(result) {
        unsafe {
            *context.owner_id = context.output.owner_id;
            *context.group_id = context.output.group_id;
            *context.attributes = context.output.attributes;
            *context.owner_perm = context.output.ownerperm;
            *context.group_perm = context.output.groupperm;
            *context.other_perm = context.output.otherperm;
        }
    }
    if let Some(callback) = context.callback {
        unsafe { callback(result, context.userdata) };
    }
    if let Some(context_ptr) = core::ptr::NonNull::new(userdata) {
        unsafe { HeapId::new(0).free(context_ptr) };
    }
    result
}

unsafe extern "C" fn async_get_usage_callback(result: i32, userdata: *mut c_void) -> i32 {
    let Some(context) = (unsafe { userdata.cast::<AsyncGetUsageContext>().as_mut() }) else {
        return ISFS_EINVAL;
    };
    if is_isfs_success(result) {
        unsafe {
            *context.output_usage1 = context.usage1;
            *context.output_usage2 = context.usage2;
        }
    }
    if let Some(callback) = context.callback {
        unsafe { callback(result, context.userdata) };
    }
    if let Some(context_ptr) = core::ptr::NonNull::new(userdata) {
        unsafe { HeapId::new(0).free(context_ptr) };
    }
    result
}

unsafe fn submit_async_output_ioctl(
    fd: &FileDescriptor,
    ioctl: i32,
    destination: *mut u8,
    output_len: usize,
    copy_only_on_success: bool,
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    if output_len > core::mem::size_of::<IsfsStats>() || destination.is_null() {
        return ISFS_EINVAL;
    }
    let Ok(heap) = HeapId::default_heap() else {
        return ISFS_ENOMEM;
    };
    let Some(context_ptr) = heap.alloc(core::mem::size_of::<AsyncOutputContext>() as i32) else {
        return ISFS_ENOMEM;
    };
    let context = context_ptr.as_ptr().cast::<AsyncOutputContext>();
    unsafe {
        core::ptr::write(
            context,
            AsyncOutputContext {
                output: [0; core::mem::size_of::<IsfsStats>()],
                output_len,
                destination,
                copy_only_on_success,
                callback,
                userdata,
            },
        );
        if !copy_only_on_success {
            core::ptr::copy_nonoverlapping(destination, (*context).output.as_mut_ptr(), output_len);
        }
    }
    let result = unsafe {
        let context_ref = &mut *context;
        fd.ioctl_async(
            ioctl,
            IoctlBuffers::from_slices(None, Some(&mut context_ref.output[..output_len])),
            Some(async_output_ioctl_callback),
            context.cast(),
        )
    };
    if result < 0 {
        unsafe { heap.free(context_ptr) };
    }
    result
}

fn is_isfs_success(result: i32) -> bool {
    result == ISFS_OK
}

unsafe fn submit_async_input_ioctl(
    ioctl: i32,
    payload: &[u8],
    callback: Ipccallback,
    userdata: *mut c_void,
) -> i32 {
    if payload.len() > ISFS_ASYNC_PAYLOAD_SIZE {
        return ISFS_EINVAL;
    }
    let Ok(fd) = open_fs_service() else {
        return ISFS_EINVAL;
    };
    let Ok(heap) = HeapId::default_heap() else {
        return ISFS_ENOMEM;
    };
    let Some(context_ptr) = heap.alloc(core::mem::size_of::<AsyncIoctlContext>() as i32) else {
        return ISFS_ENOMEM;
    };
    let context = context_ptr.as_ptr().cast::<AsyncIoctlContext>();
    unsafe {
        core::ptr::write(
            context,
            AsyncIoctlContext {
                payload: [0; ISFS_ASYNC_PAYLOAD_SIZE],
                payload_len: payload.len(),
                callback,
                userdata,
            },
        );
        core::ptr::copy_nonoverlapping(
            payload.as_ptr(),
            (*context).payload.as_mut_ptr(),
            payload.len(),
        );
    }
    let rc = unsafe {
        let context_ref = &mut *context;
        fd.ioctl_async(
            ioctl,
            IoctlBuffers::from_slices(
                Some(&mut context_ref.payload[..context_ref.payload_len]),
                None,
            ),
            Some(async_ioctl_callback),
            context.cast(),
        )
    };
    if rc < 0 {
        unsafe { heap.free(context_ptr) };
    }
    rc
}

fn validate_isfs_path(path: &CStr) -> Result<(), i32> {
    let bytes = path.to_bytes_with_nul();
    if path.to_bytes().len() >= ISFS_MAXPATH || bytes.len() > ISFS_MAXPATH {
        Err(ISFS_EINVAL)
    } else {
        Ok(())
    }
}

fn validate_isfs_read_buffer(buffer: &[u8]) -> Result<(), i32> {
    if buffer.is_empty() || !is_32_byte_aligned(buffer.as_ptr()) {
        Err(ISFS_EINVAL)
    } else {
        Ok(())
    }
}

fn is_32_byte_aligned(pointer: *const u8) -> bool {
    (pointer as usize) & 31 == 0
}

pub fn mount(name: &CStr, disc_interface: &DiscInterface) -> bool {
    iso9660_rs::mount(name, disc_interface)
}

pub fn unmount(name: &CStr) -> bool {
    iso9660_rs::unmount(name)
}

pub fn volume_label(name: &CStr) -> *const c_char {
    iso9660_rs::volume_label(name)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn stats_and_fstats_are_layout_stable() {
        assert_eq!(size_of::<Fstats>(), 8);
        assert_eq!(align_of::<Fstats>(), align_of::<u32>());
        assert_eq!(size_of::<IsfsStats>(), 28);
        assert_eq!(align_of::<IsfsStats>(), align_of::<u32>());
    }

    #[test]
    fn isfs_read_requires_a_nonempty_32_byte_aligned_buffer() {
        #[repr(align(32))]
        struct Aligned([u8; 64]);

        let aligned = Aligned([0; 64]);
        assert!(validate_isfs_read_buffer(&aligned.0).is_ok());
        assert_eq!(validate_isfs_read_buffer(&aligned.0[1..]), Err(ISFS_EINVAL));
        assert_eq!(validate_isfs_read_buffer(&[]), Err(ISFS_EINVAL));
        assert!(is_32_byte_aligned(aligned.0.as_ptr()));
        assert!(!is_32_byte_aligned(aligned.0[1..].as_ptr()));
    }

    #[test]
    fn stats_copy_only_on_zero_result() {
        assert!(is_isfs_success(ISFS_OK));
        assert!(!is_isfs_success(1));
        assert!(!is_isfs_success(-1));
    }

    #[test]
    fn negative_raw_close_skips_ipc_and_returns_success() {
        let mut called = false;
        let result = close_if_valid(-1, || {
            called = true;
            -7
        });

        assert_eq!(result, ISFS_OK);
        assert!(!called);
    }

    #[test]
    fn nonnegative_raw_close_runs_ipc_operation() {
        let mut called = false;
        let result = close_if_valid(3, || {
            called = true;
            9
        });

        assert_eq!(result, 9);
        assert!(called);
    }

    #[test]
    fn async_read_dir_count_copy_matches_c_zero_result_rule() {
        let mut count = 7;
        unsafe { copy_read_dir_count_on_success(1, &mut count, 12) };
        assert_eq!(count, 7);

        unsafe { copy_read_dir_count_on_success(ISFS_OK, &mut count, 12) };
        assert_eq!(count, 12);
    }

    #[test]
    fn metadata_helpers_exist_without_backend() {
        let _ = get_stats as fn(&mut IsfsStats) -> i32;
        let _ = get_file_stats as fn(&IsfsFile, &mut Fstats) -> i32;
        let _ =
            get_attr as fn(&CStr, &mut u32, &mut u16, &mut u8, &mut u8, &mut u8, &mut u8) -> i32;
        let _ = set_attr as fn(&CStr, u32, u16, u8, u8, u8, u8) -> i32;
        let _ = get_usage as fn(&CStr, &mut u32, &mut u32) -> i32;
        let _ = rename as fn(&CStr, &CStr) -> i32;
        let _ = run_fs_ioctl as fn(i32, IoctlBuffers<'_>) -> i32;
        let _ = run_fs_ioctlv::<3> as fn(i32, usize, [IoctlvEntry<'_>; 3]) -> i32;
    }

    #[test]
    fn attr_payload_keeps_expected_offsets() {
        assert_eq!(offset_of!(FsAttrPayload, owner_id), 0);
        assert_eq!(offset_of!(FsAttrPayload, group_id), 4);
        assert_eq!(offset_of!(FsAttrPayload, filepath), 6);
        assert_eq!(offset_of!(FsAttrPayload, ownerperm), 70);
    }

    #[test]
    fn rename_paths_match_isfs_ioctl_layout() {
        assert_eq!(size_of::<AlignedRenamePaths>(), ISFS_MAXPATH * 2);
        assert_eq!(align_of::<AlignedRenamePaths>(), 32);
        assert_eq!(offset_of!(AlignedRenamePaths, old_path), 0);
        assert_eq!(offset_of!(AlignedRenamePaths, new_path), ISFS_MAXPATH);

        let mut paths = AlignedRenamePaths::new(c"/title/old", c"/title/new").unwrap();
        assert_eq!(&paths.old_path[..11], b"/title/old\0");
        assert_eq!(&paths.new_path[..11], b"/title/new\0");
        assert!(paths.old_path[11..].iter().all(|byte| *byte == 0));
        assert!(paths.new_path[11..].iter().all(|byte| *byte == 0));
        assert_eq!(paths.bytes().len(), ISFS_MAXPATH * 2);
    }

    #[test]
    fn rename_rejects_paths_that_exceed_isfs_limit() {
        let mut too_long_bytes = [b'a'; ISFS_MAXPATH + 1];
        too_long_bytes[ISFS_MAXPATH] = 0;
        let too_long = CStr::from_bytes_with_nul(&too_long_bytes).unwrap();
        assert_eq!(
            AlignedRenamePaths::new(too_long, c"/title/new").err(),
            Some(ISFS_EINVAL)
        );
        assert_eq!(
            AlignedRenamePaths::new(c"/title/old", too_long).err(),
            Some(ISFS_EINVAL)
        );
    }
}
