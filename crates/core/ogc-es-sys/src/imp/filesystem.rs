use alloc::alloc::{Layout, alloc, dealloc};
use core::ffi::{CStr, c_char, c_void};
use core::mem::size_of;
use core::ptr::{addr_of, null_mut};

use ogc_lwp_sys as lwp;
use ogc_sysbase::{self as sysbase, Devoptab, Reent, Stat};

use super::{
    MAX_SIGNED_TMD_SIZE, close_content, get_num_ticket_views, get_stored_tmd, get_stored_tmd_size,
    get_ticket_views, get_title_id, open_content, open_title_content, read_content, seek_content,
    signature_payload, signed_blob_signature_type, signed_tmd_size_checked,
};
use crate::{SignedBlob, Tikview, Tmd, TmdContent};

const ES_READ_BUF_SIZE: usize = 65_536;
const CONTENT_SHARED: u16 = 0x8000;

static ES_DEVICE_NAME: &[u8; 3] = b"es\0";
static ES_DEVICE_PATH: &[u8; 4] = b"es:\0";

#[repr(C, align(32))]
struct AlignedTmdBuffer([u8; MAX_SIGNED_TMD_SIZE]);

#[repr(C)]
struct EsFile {
    cfd: i32,
    title_id: u64,
    content: TmdContent,
    iobuf: *mut u8,
    mutex: lwp::Mutex,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ContentSelector {
    Index(u32),
    Id(u32),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ParsedPath {
    title_id: Option<u64>,
    selector: ContentSelector,
}

struct FileGuard(lwp::Mutex);

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = lwp::mutex_unlock(self.0);
    }
}

fn set_errno(r: *mut Reent, errno: i32) {
    unsafe { sysbase::set_reent_errno(r, errno) };
}

fn lock_file(r: *mut Reent, file: &EsFile) -> Option<FileGuard> {
    if lwp::mutex_lock(file.mutex) == 0 {
        Some(FileGuard(file.mutex))
    } else {
        set_errno(r, sysbase::EIO);
        None
    }
}

fn ascii_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_hex(bytes: &[u8], max: u64) -> Result<(u64, usize), i32> {
    let mut cursor = 0usize;
    while cursor < bytes.len() && ascii_space(bytes[cursor]) {
        cursor += 1;
    }

    let negative = match bytes.get(cursor).copied() {
        Some(b'+') => {
            cursor += 1;
            false
        }
        Some(b'-') => {
            cursor += 1;
            true
        }
        _ => false,
    };

    if bytes.get(cursor).copied() == Some(b'0')
        && matches!(bytes.get(cursor + 1).copied(), Some(b'x') | Some(b'X'))
    {
        cursor += 2;
    }

    let digits_start = cursor;
    let mut value = 0u64;
    while let Some(&byte) = bytes.get(cursor) {
        let Some(digit) = hex_digit(byte) else {
            break;
        };
        let digit = u64::from(digit);
        if value > (max.saturating_sub(digit)) / 16 {
            return Err(sysbase::ERANGE);
        }
        value = value * 16 + digit;
        cursor += 1;
    }
    if cursor == digits_start {
        return Err(sysbase::EINVAL);
    }

    if negative {
        value = if max == u32::MAX as u64 {
            u64::from((0u32).wrapping_sub(value as u32))
        } else {
            (0u64).wrapping_sub(value)
        };
    }
    Ok((value, cursor))
}

fn parse_path(path: &[u8]) -> Result<ParsedPath, i32> {
    let Some(mut rest) = path.strip_prefix(b"es:") else {
        return Err(sysbase::EINVAL);
    };

    let title_id = if let Some(absolute) = rest.strip_prefix(b"/") {
        if absolute.is_empty() {
            return Err(sysbase::EINVAL);
        }
        let (title_id, consumed) = parse_hex(absolute, u64::MAX)?;
        if absolute.get(consumed) != Some(&b'/') {
            return Err(sysbase::EINVAL);
        }
        rest = &absolute[consumed + 1..];
        Some(title_id)
    } else {
        None
    };

    let (is_content_id, argument) = if let Some(content_id) = rest.strip_prefix(b"ID") {
        (true, content_id)
    } else {
        (false, rest)
    };
    let (argument, consumed) = parse_hex(argument, u32::MAX as u64)?;
    let argument_bytes = if is_content_id { &rest[2..] } else { rest };
    if consumed != argument_bytes.len() {
        return Err(sysbase::EINVAL);
    }

    Ok(ParsedPath {
        title_id,
        selector: if is_content_id {
            ContentSelector::Id(argument as u32)
        } else {
            ContentSelector::Index(argument as u32)
        },
    })
}

