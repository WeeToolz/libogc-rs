//! Typed FAT and exFAT provider. No raw devoptab registration pointer escapes a lookup.
use alloc::boxed::Box;
use alloc::ffi::CString;
use core::ffi::{CStr, c_void};
use ogc_fat::{FatDirEntry, FatDirectory, FatFile, fat_errno};
use ogc_sysbase::{EBADF, EFBIG, EINVAL, EIO, ENOTSUP, EROFS};
#[path = "fs_abi.rs"]
pub mod abi;
pub use abi::FsRequest;
use abi::op;

unsafe fn bytes<'a>(r: &FsRequest) -> Result<&'a [u8], i32> {
    if r.length > isize::MAX as usize || (r.length != 0 && r.data.is_null()) {
        return Err(EINVAL);
    }
    if r.length == 0 {
        Ok(&[])
    } else {
        Ok(unsafe { core::slice::from_raw_parts(r.data, r.length) })
    }
}
enum FsFile {
    Fat(FatFile),
    Exfat(ogc_blockfs::File),
}

enum FsDirectory {
    Fat(FatDirectory),
    Exfat(ogc_blockfs::Directory),
}

struct FsDirEntry {
    name: alloc::vec::Vec<u8>,
    size: u64,
    is_directory: bool,
}

impl FsFile {
    fn len(&self) -> Result<u64, i32> {
        match self {
            Self::Fat(file) => Ok(u64::from(file.len())),
            Self::Exfat(file) => file.len().map_err(|error| ogc_blockfs::errno(&error)),
        }
    }

    fn position(&self) -> u64 {
        match self {
            Self::Fat(file) => u64::from(file.position()),
            Self::Exfat(file) => file.position(),
        }
    }

    fn read(&mut self, out: &mut [u8]) -> Result<usize, i32> {
        match self {
            Self::Fat(file) => file.read(out).map_err(fat_errno),
            Self::Exfat(file) => file.read(out).map_err(|error| ogc_blockfs::errno(&error)),
        }
    }

    fn write(&mut self, input: &[u8]) -> Result<usize, i32> {
        match self {
            Self::Fat(file) => file.write(input).map_err(fat_errno),
            Self::Exfat(_) => Err(EROFS),
        }
    }

    fn seek(&mut self, position: u64) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file
                .seek_checked(u32::try_from(position).map_err(|_| EFBIG)?)
                .map_err(fat_errno),
            Self::Exfat(file) => file
                .seek(position)
                .map_err(|error| ogc_blockfs::errno(&error)),
        }
    }

    fn flush(&mut self) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file.flush().map_err(fat_errno),
            Self::Exfat(_) => Ok(()),
        }
    }

    fn truncate(&mut self, size: u64) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file
                .truncate(u32::try_from(size).map_err(|_| EFBIG)?)
                .map_err(fat_errno),
            Self::Exfat(_) => Err(EROFS),
        }
    }

    fn set_times(&mut self, accessed: Option<u64>, modified: Option<u64>) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file.set_times(accessed, modified).map_err(fat_errno),
            Self::Exfat(_) => Err(EROFS),
        }
    }

    fn set_readonly(&mut self, readonly: bool) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file.set_readonly(readonly).map_err(fat_errno),
            Self::Exfat(_) => Err(EROFS),
        }
    }

    fn close(self) -> Result<(), i32> {
        match self {
            Self::Fat(file) => file.close().map_err(fat_errno),
            Self::Exfat(file) => {
                drop(file);
                Ok(())
            }
        }
    }
}

impl FsDirectory {
    fn next_entry(&mut self) -> Result<Option<FsDirEntry>, i32> {
        match self {
            Self::Fat(directory) => directory
                .next_entry()
                .map(|entry| {
                    entry.map(|entry: FatDirEntry| FsDirEntry {
                        name: entry.name,
                        size: u64::from(entry.size),
                        is_directory: entry.is_directory,
                    })
                })
                .map_err(fat_errno),
            Self::Exfat(directory) => directory
                .next()
                .map(|entry| {
                    entry.map(|entry| FsDirEntry {
                        name: entry.name.into_bytes(),
                        size: entry.size,
                        is_directory: entry.is_dir,
                    })
                })
                .map_err(|error| ogc_blockfs::errno(&error)),
        }
    }
}

