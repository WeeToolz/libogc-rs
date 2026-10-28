//! Typed FAT provider. No raw devoptab registration pointer escapes a lookup.
use alloc::boxed::Box;
use alloc::ffi::CString;
use core::ffi::c_void;
use ogc_fat::{FatDirectory, FatFile, fat_errno};
use ogc_sysbase::{EBADF, EFBIG, EINVAL, EIO, ENOTSUP};
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
unsafe fn file<'a>(handle: *mut c_void) -> Result<&'a mut FatFile, i32> {
    if handle.is_null() || !(handle as usize).is_multiple_of(core::mem::align_of::<FatFile>()) {
        return Err(EBADF);
    }
    Ok(unsafe { &mut *handle.cast::<FatFile>() })
}
unsafe fn directory<'a>(handle: *mut c_void) -> Result<&'a mut FatDirectory, i32> {
    if handle.is_null() || !handle.cast::<FatDirectory>().is_aligned() {
        return Err(EBADF);
    }
    Ok(unsafe { &mut *handle.cast::<FatDirectory>() })
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
            ogc_fat::set_times(&unsafe { path(r)? }, accessed, modified).map_err(fat_errno)?;
            Ok(0)
        }
        op::FILE_SET_TIMES => {
            let (accessed, modified) = requested_times(r)?;
            unsafe { file(r.handle)? }
                .set_times(accessed, modified)
                .map_err(fat_errno)?;
            Ok(0)
        }
        op::CANONICALIZE => {
            let resolved = ogc_fat::canonicalize(&unsafe { path(r)? }).map_err(fat_errno)?;
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
            ogc_fat::rename_replace(&from, &to).map_err(fat_errno)?;
            Ok(0)
        }
        op::SET_PERMISSIONS => {
            if r.flags > 1 {
                return Err(EINVAL);
            }
            ogc_fat::set_readonly(&unsafe { path(r)? }, r.flags != 0).map_err(fat_errno)?;
            Ok(0)
        }
        op::FILE_SET_PERMISSIONS => {
            if r.flags > 1 {
                return Err(EINVAL);
            }
            unsafe { file(r.handle)? }
                .set_readonly(r.flags != 0)
                .map_err(fat_errno)?;
            Ok(0)
        }
        op::COPY => {
            let from = unsafe { path(r)? };
            let to = unsafe { second_path(r)? };
            copy_file(&from, &to).map(|count| count as i64)
        }
        op::OPEN_DIR => {
            let directory = FatDirectory::open(&unsafe { path(r)? }).map_err(fat_errno)?;
            Ok(Box::into_raw(Box::new(directory)) as usize as i64)
        }
        op::READ_DIR => {
            // Validate the full output extent before advancing the FAT cursor.
            unsafe { bytes(r)? };
            if r.length < abi::DIR_NAME_MAX {
                return Err(EINVAL);
            }
            let directory = unsafe { directory(r.handle)? };
            let Some(entry) = abi::next_directory_entry(
                || directory.next_entry().map_err(fat_errno),
                |entry| &entry.name,
            )?
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
            r.size = u64::from(entry.size);
            Ok(1)
        }
        op::CLOSE_DIR => {
            unsafe { directory(r.handle)? };
            // FAT directory Drop releases only a mount reference; it has no I/O.
            drop(unsafe { Box::from_raw(r.handle.cast::<FatDirectory>()) });
            Ok(0)
        }
        op::OPEN => {
            let flags = abi::open_flags(r.flags)?;
            let path = unsafe { path(r)? };
            let file = FatFile::open_with_flags(&path, flags).map_err(fat_errno)?;
            Ok(Box::into_raw(Box::new(file)) as usize as i64)
        }
        op::CLOSE => {
            unsafe { file(r.handle)? };
            let owned = unsafe { Box::from_raw(r.handle.cast::<FatFile>()) };
            (*owned).close().map_err(fat_errno)?;
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
            let count = unsafe { file(r.handle)? }.read(buffer).map_err(fat_errno)?;
            if count > r.length {
                return Err(EIO);
            }
            Ok(count as i64)
        }
        op::WRITE => {
            let input = unsafe { bytes(r)? };
            let count = unsafe { file(r.handle)? }.write(input).map_err(fat_errno)?;
            if count > r.length {
                return Err(EIO);
            }
            Ok(count as i64)
        }
        op::SEEK => {
            let file = unsafe { file(r.handle)? };
            let position = abi::seek_position(file.position(), file.len(), r.offset, r.flags)?;
            file.seek_checked(position).map_err(fat_errno)?;
            Ok(i64::from(position))
        }
        op::SYNC => {
            unsafe { file(r.handle)? }.flush().map_err(fat_errno)?;
            Ok(0)
        }
        op::SET_LEN => {
            let size = u32::try_from(r.size).map_err(|_| EFBIG)?;
            unsafe { file(r.handle)? }
                .truncate(size)
                .map_err(fat_errno)?;
            Ok(0)
        }
        op::FILE_METADATA => {
            let (size, readonly) = unsafe { file(r.handle)? }.metadata().map_err(fat_errno)?;
            return_times(
                r,
                unsafe { file(r.handle)? }.timestamps().map_err(fat_errno)?,
            );
            r.size = u64::from(size);
            r.mode = 0o100000 | if readonly { 0o444 } else { 0o666 };
            Ok(0)
        }
        op::PATH_METADATA => {
            let path = unsafe { path(r)? };
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
            ogc_fat::remove_file(&unsafe { path(r)? }).map_err(fat_errno)?;
            Ok(0)
        }
        op::MKDIR => {
            ogc_fat::create_directory(&unsafe { path(r)? }).map_err(fat_errno)?;
            Ok(0)
        }
        op::RMDIR => {
            ogc_fat::remove_directory(&unsafe { path(r)? }).map_err(fat_errno)?;
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