fn decode_path(r: *mut Reent, path: *const c_char) -> Option<(u64, TmdContent)> {
    if path.is_null() {
        set_errno(r, sysbase::EINVAL);
        return None;
    }
    let path = unsafe { CStr::from_ptr(path) };
    if !path.to_bytes().starts_with(b"es:") {
        set_errno(r, sysbase::EINVAL);
        return None;
    }

    let mut current_title = 0u64;
    if unsafe { get_title_id(&mut current_title) } < 0 {
        set_errno(r, sysbase::EIO);
        return None;
    }

    let parsed = match parse_path(path.to_bytes()) {
        Ok(parsed) => parsed,
        Err(errno) => {
            set_errno(r, errno);
            return None;
        }
    };
    let requested_title = parsed.title_id.unwrap_or(current_title);

    let mut tmd_size = 0u32;
    if unsafe { get_stored_tmd_size(requested_title, &mut tmd_size) } < 0 {
        set_errno(r, sysbase::ENOENT);
        return None;
    }
    if tmd_size as usize > MAX_SIGNED_TMD_SIZE {
        set_errno(r, sysbase::EIO);
        return None;
    }

    let mut buffer = AlignedTmdBuffer([0; MAX_SIGNED_TMD_SIZE]);
    let blob = buffer.0.as_mut_ptr().cast::<SignedBlob>();
    if unsafe { get_stored_tmd(requested_title, blob, tmd_size) } < 0 {
        set_errno(r, sysbase::EIO);
        return None;
    }
    if signed_blob_signature_type(blob).is_none() {
        set_errno(r, sysbase::EIO);
        return None;
    }
    let Some(required_tmd_size) = signed_tmd_size_checked(blob, tmd_size) else {
        set_errno(r, sysbase::EIO);
        return None;
    };
    if required_tmd_size > tmd_size {
        set_errno(r, sysbase::EIO);
        return None;
    }

    let tmd = signature_payload(blob).cast::<Tmd>();
    let count = unsafe { core::ptr::read_unaligned(addr_of!((*tmd).num_contents)) } as usize;
    let contents = unsafe { signature_payload(blob).add(size_of::<Tmd>()) }.cast::<TmdContent>();
    for index in 0..count {
        let content = unsafe { core::ptr::read_unaligned(contents.add(index)) };
        let cid = unsafe { core::ptr::read_unaligned(addr_of!(content.cid)) };
        let content_index = unsafe { core::ptr::read_unaligned(addr_of!(content.index)) };
        let matches = match parsed.selector {
            ContentSelector::Id(expected) => cid == expected,
            ContentSelector::Index(expected) => u32::from(content_index) == expected,
        };
        if matches {
            let open_title = if requested_title == current_title {
                0
            } else {
                requested_title
            };
            return Some((open_title, content));
        }
    }

    set_errno(r, sysbase::ENOENT);
    None
}

fn aligned_layout(size: usize) -> Option<Layout> {
    Layout::from_size_align(size.max(1), 32).ok()
}

fn open_other_title(title_id: u64, index: u16) -> Result<i32, i32> {
    let mut count = 0u32;
    if unsafe { get_num_ticket_views(title_id, &mut count) } < 0 || count == 0 {
        return Err(sysbase::EIO);
    }
    let bytes = size_of::<Tikview>()
        .checked_mul(count as usize)
        .ok_or(sysbase::ENOMEM)?;
    let layout = aligned_layout(bytes).ok_or(sysbase::ENOMEM)?;
    let views = unsafe { alloc(layout) }.cast::<Tikview>();
    if views.is_null() {
        return Err(sysbase::ENOMEM);
    }

    let views_rc = unsafe { get_ticket_views(title_id, views, count) };
    let result = if views_rc < 0 {
        Err(sysbase::EIO)
    } else {
        let cfd = unsafe { open_title_content(title_id, views, index) };
        if cfd < 0 { Err(sysbase::EIO) } else { Ok(cfd) }
    };
    unsafe { dealloc(views.cast::<u8>(), layout) };
    result
}