unsafe fn file<'a>(handle: *mut c_void) -> Result<&'a mut FsFile, i32> {
    if handle.is_null() || !handle.cast::<FsFile>().is_aligned() {
        return Err(EBADF);
    }
    Ok(unsafe { &mut *handle.cast::<FsFile>() })
}
unsafe fn directory<'a>(handle: *mut c_void) -> Result<&'a mut FsDirectory, i32> {
    if handle.is_null() || !handle.cast::<FsDirectory>().is_aligned() {
        return Err(EBADF);
    }
    Ok(unsafe { &mut *handle.cast::<FsDirectory>() })
}
unsafe fn path(r: &FsRequest) -> Result<CString, i32> {
    let bytes = unsafe { bytes(r)? };
    abi::validate_path(bytes)?;
    CString::new(bytes).map_err(|_| EINVAL)
}
unsafe fn second_path(r: &FsRequest) -> Result<CString, i32> {
    let mut input = FsRequest::new(r.operation, core::ptr::null_mut());
    input.data = r.second_path.cast_mut();
    input.length = r.second_length;
    unsafe { path(&input) }
}

fn exfat_path(path: &CStr) -> Result<Option<&str>, i32> {
    let bytes = path.to_bytes();
    if !bytes.starts_with(b"sd:") || !ogc_blockfs::exfat_is_mounted() {
        return Ok(None);
    }
    core::str::from_utf8(&bytes[3..])
        .map(Some)
        .map_err(|_| EINVAL)
}

fn reject_exfat_mutation(path: &CStr) -> Result<(), i32> {
    if exfat_path(path)?.is_some() {
        Err(EROFS)
    } else {
        Ok(())
    }
}

fn open_fs_file(path: &CStr, flags: i32) -> Result<FsFile, i32> {
    if let Some(relative) = exfat_path(path)? {
        if flags != ogc_sysbase::O_RDONLY {
            return Err(EROFS);
        }
        return ogc_blockfs::open(relative)
            .map(FsFile::Exfat)
            .map_err(|error| ogc_blockfs::errno(&error));
    }
    FatFile::open_with_flags(path, flags)
        .map(FsFile::Fat)
        .map_err(fat_errno)
}

fn copy_file(from: &core::ffi::CStr, to: &core::ffi::CStr) -> Result<u64, i32> {
    // Source ownership prevents the destination open from truncating the same
    // FAT entry, including aliases. Independent writer opens retain EBUSY.
    let mut source = FatFile::open(from).map_err(fat_errno)?;
    let prepared = source
        .metadata()
        .map_err(fat_errno)
        .and_then(|(_, readonly)| {
            FatFile::open_with_flags(
                to,
                abi::open_flags(abi::WRITE | abi::CREATE | abi::TRUNCATE)?,
            )
            .map(|file| (file, readonly))
            .map_err(fat_errno)
        });
    let (mut destination, readonly) = match prepared {
        Ok(value) => value,
        Err(error) => {
            return abi::finish_copy(Err(error), || Ok(()), || source.close().map_err(fat_errno));
        }
    };
    let transferred = abi::copy_bytes(
        |buffer| source.read(buffer).map_err(fat_errno),
        |buffer| destination.write(buffer).map_err(fat_errno),
    )
    .and_then(|count| destination.flush().map(|()| count).map_err(fat_errno));
    let count = abi::finish_copy(
        transferred,
        || destination.close().map_err(fat_errno),
        || source.close().map_err(fat_errno),
    )?;
    let attributes = ogc_fat::get_attributes(to).map_err(fat_errno)?;
    let attributes = if readonly {
        attributes | ogc_fat::ATTR_READONLY
    } else {
        attributes & !ogc_fat::ATTR_READONLY
    };
    ogc_fat::set_attributes(to, attributes).map_err(fat_errno)?;
    Ok(count)
}

fn return_times(r: &mut FsRequest, times: ogc_fat::FatTimestamps) {
    r.times = [
        times.accessed.map_or(-1, |s| s as i64),
        times.modified.map_or(-1, |s| s as i64),
        times.created.map_or(-1, |(s, _)| s as i64),
    ];
    r.created_nanos = times.created.map_or(0, |(_, nanos)| nanos);
}

fn requested_times(r: &FsRequest) -> Result<(Option<u64>, Option<u64>), i32> {
    if r.flags & !3 != 0 {
        return Err(EINVAL);
    }
    let time = |index, flag| {
        if r.flags & flag == 0 {
            Ok(None)
        } else {
            u64::try_from(r.times[index]).map(Some).map_err(|_| EINVAL)
        }
    };
    Ok((time(0, 1)?, time(1, 2)?))
}

