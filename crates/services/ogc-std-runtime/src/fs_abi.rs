//! Shared std/provider file ABI and input rules. Errors are positive Newlib errno.
use core::ffi::c_void;

pub const READ: u32 = 1;
pub const WRITE: u32 = 2;
pub const APPEND: u32 = 4;
pub const TRUNCATE: u32 = 8;
pub const CREATE: u32 = 16;
pub const CREATE_NEW: u32 = 32;

/// Operation values shared by std and the application provider.
pub mod op {
    pub const OPEN: u32 = 1;
    pub const CLOSE: u32 = 2;
    pub const READ: u32 = 3;
    pub const WRITE: u32 = 4;
    pub const SEEK: u32 = 5;
    pub const SYNC: u32 = 6;
    pub const SET_LEN: u32 = 7;
    pub const FILE_METADATA: u32 = 8;
    pub const PATH_METADATA: u32 = 9;
    pub const UNLINK: u32 = 10;
    pub const MKDIR: u32 = 11;
    pub const RMDIR: u32 = 12;
    pub const OPEN_DIR: u32 = 13;
    pub const READ_DIR: u32 = 14;
    pub const CLOSE_DIR: u32 = 15;
    pub const COPY: u32 = 16;
    pub const RENAME: u32 = 17;
    pub const SET_PERMISSIONS: u32 = 18;
    pub const FILE_SET_PERMISSIONS: u32 = 19;
    pub const CANONICALIZE: u32 = 20;
    pub const SET_TIMES: u32 = 21;
    pub const FILE_SET_TIMES: u32 = 22;
}

/// Maximum filename byte count accepted by the production FAT service.
pub const DIR_NAME_MAX: usize = 255;

/// Check a returned directory name. Dot entries are excluded from std traversal.
pub fn directory_name_visible(name: &[u8]) -> Result<bool, i32> {
    if name.is_empty()
        || name.len() > DIR_NAME_MAX
        || name.iter().any(|b| matches!(b, 0 | b'/' | b':'))
    {
        return Err(5);
    }
    Ok(name != b"." && name != b"..")
}

/// Read through dot entries. Preserve transport errors and validate each name.
pub fn next_directory_entry<T>(
    mut read: impl FnMut() -> Result<Option<T>, i32>,
    name: impl Fn(&T) -> &[u8],
) -> Result<Option<T>, i32> {
    loop {
        let Some(entry) = read()? else {
            return Ok(None);
        };
        if directory_name_visible(name(&entry))? {
            return Ok(Some(entry));
        }
    }
}

/// Copy bytes through the production read/write boundaries. A zero write is EIO.
pub fn copy_bytes(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, i32>,
    mut write: impl FnMut(&[u8]) -> Result<usize, i32>,
) -> Result<u64, i32> {
    let mut buffer = [0u8; 4096];
    let mut total = 0u64;
    loop {
        let count = read(&mut buffer)?;
        if count > buffer.len() {
            return Err(5);
        }
        if count == 0 {
            return Ok(total);
        }
        let mut offset = 0;
        while offset < count {
            let written = write(&buffer[offset..count])?;
            if written == 0 || written > count - offset {
                return Err(5);
            }
            offset += written;
            total = total.checked_add(written as u64).ok_or(139)?;
        }
    }
}

/// Both closes must run, including after transfer failure. Return the first error.
/// FAT records each failed close separately in its cleanup diagnostics.
pub fn finish_copy(
    result: Result<u64, i32>,
    destination_close: impl FnOnce() -> Result<(), i32>,
    source_close: impl FnOnce() -> Result<(), i32>,
) -> Result<u64, i32> {
    let destination = destination_close();
    let source = source_close();
    let count = result.as_ref().copied().unwrap_or(0);
    result.and(destination.and(source).map(|()| count))
}

pub fn open_flags(options: u32) -> Result<i32, i32> {
    if options & !63 != 0 || options & (READ | WRITE | APPEND) == 0 {
        return Err(22);
    }
    let writable = options & (WRITE | APPEND) != 0;
    if !writable && options & (TRUNCATE | CREATE | CREATE_NEW) != 0 {
        return Err(22);
    }
    if options & APPEND != 0 && options & TRUNCATE != 0 && options & CREATE_NEW == 0 {
        return Err(22);
    }
    let mut flags = match (options & READ != 0, writable) {
        (true, false) => 0,
        (false, true) => 1,
        (true, true) => 2,
        (false, false) => return Err(22),
    };
    if options & APPEND != 0 {
        flags |= 0x0008;
    }
    if options & CREATE_NEW != 0 {
        flags |= 0x0200 | 0x0800;
    } else {
        if options & CREATE != 0 {
            flags |= 0x0200;
        }
        if options & TRUNCATE != 0 {
            flags |= 0x0400;
        }
    }
    Ok(flags)
}