unsafe extern "C" fn es_open(
    r: *mut Reent,
    file_struct: *mut c_void,
    path: *const c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    if file_struct.is_null() {
        set_errno(r, sysbase::EINVAL);
        return -1;
    }
    let Some((title_id, content)) = decode_path(r, path) else {
        return -1;
    };
    if flags & sysbase::O_ACCMODE != sysbase::O_RDONLY {
        set_errno(r, sysbase::EROFS);
        return -1;
    }

    let content_index = unsafe { core::ptr::read_unaligned(addr_of!(content.index)) };
    let cfd = if title_id == 0 {
        let cfd = open_content(content_index);
        if cfd < 0 {
            set_errno(r, sysbase::EIO);
            return -1;
        }
        cfd
    } else {
        match open_other_title(title_id, content_index) {
            Ok(cfd) => cfd,
            Err(errno) => {
                set_errno(r, errno);
                return -1;
            }
        }
    };

    let file = unsafe { &mut *file_struct.cast::<EsFile>() };
    file.cfd = cfd;
    file.title_id = title_id;
    file.content = content;
    file.iobuf = null_mut();
    let mut mutex = lwp::LWP_MUTEX_NULL;
    if unsafe { lwp::mutex_init(&mut mutex, 0) } != 0 {
        let _ = close_content(cfd);
        set_errno(r, sysbase::EIO);
        return -1;
    }
    file.mutex = mutex;
    file_struct as usize as i32
}