unsafe fn dispatch(r: &mut FsRequest) -> Result<i64, i32> {
    match r.operation {
        op::SET_TIMES => {
            let (accessed, modified) = requested_times(r)?;
            let path = unsafe { path(r)? };
            reject_exfat_mutation(&path)?;
            ogc_fat::set_times(&path, accessed, modified).map_err(fat_errno)?;
            Ok(0)
        }
        op::FILE_SET_TIMES => {
            let (accessed, modified) = requested_times(r)?;
            unsafe { file(r.handle)? }.set_times(accessed, modified)?;
            Ok(0)
        }
        op::CANONICALIZE => {
            let path = unsafe { path(r)? };
            if exfat_path(&path)?.is_some() {
                return Err(ENOTSUP);
            }
            let resolved = ogc_fat::canonicalize(&path).map_err(fat_errno)?;
            r.size = resolved.len() as u64;
            if r.second_length == 0 {
                return Ok(0);
            }
            if r.second_path.is_null() || r.second_length > isize::MAX as usize {
                return Err(EINVAL);
            }
            if r.second_length < resolved.len() {
                return Err(34);
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    resolved.as_ptr(),
                    r.second_path.cast_mut(),
                    resolved.len(),
                )
            };
            Ok(resolved.len() as i64)
        }
        op::RENAME => {
            let from = unsafe { path(r)? };
            let to = unsafe { second_path(r)? };
            reject_exfat_mutation(&from)?;
            reject_exfat_mutation(&to)?;
            ogc_fat::rename_replace(&from, &to).map_err(fat_errno)?;
            Ok(0)
        }
        op::SET_PERMISSIONS => {
            if r.flags > 1 {
                return Err(EINVAL);
            }
            let path = unsafe { path(r)? };
            reject_exfat_mutation(&path)?;
            ogc_fat::set_readonly(&path, r.flags != 0).map_err(fat_errno)?;
            Ok(0)
        }
        op::FILE_SET_PERMISSIONS => {
            if r.flags > 1 {
                return Err(EINVAL);
            }
            unsafe { file(r.handle)? }.set_readonly(r.flags != 0)?;
            Ok(0)
        }
        op::COPY => {
            let from = unsafe { path(r)? };
            let to = unsafe { second_path(r)? };
            reject_exfat_mutation(&from)?;
            reject_exfat_mutation(&to)?;
            copy_file(&from, &to).map(|count| count as i64)
        }
        op::OPEN_DIR => {
            let path = unsafe { path(r)? };
            let directory = if let Some(relative) = exfat_path(&path)? {
                FsDirectory::Exfat(
                    ogc_blockfs::open_dir(relative).map_err(|error| ogc_blockfs::errno(&error))?,
                )
            } else {
                FsDirectory::Fat(FatDirectory::open(&path).map_err(fat_errno)?)
            };
            Ok(Box::into_raw(Box::new(directory)) as usize as i64)
        }
        op::READ_DIR => {
            // Validate the full output extent before advancing the FAT cursor.
            unsafe { bytes(r)? };
            if r.length < abi::DIR_NAME_MAX {
                return Err(EINVAL);
            }
            let directory = unsafe { directory(r.handle)? };
            let Some(entry) =
                abi::next_directory_entry(|| directory.next_entry(), |entry| &entry.name)?
            else {
                r.length = 0;
                return Ok(0);
            };
            unsafe {
                core::ptr::copy_nonoverlapping(entry.name.as_ptr(), r.data, entry.name.len())
            };
            r.length = entry.name.len();
            r.mode = if entry.is_directory {
                0o040000
            } else {
                0o100000
            };
            r.size = entry.size;
            Ok(1)
        }
        op::CLOSE_DIR => {
            unsafe { directory(r.handle)? };
            drop(unsafe { Box::from_raw(r.handle.cast::<FsDirectory>()) });
            Ok(0)
        }
        op::OPEN => {
            let flags = abi::open_flags(r.flags)?;
            let path = unsafe { path(r)? };
            let file = open_fs_file(&path, flags)?;
            Ok(Box::into_raw(Box::new(file)) as usize as i64)
        }
        op::CLOSE => {
            unsafe { file(r.handle)? };
            let owned = unsafe { Box::from_raw(r.handle.cast::<FsFile>()) };
            (*owned).close()?;
            Ok(0)
        }
        op::READ => {
            unsafe { bytes(r)? };
            let data = if r.length == 0 {
                core::ptr::NonNull::<u8>::dangling().as_ptr()
            } else {
                r.data
            };
            let buffer = unsafe { core::slice::from_raw_parts_mut(data, r.length) };
            let count = unsafe { file(r.handle)? }.read(buffer)?;
            if count > r.length {
                return Err(EIO);
            }
            Ok(count as i64)
        }
        op::WRITE => {
            let input = unsafe { bytes(r)? };
            let count = unsafe { file(r.handle)? }.write(input)?;
            if count > r.length {
                return Err(EIO);
            }
            Ok(count as i64)
        }
        op::SEEK => {
            let file = unsafe { file(r.handle)? };
            let position = if matches!(file, FsFile::Fat(_)) {
                u64::from(abi::seek_position(
                    u32::try_from(file.position()).map_err(|_| EFBIG)?,
                    u32::try_from(file.len()?).map_err(|_| EFBIG)?,
                    r.offset,
                    r.flags,
                )?)
            } else {
                let base = match r.flags {
                    0 => 0,
                    1 => file.position(),
                    2 => file.len()?,
                    _ => return Err(EINVAL),
                };
                let target = i128::from(base) + i128::from(r.offset);
                if !(0..=i128::from(i64::MAX)).contains(&target) {
                    return Err(EINVAL);
                }
                target as u64
            };
            file.seek(position)?;
            i64::try_from(position).map_err(|_| EFBIG)
        }
        op::SYNC => {
            unsafe { file(r.handle)? }.flush()?;
            Ok(0)
        }
        op::SET_LEN => {
            unsafe { file(r.handle)? }.truncate(r.size)?;
            Ok(0)
        }
        op::FILE_METADATA => {
            let file = unsafe { file(r.handle)? };
            let (size, readonly) = match file {
                FsFile::Fat(file) => {
                    let (size, readonly) = file.metadata().map_err(fat_errno)?;
                    return_times(r, file.timestamps().map_err(fat_errno)?);
                    (u64::from(size), readonly)
                }
                FsFile::Exfat(file) => (
                    file.len().map_err(|error| ogc_blockfs::errno(&error))?,
                    true,
                ),
            };
            r.size = size;
            r.mode = 0o100000 | if readonly { 0o444 } else { 0o666 };
            Ok(0)
        }
        op::PATH_METADATA => {
            let path = unsafe { path(r)? };
            if let Some(relative) = exfat_path(&path)? {
                let info =
                    ogc_blockfs::stat(relative).map_err(|error| ogc_blockfs::errno(&error))?;
                r.size = info.size;
                r.mode = (if info.is_dir { 0o040000 } else { 0o100000 }) | 0o444;
                return Ok(0);
            }
            let info = ogc_fat::stat(&path).map_err(fat_errno)?;
            return_times(r, ogc_fat::get_times(&path).map_err(fat_errno)?);
            r.size = u64::from(info.size);
            r.mode = (if info.is_directory {
                0o040000
            } else {
                0o100000
            }) | if info.readonly { 0o444 } else { 0o666 };
            Ok(0)
        }
        op::UNLINK => {
            let path = unsafe { path(r)? };
            reject_exfat_mutation(&path)?;
            ogc_fat::remove_file(&path).map_err(fat_errno)?;
            Ok(0)
        }
        op::MKDIR => {
            let path = unsafe { path(r)? };
            reject_exfat_mutation(&path)?;
            ogc_fat::create_directory(&path).map_err(fat_errno)?;
            Ok(0)
        }
        op::RMDIR => {
            let path = unsafe { path(r)? };
            reject_exfat_mutation(&path)?;
            ogc_fat::remove_directory(&path).map_err(fat_errno)?;
            Ok(0)
        }
        _ => Err(ENOTSUP),
    }
}

/// # Safety
/// The request must be an aligned unique FsRequest. Each handle must come from
/// OPEN (file) or OPEN_DIR (directory) and remain live until the matching CLOSE
/// or CLOSE_DIR. Handles must only be used with operations for their type.
/// All handle operations must
/// be serialized, including release. Data must have the required read/write
/// extent (including second_path for COPY/RENAME). CANONICALIZE uses second_path
/// as writable output of second_length bytes. No buffer may overlap the request,
/// another input/output buffer, or the handle allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_fs(request: *mut FsRequest) -> i64 {
    if request.is_null() || !request.is_aligned() {
        return -i64::from(EINVAL);
    }
    match unsafe { dispatch(&mut *request) } {
        Ok(value) => value,
        Err(errno) => -i64::from(errno),
    }
}
