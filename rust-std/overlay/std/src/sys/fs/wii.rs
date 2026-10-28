//! FAT files and directories, with application current-directory resolution.
use crate::ffi::OsString;
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::path::{Path, PathBuf};
use crate::sync::{Arc, Mutex};
pub use crate::sys::fs::common::{Dir, ExtraHomeDirs, ExtraMediaDirs};
use crate::sys::time::SystemTime;
use crate::sys::unsupported;
use crate::sys::wii::{FsRequest, fs_abi};
use fs_abi::op;

fn call(request: &mut FsRequest) -> io::Result<i64> {
    let value = unsafe { crate::sys::wii::fs(request) };
    if value < 0 {
        Err(io::Error::from_raw_os_error((-value) as i32))
    } else {
        Ok(value)
    }
}
fn path_request(operation: u32, path: &Path) -> io::Result<(FsRequest, PathBuf)> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::from_raw_os_error(2));
    }
    let path = crate::sys::path::absolute(path)?;
    let bytes = path.as_os_str().as_encoded_bytes();
    fs_abi::validate_path(bytes).map_err(io::Error::from_raw_os_error)?;
    let mut request = FsRequest::new(operation, crate::ptr::null_mut());
    request.data = bytes.as_ptr().cast_mut();
    request.length = bytes.len();
    Ok((request, path))
}
fn path_unit(operation: u32, path: &Path) -> io::Result<()> {
    let (mut request, _path) = path_request(operation, path)?;
    call(&mut request).map(|_| ())
}