/// Return the mount-name extent. Mount names use ASCII letters, digits, '_' and '-'.
pub fn mount_prefix(path: &[u8]) -> Option<usize> {
    let end = path.iter().position(|&byte| byte == b':')?;
    if end == 0
        || !path[..end]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return None;
    }
    Some(end)
}

/// Relative and mount-relative paths need the future current-directory service.
pub fn validate_path(path: &[u8]) -> Result<(), i32> {
    if path.is_empty() || path.contains(&0) {
        return Err(22);
    }
    let Some(end) = mount_prefix(path) else {
        return Err(if path.contains(&b':') { 22 } else { 134 });
    };
    if path.get(end + 1) != Some(&b'/') {
        return Err(134);
    }
    if path[end + 1..].contains(&b':') {
        return Err(22);
    }
    Ok(())
}

pub fn seek_position(current: u32, size: u32, offset: i64, origin: u32) -> Result<u32, i32> {
    let base = match origin {
        0 => 0,
        1 => i64::from(current),
        2 => i64::from(size),
        _ => return Err(22),
    };
    let value = base.checked_add(offset).ok_or(139)?;
    if value < 0 {
        return Err(22);
    }
    u32::try_from(value).map_err(|_| 27)
}

#[repr(C)]
pub struct FsRequest {
    pub operation: u32,
    pub flags: u32,
    pub handle: *mut c_void,
    pub data: *mut u8,
    pub length: usize,
    pub second_path: *const u8,
    pub second_length: usize,
    pub offset: i64,
    pub size: u64,
    pub mode: u32,
    /// Access, modification, creation in Unix seconds; -1 means absent.
    pub times: [i64; 3],
    pub created_nanos: u32,
}
impl FsRequest {
    pub fn new(operation: u32, handle: *mut c_void) -> Self {
        Self {
            operation,
            flags: 0,
            handle,
            data: core::ptr::null_mut(),
            length: 0,
            second_path: core::ptr::null(),
            second_length: 0,
            offset: 0,
            size: 0,
            mode: 0,
            times: [-1; 3],
            created_nanos: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_preserves_bytes_with_short_reads_and_writes() {
        let input = b"independent copy bytes";
        let mut offset = 0;
        let mut output = Vec::new();
        let count = copy_bytes(
            |buffer| {
                let count = (input.len() - offset).min(3);
                buffer[..count].copy_from_slice(&input[offset..offset + count]);
                offset += count;
                Ok(count)
            },
            |buffer| {
                let count = buffer.len().min(2);
                output.extend_from_slice(&buffer[..count]);
                Ok(count)
            },
        );
        assert_eq!(count, Ok(22));
        assert_eq!(output, b"independent copy bytes");
        assert_eq!(copy_bytes(|_| Ok(0), |_| panic!("write after EOF")), Ok(0));
    }
    #[test]
    fn copy_reports_transfer_errors_and_invalid_counts() {
        assert_eq!(
            copy_bytes(|_| Err(19), |_| panic!("write after read error")),
            Err(19)
        );
        assert_eq!(
            copy_bytes(|_| Ok(4097), |_| panic!("write after invalid read")),
            Err(5)
        );
        for written in [0, 2] {
            assert_eq!(
                copy_bytes(
                    |buffer| {
                        buffer[0] = 7;
                        Ok(1)
                    },
                    |_| Ok(written)
                ),
                Err(5)
            );
        }
        let mut output = Vec::new();
        let mut reads = 0;
        assert_eq!(
            copy_bytes(
                |buffer| {
                    reads += 1;
                    if reads == 2 {
                        return Err(19);
                    }
                    buffer[..4].copy_from_slice(b"data");
                    Ok(4)
                },
                |buffer| {
                    output.extend_from_slice(buffer);
                    Ok(buffer.len())
                }
            ),
            Err(19)
        );
        assert_eq!(output, b"data");
        let mut writes = 0;
        assert_eq!(
            copy_bytes(
                |buffer| {
                    buffer[..4].copy_from_slice(b"data");
                    Ok(4)
                },
                |_| {
                    writes += 1;
                    if writes == 1 { Ok(2) } else { Err(28) }
                }
            ),
            Err(28)
        );
        assert_eq!(writes, 2);
    }
    #[test]
    fn copy_requires_both_closes_and_preserves_error_priority() {
        use std::cell::RefCell;
        for (transfer, destination, source, expected) in [
            (Ok(22), Ok(()), Ok(()), Ok(22)),
            (Ok(22), Err(5), Ok(()), Err(5)),
            (Ok(22), Ok(()), Err(19), Err(19)),
            (Ok(22), Err(5), Err(19), Err(5)),
            (Err(28), Err(5), Err(19), Err(28)),
        ] {
            let order = RefCell::new(Vec::new());
            assert_eq!(
                finish_copy(
                    transfer,
                    || {
                        order.borrow_mut().push("destination");
                        destination
                    },
                    || {
                        order.borrow_mut().push("source");
                        source
                    }
                ),
                expected
            );
            assert_eq!(*order.borrow(), ["destination", "source"]);
        }
    }
    #[test]
    fn directory_reads_skip_dots_and_preserve_end_and_failures() {
        let mut input = [
            Ok(Some(b".".as_slice())),
            Ok(Some(b"..".as_slice())),
            Ok(Some(b"entry-\xff".as_slice())),
            Err(19),
            Ok(None),
        ]
        .into_iter();
        let mut read = || input.next().expect("unexpected extra read");
        assert_eq!(
            next_directory_entry(&mut read, |name| *name),
            Ok(Some(b"entry-\xff".as_slice()))
        );
        assert_eq!(next_directory_entry(&mut read, |name| *name), Err(19));
        assert_eq!(next_directory_entry(&mut read, |name| *name), Ok(None));
        assert_eq!(
            next_directory_entry(|| Ok(Some(b"bad/name".as_slice())), |name| *name),
            Err(5)
        );
    }
    #[test]
    fn directory_names_preserve_bytes_and_reject_invalid_entries() {
        assert_eq!(directory_name_visible(b"."), Ok(false));
        assert_eq!(directory_name_visible(b".."), Ok(false));
        assert_eq!(directory_name_visible(b".hidden"), Ok(true));
        assert_eq!(directory_name_visible(b"name-\xff"), Ok(true));
        assert_eq!(directory_name_visible(&[b'x'; 255]), Ok(true));
        for name in [
            b"".as_slice(),
            b"bad/name",
            b"bad:name",
            b"bad\0name",
            &[b'x'; 256],
        ] {
            assert_eq!(directory_name_visible(name), Err(5));
        }
    }
    #[test]
    fn open_options_exact_flags_and_invalid_combinations() {
        // Fixed independent Newlib expectations for option bit patterns 0..63.
        // None means InvalidInput. The pinned std contract rejects append with
        // truncate unless create_new overrides truncation.
        let expected = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(9),
            Some(10),
            Some(9),
            Some(10),
            None,
            None,
            Some(0x401),
            Some(0x402),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(0x201),
            Some(0x202),
            Some(0x209),
            Some(0x20a),
            Some(0x209),
            Some(0x20a),
            None,
            None,
            Some(0x601),
            Some(0x602),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(0xa01),
            Some(0xa02),
            Some(0xa09),
            Some(0xa0a),
            Some(0xa09),
            Some(0xa0a),
            None,
            None,
            Some(0xa01),
            Some(0xa02),
            Some(0xa09),
            Some(0xa0a),
            Some(0xa09),
            Some(0xa0a),
            None,
            None,
            Some(0xa01),
            Some(0xa02),
            Some(0xa09),
            Some(0xa0a),
            Some(0xa09),
            Some(0xa0a),
            None,
            None,
            Some(0xa01),
            Some(0xa02),
            Some(0xa09),
            Some(0xa0a),
            Some(0xa09),
            Some(0xa0a),
        ];
        for (bits, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                open_flags(bits as u32),
                expected.ok_or(22),
                "options {bits}"
            );
        }
        assert_eq!(open_flags(66), Err(22));
        assert_eq!(open_flags(WRITE | CREATE_NEW | TRUNCATE), Ok(0x0a01));
    }
    #[test]
    fn mounted_paths_and_bytes() {
        assert_eq!(mount_prefix(b"carda:/apps"), Some(5));
        assert_eq!(mount_prefix(b"bad mount:/apps"), None);
        assert_eq!(validate_path(b"sd:/apps/../data"), Ok(()));
        assert_eq!(validate_path(b"usb:/non-utf8-\xff"), Ok(()));
        for path in [b"relative".as_slice(), b"/root", b"sd:relative", b"sd:"] {
            assert_eq!(validate_path(path), Err(134));
        }
        for path in [
            b"".as_slice(),
            b"sd:/nul\0tail",
            b":/x",
            b"s d:/x",
            b"sd:/other:x",
        ] {
            assert_eq!(validate_path(path), Err(22));
        }
    }
    #[test]
    fn seek_width_and_signed_bounds() {
        assert_eq!(seek_position(3, 10, -2, 1), Ok(1));
        assert_eq!(seek_position(3, 10, -2, 2), Ok(8));
        assert_eq!(seek_position(3, 10, -11, 2), Err(22));
        assert_eq!(seek_position(0, 0, i64::from(u32::MAX), 0), Ok(u32::MAX));
        assert_eq!(seek_position(u32::MAX, 0, 1, 1), Err(27));
        assert_eq!(seek_position(1, 0, i64::MAX, 1), Err(139));
        assert_eq!(seek_position(0, 0, 0, 3), Err(22));
    }
}