unsafe extern "C" fn es_close(r: *mut Reent, file_struct: *mut c_void) -> i32 {
    if file_struct.is_null() {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<EsFile>() };
    let Some(guard) = lock_file(r, file) else {
        return -1;
    };
    if file.cfd < 0 || close_content(file.cfd) < 0 {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    file.cfd = -1;
    if !file.iobuf.is_null() {
        if let Some(layout) = aligned_layout(ES_READ_BUF_SIZE) {
            unsafe { dealloc(file.iobuf, layout) };
        }
        file.iobuf = null_mut();
    }
    let mutex = file.mutex;
    drop(guard);
    let _ = lwp::mutex_destroy(mutex);
    0
}

unsafe extern "C" fn es_read(
    r: *mut Reent,
    file_struct: *mut c_void,
    ptr: *mut c_char,
    mut len: usize,
) -> isize {
    if file_struct.is_null() {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<EsFile>() };
    let Some(_guard) = lock_file(r, file) else {
        return -1;
    };
    if file.cfd < 0 {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    if ptr.is_null() {
        set_errno(r, sysbase::EIO);
        return -1;
    }

    if (ptr as usize) & 31 == 0 {
        let rc = unsafe { read_content(file.cfd, ptr.cast::<u8>(), len as u32) };
        if rc < 0 {
            set_errno(r, sysbase::EIO);
            return -1;
        }
        return rc as isize;
    }

    if file.iobuf.is_null() {
        let Some(layout) = aligned_layout(ES_READ_BUF_SIZE) else {
            set_errno(r, sysbase::ENOMEM);
            return -1;
        };
        file.iobuf = unsafe { alloc(layout) };
        if file.iobuf.is_null() {
            set_errno(r, sysbase::ENOMEM);
            return -1;
        }
    }

    let mut output = ptr.cast::<u8>();
    let mut total = 0usize;
    while len != 0 {
        let chunk = len.min(ES_READ_BUF_SIZE);
        let rc = unsafe { read_content(file.cfd, file.iobuf, chunk as u32) };
        if rc < 0 {
            set_errno(r, sysbase::EIO);
            return -1;
        }
        let read = rc as usize;
        unsafe { core::ptr::copy_nonoverlapping(file.iobuf, output, read) };
        output = unsafe { output.add(read) };
        total += read;
        len -= read;
        if read < chunk {
            break;
        }
    }
    total as isize
}

unsafe extern "C" fn es_seek(
    r: *mut Reent,
    file_struct: *mut c_void,
    where_: i64,
    whence: i32,
) -> i64 {
    if file_struct.is_null() {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<EsFile>() };
    let Some(_guard) = lock_file(r, file) else {
        return -1;
    };
    if file.cfd < 0 {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    // C's newlib callback receives 64-bit off_t, then ES_SeekContent narrows
    // the offset to its 32-bit IOS API argument.
    let rc = seek_content(file.cfd, where_ as i32, whence);
    if rc < 0 {
        set_errno(r, sysbase::EINVAL);
        -1
    } else {
        i64::from(rc)
    }
}

fn fill_stat(title_id: u64, content: &TmdContent) -> Stat {
    let cid = unsafe { core::ptr::read_unaligned(addr_of!(content.cid)) };
    let content_type = unsafe { core::ptr::read_unaligned(addr_of!(content.type_)) };
    let size = unsafe { core::ptr::read_unaligned(addr_of!(content.size)) };
    let device = if title_id >> 32 == 1 {
        title_id as u16
    } else {
        (title_id >> 8) as u16
    };

    let mut stat = Stat {
        st_dev: u32::from(device),
        st_ino: cid,
        st_mode: sysbase::S_IFREG | sysbase::S_IRUSR,
        st_nlink: 1,
        st_uid: device,
        st_gid: 0,
        st_size: size as i64,
        st_blksize: 16_384,
        st_blocks: (size.wrapping_add(511) / 512) as i32,
        ..Stat::default()
    };
    if content_type & CONTENT_SHARED != 0 {
        stat.st_mode |= sysbase::S_IRGRP;
    }
    stat
}

unsafe extern "C" fn es_fstat(r: *mut Reent, file_struct: *mut c_void, st: *mut c_void) -> i32 {
    if file_struct.is_null() || st.is_null() {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<EsFile>() };
    let Some(_guard) = lock_file(r, file) else {
        return -1;
    };
    if file.cfd < 0 {
        set_errno(r, sysbase::EBADF);
        return -1;
    }
    unsafe {
        st.cast::<Stat>()
            .write(fill_stat(file.title_id, &file.content))
    };
    0
}

unsafe extern "C" fn es_stat(r: *mut Reent, path: *const c_char, st: *mut c_void) -> i32 {
    if st.is_null() {
        set_errno(r, sysbase::EINVAL);
        return -1;
    }
    let Some((title_id, content)) = decode_path(r, path) else {
        return -1;
    };
    unsafe { st.cast::<Stat>().write(fill_stat(title_id, &content)) };
    0
}

struct StaticDevoptab(Devoptab);

// SAFETY: this devoptab and every pointer reachable from it are immutable for
// the program lifetime. Mutable per-file state lives in devoptab file structs.
unsafe impl Sync for StaticDevoptab {}

static ES_DEVOPTAB: StaticDevoptab = StaticDevoptab(Devoptab {
    name: ES_DEVICE_NAME.as_ptr() as *const c_char,
    struct_size: size_of::<EsFile>(),
    open_r: Some(es_open),
    close_r: Some(es_close),
    write_r: None,
    read_r: Some(es_read),
    seek_r: Some(es_seek),
    fstat_r: Some(es_fstat),
    stat_r: Some(es_stat),
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
});

pub(super) fn init_fs() -> i32 {
    // C calls AddDevice unconditionally. The registry replaces an existing
    // entry with the same bare device name, so preserve that behavior here.
    let result = unsafe { sysbase::add_device(&ES_DEVOPTAB.0) };
    if result < 0 {
        return result;
    }
    0
}

pub(super) fn deinit_fs() -> i32 {
    let path = unsafe { CStr::from_bytes_with_nul_unchecked(ES_DEVICE_PATH) };
    if sysbase::find_device(path) < 0 {
        return 0;
    }
    let result = sysbase::remove_device(path);
    if result < 0 { result } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    static EXISTING_ES_NAME: &[u8] = b"es\0";
    static EXISTING_ES_DEVOPTAB: StaticDevoptab = StaticDevoptab(Devoptab {
        name: EXISTING_ES_NAME.as_ptr().cast(),
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
    });

    #[test]
    fn rust_devoptab_uses_bare_name_and_es_callbacks() {
        assert_eq!(
            unsafe { CStr::from_ptr(ES_DEVOPTAB.0.name) }.to_bytes(),
            b"es"
        );
        assert_eq!(
            CStr::from_bytes_with_nul(ES_DEVICE_PATH)
                .expect("ES device path must be NUL terminated")
                .to_bytes(),
            b"es:"
        );
        assert!(ES_DEVOPTAB.0.open_r.is_some());
        assert!(ES_DEVOPTAB.0.close_r.is_some());
        assert!(ES_DEVOPTAB.0.read_r.is_some());
        assert!(ES_DEVOPTAB.0.seek_r.is_some());
        assert!(ES_DEVOPTAB.0.stat_r.is_some());
    }

    #[test]
    fn init_replaces_an_existing_es_registration() {
        let path = unsafe { CStr::from_bytes_with_nul_unchecked(ES_DEVICE_PATH) };
        let index = unsafe { sysbase::add_device(&EXISTING_ES_DEVOPTAB.0) };
        assert!(index >= 0);
        assert!(core::ptr::eq(
            sysbase::device_op_tab(path).expect("existing es entry should be visible"),
            &EXISTING_ES_DEVOPTAB.0
        ));

        assert_eq!(init_fs(), 0);
        assert_eq!(sysbase::find_device(path), index);
        assert!(core::ptr::eq(
            sysbase::device_op_tab(path).expect("replacement es entry should be visible"),
            &ES_DEVOPTAB.0
        ));

        assert_eq!(deinit_fs(), 0);
        assert_eq!(sysbase::find_device(path), -1);
    }

    #[test]
    fn devoptab_seek_and_stat_match_devkitppc_abi_widths() {
        let _: sysbase::SeekFn = es_seek;
        assert_eq!(core::mem::size_of::<Stat>(), 96);
        assert_eq!(core::mem::offset_of!(Stat, st_ino), 4);
        assert_eq!(core::mem::size_of::<u32>(), 4);
    }

    #[test]
    fn parses_all_documented_es_path_forms() {
        assert_eq!(
            parse_path(b"es:00000004"),
            Ok(ParsedPath {
                title_id: None,
                selector: ContentSelector::Index(4),
            })
        );
        assert_eq!(
            parse_path(b"es:ID00000033"),
            Ok(ParsedPath {
                title_id: None,
                selector: ContentSelector::Id(0x33),
            })
        );
        assert_eq!(
            parse_path(b"es:/0000000100000002/00000004"),
            Ok(ParsedPath {
                title_id: Some(0x0000000100000002),
                selector: ContentSelector::Index(4),
            })
        );
        assert_eq!(
            parse_path(b"es:/0x100000002/ID0x33"),
            Ok(ParsedPath {
                title_id: Some(0x0000000100000002),
                selector: ContentSelector::Id(0x33),
            })
        );
    }

    #[test]
    fn rejects_malformed_paths_and_overflow() {
        assert_eq!(parse_path(b"sd:4"), Err(sysbase::EINVAL));
        assert_eq!(parse_path(b"es:"), Err(sysbase::EINVAL));
        assert_eq!(parse_path(b"es:/1"), Err(sysbase::EINVAL));
        assert_eq!(parse_path(b"es:/1/4/extra"), Err(sysbase::EINVAL));
        assert_eq!(parse_path(b"es:100000000"), Err(sysbase::ERANGE));
    }

    #[test]
    fn stat_matches_c_content_metadata_rules() {
        let content = TmdContent {
            cid: 0x1234_5678,
            index: 4,
            type_: CONTENT_SHARED,
            size: 1025,
            hash: [0; 20],
        };
        let stat = fill_stat(0x0000_0001_0000_0002, &content);
        assert_eq!(stat.st_ino, 0x1234_5678);
        assert_eq!(stat.st_dev, 2);
        assert_eq!(stat.st_uid, 2);
        assert_eq!(
            stat.st_mode,
            sysbase::S_IFREG | sysbase::S_IRUSR | sysbase::S_IRGRP
        );
        assert_eq!(stat.st_size, 1025);
        assert_eq!(stat.st_blocks, 3);
        assert_eq!(stat.st_blksize, 16_384);
    }
}