#[derive(Debug)]
struct Handle(*mut core::ffi::c_void);
// SAFETY: One Handle is owned by Arc<Mutex<Handle>>. Every ABI operation holds
// this std mutex; all clones share it. FAT holds the mount until final close.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(error) = call(&mut FsRequest::new(op::CLOSE, self.0)) {
            panic!("Wii file cleanup failed: {error}");
        }
    }
}
#[derive(Debug)]
pub struct File(Arc<Mutex<Handle>>);
#[derive(Clone)]
pub struct FileAttr {
    size: u64,
    mode: u32,
    times: [i64; 3],
    created_nanos: u32,
}
#[derive(Debug)]
struct DirectoryHandle(*mut core::ffi::c_void);
// SAFETY: Directory operations and final release are serialized by a Mutex.
unsafe impl Send for DirectoryHandle {}
impl Drop for DirectoryHandle {
    fn drop(&mut self) {
        if let Err(error) = call(&mut FsRequest::new(op::CLOSE_DIR, self.0)) {
            panic!("Wii directory cleanup failed: {error}");
        }
    }
}
#[derive(Debug)]
pub struct ReadDir {
    handle: Mutex<DirectoryHandle>,
    root: Arc<PathBuf>,
    finished: bool,
}
pub struct DirEntry {
    root: Arc<PathBuf>,
    name: OsString,
    kind: FileType,
}
#[derive(Clone, Debug)]
pub struct OpenOptions {
    bits: u32,
}
#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {
    accessed: Option<SystemTime>,
    modified: Option<SystemTime>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePermissions(bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileType(u32);
#[derive(Debug)]
pub struct DirBuilder {}

impl FileAttr {
    fn from_request(r: &FsRequest) -> Self {
        Self {
            size: r.size,
            mode: r.mode,
            times: r.times,
            created_nanos: r.created_nanos,
        }
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn perm(&self) -> FilePermissions {
        FilePermissions(self.mode & 0o222 == 0)
    }
    pub fn file_type(&self) -> FileType {
        FileType(self.mode & 0o170000)
    }
    fn timestamp(&self, index: usize) -> io::Result<SystemTime> {
        let seconds = self.times[index];
        if seconds == -1 {
            return unsupported();
        }
        let seconds = u64::try_from(seconds).map_err(|_| io::Error::from_raw_os_error(5))?;
        let nanos = if index == 2 { self.created_nanos } else { 0 };
        if nanos >= 1_000_000_000 {
            return Err(io::Error::from_raw_os_error(5));
        }
        crate::sys::time::UNIX_EPOCH
            .checked_add_duration(&crate::time::Duration::new(seconds, nanos))
            .ok_or_else(|| io::Error::from_raw_os_error(139))
    }
    pub fn modified(&self) -> io::Result<SystemTime> {
        self.timestamp(1)
    }
    pub fn accessed(&self) -> io::Result<SystemTime> {
        self.timestamp(0)
    }
    pub fn created(&self) -> io::Result<SystemTime> {
        self.timestamp(2)
    }
}
impl FilePermissions {
    pub fn readonly(&self) -> bool {
        self.0
    }
    pub fn set_readonly(&mut self, readonly: bool) {
        self.0 = readonly;
    }
}
impl FileTimes {
    pub fn set_accessed(&mut self, time: SystemTime) {
        self.accessed = Some(time);
    }
    pub fn set_modified(&mut self, time: SystemTime) {
        self.modified = Some(time);
    }
}
impl FileType {
    pub fn is_dir(&self) -> bool {
        self.0 == 0o040000
    }
    pub fn is_file(&self) -> bool {
        self.0 == 0o100000
    }
    pub fn is_symlink(&self) -> bool {
        self.0 == 0o120000
    }
}
impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let mut name = [0u8; fs_abi::DIR_NAME_MAX];
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut request = FsRequest::new(op::READ_DIR, handle.0);
        request.data = name.as_mut_ptr();
        request.length = name.len();
        let result = (|| {
            match call(&mut request)? {
                0 => Ok(None),
                1 if request.length <= name.len() => {
                    let bytes = &name[..request.length];
                    if !fs_abi::directory_name_visible(bytes)
                        .map_err(io::Error::from_raw_os_error)?
                        || !matches!(request.mode, 0o040000 | 0o100000)
                    {
                        return Err(io::Error::from_raw_os_error(5));
                    }
                    // SAFETY: Wii OsString stores arbitrary bytes, as in args_os.
                    let name = unsafe { OsString::from_encoded_bytes_unchecked(bytes.to_vec()) };
                    Ok(Some(DirEntry {
                        root: self.root.clone(),
                        name,
                        kind: FileType(request.mode),
                    }))
                }
                _ => Err(io::Error::from_raw_os_error(5)),
            }
        })();
        match result {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}
impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.root.join(&self.name)
    }
    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }
    pub fn metadata(&self) -> io::Result<FileAttr> {
        stat(&self.path())
    }
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.kind)
    }
}
impl OpenOptions {
    pub fn new() -> Self {
        Self { bits: 0 }
    }
    fn set(&mut self, bit: u32, enabled: bool) {
        if enabled {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }
    pub fn read(&mut self, value: bool) {
        self.set(fs_abi::READ, value);
    }
    pub fn write(&mut self, value: bool) {
        self.set(fs_abi::WRITE, value);
    }
    pub fn append(&mut self, value: bool) {
        self.set(fs_abi::APPEND, value);
    }
    pub fn truncate(&mut self, value: bool) {
        self.set(fs_abi::TRUNCATE, value);
    }
    pub fn create(&mut self, value: bool) {
        self.set(fs_abi::CREATE, value);
    }
    pub fn create_new(&mut self, value: bool) {
        self.set(fs_abi::CREATE_NEW, value);
    }
}
impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<Self> {
        fs_abi::open_flags(opts.bits).map_err(io::Error::from_raw_os_error)?;
        let (mut request, _path) = path_request(op::OPEN, path)?;
        request.flags = opts.bits;
        let pointer = call(&mut request)? as usize as *mut core::ffi::c_void;
        if pointer.is_null() {
            return Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "null Wii file handle"
            ));
        }
        Ok(Self(Arc::new(Mutex::new(Handle(pointer)))))
    }
    fn with_request<T>(
        &self,
        operation: u32,
        f: impl FnOnce(&mut FsRequest) -> io::Result<T>,
    ) -> io::Result<T> {
        let handle = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        f(&mut FsRequest::new(operation, handle.0))
    }
    fn unit(&self, operation: u32) -> io::Result<()> {
        self.with_request(operation, |r| call(r).map(|_| ()))
    }
    pub fn file_attr(&self) -> io::Result<FileAttr> {
        self.with_request(op::FILE_METADATA, |r| {
            call(r)?;
            Ok(FileAttr::from_request(r))
        })
    }
    pub fn fsync(&self) -> io::Result<()> {
        self.unit(op::SYNC)
    }
    pub fn datasync(&self) -> io::Result<()> {
        self.fsync()
    }
    pub fn flush(&self) -> io::Result<()> {
        self.fsync()
    }
    pub fn lock(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn lock_shared(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::Error::UNSUPPORTED_PLATFORM))
    }
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        self.try_lock()
    }
    pub fn unlock(&self) -> io::Result<()> {
        unsupported()
    }
    pub fn truncate(&self, size: u64) -> io::Result<()> {
        self.with_request(op::SET_LEN, |r| {
            r.size = size;
            call(r).map(|_| ())
        })
    }
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.with_request(op::READ, |r| {
            r.data = buf.as_mut_ptr();
            r.length = buf.len();
            call(r).map(|v| v as usize)
        })
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.with_request(op::WRITE, |r| {
            r.data = buf.as_ptr().cast_mut();
            r.length = buf.len();
            call(r).map(|v| v as usize)
        })
    }
    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        io::default_read_vectored(|b| self.read(b), bufs)
    }
    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        io::default_write_vectored(|b| self.write(b), bufs)
    }
    pub fn is_read_vectored(&self) -> bool {
        false
    }
    pub fn is_write_vectored(&self) -> bool {
        false
    }
    pub fn read_buf(&self, cursor: BorrowedCursor<'_, u8>) -> io::Result<()> {
        io::default_read_buf(|b| self.read(b), cursor)
    }
    pub fn seek(&self, position: SeekFrom) -> io::Result<u64> {
        self.with_request(op::SEEK, |r| {
            let (offset, origin) = match position {
                SeekFrom::Start(value) => (
                    i64::try_from(value).map_err(|_| io::Error::from_raw_os_error(27))?,
                    0,
                ),
                SeekFrom::Current(value) => (value, 1),
                SeekFrom::End(value) => (value, 2),
            };
            r.offset = offset;
            r.flags = origin;
            call(r).map(|v| v as u64)
        })
    }
    pub fn tell(&self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
    pub fn size(&self) -> Option<io::Result<u64>> {
        Some(self.file_attr().map(|attr| attr.size()))
    }
    pub fn duplicate(&self) -> io::Result<Self> {
        Ok(Self(self.0.clone()))
    }
    pub fn set_permissions(&self, perm: FilePermissions) -> io::Result<()> {
        self.with_request(op::FILE_SET_PERMISSIONS, |r| {
            r.flags = u32::from(perm.0);
            call(r).map(|_| ())
        })
    }
    pub fn set_times(&self, times: FileTimes) -> io::Result<()> {
        self.with_request(op::FILE_SET_TIMES, |r| {
            prepare_times(r, times)?;
            call(r).map(|_| ())
        })
    }
}
impl DirBuilder {
    pub fn new() -> Self {
        Self {}
    }
    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        path_unit(op::MKDIR, p)
    }
}
pub fn readdir(p: &Path) -> io::Result<ReadDir> {
    let (mut request, path) = path_request(op::OPEN_DIR, p)?;
    let pointer = call(&mut request)? as usize as *mut core::ffi::c_void;
    if pointer.is_null() {
        return Err(io::Error::from_raw_os_error(5));
    }
    let handle = DirectoryHandle(pointer);
    Ok(ReadDir {
        handle: Mutex::new(handle),
        root: Arc::new(path),
        finished: false,
    })
}
pub fn unlink(p: &Path) -> io::Result<()> {
    path_unit(op::UNLINK, p)
}
pub fn rmdir(p: &Path) -> io::Result<()> {
    path_unit(op::RMDIR, p)
}
pub fn stat(p: &Path) -> io::Result<FileAttr> {
    let (mut r, _path) = path_request(op::PATH_METADATA, p)?;
    call(&mut r)?;
    Ok(FileAttr::from_request(&r))
}
pub fn lstat(p: &Path) -> io::Result<FileAttr> {
    stat(p)
}
pub fn exists(path: &Path) -> io::Result<bool> {
    crate::sys::fs::common::exists(path)
}
pub fn rename(old: &Path, new: &Path) -> io::Result<()> {
    two_paths(op::RENAME, old, new).map(|_| ())
}
pub fn set_perm(p: &Path, perm: FilePermissions) -> io::Result<()> {
    let (mut request, _path) = path_request(op::SET_PERMISSIONS, p)?;
    request.flags = u32::from(perm.0);
    call(&mut request).map(|_| ())
}
pub fn set_perm_nofollow(p: &Path, perm: FilePermissions) -> io::Result<()> {
    set_perm(p, perm)
}
fn prepare_times(request: &mut FsRequest, times: FileTimes) -> io::Result<()> {
    for (index, value) in [times.accessed, times.modified].into_iter().enumerate() {
        if let Some(value) = value {
            let duration = value
                .sub_time(&crate::sys::time::UNIX_EPOCH)
                .map_err(|_| io::Error::from_raw_os_error(22))?;
            request.times[index] =
                i64::try_from(duration.as_secs()).map_err(|_| io::Error::from_raw_os_error(22))?;
            request.flags |= 1 << index;
        }
    }
    Ok(())
}
pub fn set_times(p: &Path, times: FileTimes) -> io::Result<()> {
    let (mut request, _path) = path_request(op::SET_TIMES, p)?;
    prepare_times(&mut request, times)?;
    call(&mut request).map(|_| ())
}
pub fn set_times_nofollow(p: &Path, times: FileTimes) -> io::Result<()> {
    set_times(p, times)
}
pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    // FAT has no links. Iteration is explicit to bound stack use on deep trees.
    let root = canonicalize(path)?;
    if !stat(&root)?.file_type().is_dir() {
        return Err(io::Error::from_raw_os_error(20));
    }
    // Never permit an accidental request to erase a mounted volume's root.
    if root.parent().is_none() {
        return Err(io::Error::from_raw_os_error(16));
    }
    let mut pending = vec![(root.clone(), readdir(&root)?)];
    while let Some((directory, entries)) = pending.last_mut() {
        match entries.next() {
            Some(entry) => {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    let children = readdir(&path)?;
                    pending.push((path, children));
                } else {
                    unlink(&path)?;
                }
            }
            None => {
                let path = directory.clone();
                // FAT refuses removal while the directory iterator is open.
                drop(pending.pop());
                rmdir(&path)?;
            }
        }
    }
    Ok(())
}
pub fn readlink(_p: &Path) -> io::Result<PathBuf> {
    unsupported()
}
pub fn symlink(_original: &Path, _link: &Path) -> io::Result<()> {
    unsupported()
}
pub fn link(_src: &Path, _dst: &Path) -> io::Result<()> {
    unsupported()
}
pub fn canonicalize(p: &Path) -> io::Result<PathBuf> {
    let (mut request, _path) = path_request(op::CANONICALIZE, p)?;
    call(&mut request)?;
    loop {
        let length =
            usize::try_from(request.size).map_err(|_| io::Error::from_raw_os_error(139))?;
        let mut output = vec![0; length];
        request.second_path = output.as_mut_ptr();
        request.second_length = output.len();
        match call(&mut request) {
            Ok(count) if count > 0 && count as usize <= output.len() => {
                output.truncate(count as usize);
                fs_abi::validate_path(&output).map_err(io::Error::from_raw_os_error)?;
                return Ok(PathBuf::from(unsafe {
                    OsString::from_encoded_bytes_unchecked(output)
                }));
            }
            Err(error) if error.raw_os_error() == Some(34) && request.size > length as u64 => {
                continue;
            }
            Err(error) => return Err(error),
            Ok(_) => return Err(io::Error::from_raw_os_error(5)),
        }
    }
}
fn two_paths(operation: u32, from: &Path, to: &Path) -> io::Result<i64> {
    let (mut request, _from) = path_request(operation, from)?;
    let (_second, to) = path_request(operation, to)?;
    let bytes = to.as_os_str().as_encoded_bytes();
    request.second_path = bytes.as_ptr();
    request.second_length = bytes.len();
    call(&mut request)
}
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    two_paths(op::COPY, from, to).map(|count| count as u64)
}
