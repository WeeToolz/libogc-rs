#![no_std]
#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_char, c_void};
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

pub use ogc_sys::Bool;

pub const SMB_MAXPATH: usize = 4096;

pub const SMB_SUCCESS: i32 = 0;
pub const SMB_ERROR: i32 = -1;
pub const SMB_BAD_PROTOCOL: i32 = -2;
pub const SMB_BAD_COMMAND: i32 = -3;
pub const SMB_PROTO_FAIL: i32 = -4;
pub const SMB_NOT_USER: i32 = -5;
pub const SMB_BAD_KEYLEN: i32 = -6;
pub const SMB_BAD_DATALEN: i32 = -7;
pub const SMB_BAD_LOGINDATA: i32 = -8;

pub const SMB_OF_OPEN: u16 = 1;
pub const SMB_OF_TRUNCATE: u16 = 2;
pub const SMB_OF_CREATE: u16 = 16;

pub const SMB_SRCH_READONLY: u16 = 1;
pub const SMB_SRCH_HIDDEN: u16 = 2;
pub const SMB_SRCH_SYSTEM: u16 = 4;
pub const SMB_SRCH_VOLUME: u16 = 8;
pub const SMB_SRCH_DIRECTORY: u16 = 16;
pub const SMB_SRCH_ARCHIVE: u16 = 32;

pub const SMB_OPEN_READING: u16 = 0;
pub const SMB_OPEN_WRITING: u16 = 1;
pub const SMB_OPEN_READWRITE: u16 = 2;
pub const SMB_OPEN_COMPATIBLE: u16 = 0;
pub const SMB_DENY_READWRITE: u16 = 0x10;
pub const SMB_DENY_WRITE: u16 = 0x20;
pub const SMB_DENY_READ: u16 = 0x30;
pub const SMB_DENY_NONE: u16 = 0x40;

pub type SmbConn = u32;
pub type SmbFile = *mut c_void;

mod ntlm;
#[cfg(target_arch = "powerpc")]
mod powerpc_backend;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SmbDirEntry {
    pub size: u64,
    pub ctime: u64,
    pub atime: u64,
    pub mtime: u64,
    pub attributes: u32,
    pub sid: u16,
    pub name: [c_char; 768],
}

impl Default for SmbDirEntry {
    fn default() -> Self {
        Self {
            size: 0,
            ctime: 0,
            atime: 0,
            mtime: 0,
            attributes: 0,
            sid: 0,
            name: [0; 768],
        }
    }
}

static SMB_INITED: AtomicBool = AtomicBool::new(false);
static SMB_SEARCH_FLAGS: AtomicU16 = AtomicU16::new(0);
static SMB_NEXT_CONN: AtomicU32 = AtomicU32::new(1);
static SMB_LAST_CONN: AtomicU32 = AtomicU32::new(0);
static SMB_NEXT_FILE: AtomicU32 = AtomicU32::new(1);
static SMB_SEARCH_STEP: AtomicU32 = AtomicU32::new(0);
static SMB_SEARCH_CONN: AtomicU32 = AtomicU32::new(0);
static SMB_DEVICE_MOUNTED: AtomicBool = AtomicBool::new(false);
static mut SMB_SEARCH_PREFIX: [u8; PATH_NAME_BYTES] = [0; PATH_NAME_BYTES];
static FILE_SLOT_USED: [AtomicBool; 8] = [const { AtomicBool::new(false) }; 8];
static FILE_SLOT_HANDLE: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static FILE_SLOT_CONN: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static FILE_SLOT_PATH: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static FILE_SLOT_ACCESS: [AtomicU16; 8] = [const { AtomicU16::new(0) }; 8];

const PATH_SLOTS: usize = 16;
const PATH_NAME_BYTES: usize = 128;
const PATH_CONTENT_BYTES: usize = 256;

static PATH_SLOT_USED: [AtomicBool; PATH_SLOTS] = [const { AtomicBool::new(false) }; PATH_SLOTS];
static PATH_SLOT_IS_DIR: [AtomicBool; PATH_SLOTS] = [const { AtomicBool::new(false) }; PATH_SLOTS];
static PATH_SLOT_SIZE: [AtomicU32; PATH_SLOTS] = [const { AtomicU32::new(0) }; PATH_SLOTS];
static PATH_SLOT_ATTRS: [AtomicU32; PATH_SLOTS] = [const { AtomicU32::new(0) }; PATH_SLOTS];
static mut DEVICE_NAME: [u8; 32] = [0; 32];
static mut PATH_SLOT_NAME: [[u8; PATH_NAME_BYTES]; PATH_SLOTS] = [[0; PATH_NAME_BYTES]; PATH_SLOTS];
static mut PATH_SLOT_CONTENT: [[u8; PATH_CONTENT_BYTES]; PATH_SLOTS] =
    [[0; PATH_CONTENT_BYTES]; PATH_SLOTS];

#[inline]
const fn smb_host_compatibility_enabled() -> bool {
    !cfg!(target_arch = "powerpc")
}

#[inline(never)]
fn smb_powerpc_backend_enabled() -> bool {
    cfg!(target_arch = "powerpc")
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct HostStatVfs {
    f_bsize: u32,
    f_frsize: u32,
    f_blocks: u32,
    f_bfree: u32,
    f_bavail: u32,
    f_files: u32,
    f_ffree: u32,
    f_favail: u32,
    f_fsid: u32,
    f_flag: u32,
    f_namemax: u32,
}

fn clear_path_slots() {
    for idx in 0..PATH_SLOTS {
        PATH_SLOT_USED[idx].store(false, Ordering::Release);
        PATH_SLOT_IS_DIR[idx].store(false, Ordering::Release);
        PATH_SLOT_SIZE[idx].store(0, Ordering::Release);
        PATH_SLOT_ATTRS[idx].store(0, Ordering::Release);
        unsafe {
            PATH_SLOT_NAME[idx] = [0; PATH_NAME_BYTES];
            PATH_SLOT_CONTENT[idx] = [0; PATH_CONTENT_BYTES];
        }
    }
    unsafe { SMB_SEARCH_PREFIX = [0; PATH_NAME_BYTES] };
}

fn path_matches(slot: usize, path: &[u8]) -> bool {
    unsafe {
        let stored = &PATH_SLOT_NAME[slot];
        let end = stored
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(PATH_NAME_BYTES);
        &stored[..end] == path
    }
}

fn direct_parent_matches(path: &[u8], parent: &[u8]) -> bool {
    if path == parent {
        return false;
    }
    if parent == b"/" {
        return path.starts_with(b"/")
            && path[1..].iter().filter(|byte| **byte == b'/').count() == 0;
    }
    if !path.starts_with(parent) || path.get(parent.len()) != Some(&b'/') {
        return false;
    }
    !path[parent.len() + 1..].contains(&b'/')
}

fn slot_name_bytes(slot: usize) -> &'static [u8] {
    unsafe {
        let end = PATH_SLOT_NAME[slot]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(PATH_NAME_BYTES);
        &PATH_SLOT_NAME[slot][..end]
    }
}

fn find_path(path: &[u8]) -> Option<usize> {
    (0..PATH_SLOTS)
        .find(|idx| PATH_SLOT_USED[*idx].load(Ordering::Acquire) && path_matches(*idx, path))
}

fn parent_path(path: &[u8]) -> Option<&[u8]> {
    if path.len() <= 1 || path[0] != b'/' {
        return None;
    }
    path.iter()
        .rposition(|byte| *byte == b'/')
        .map(|idx| if idx == 0 { &path[..1] } else { &path[..idx] })
}

fn parent_exists(path: &[u8]) -> bool {
    let Some(parent) = parent_path(path) else {
        return false;
    };
    find_path(parent)
        .map(|idx| PATH_SLOT_IS_DIR[idx].load(Ordering::Acquire))
        .unwrap_or(false)
}

fn has_children(path_idx: usize) -> bool {
    let parent = slot_name_bytes(path_idx);
    (0..PATH_SLOTS).any(|idx| {
        idx != path_idx
            && PATH_SLOT_USED[idx].load(Ordering::Acquire)
            && direct_parent_matches(slot_name_bytes(idx), parent)
    })
}

fn alloc_path(path: &[u8], is_dir: bool, attrs: u32) -> Option<usize> {
    for idx in 0..PATH_SLOTS {
        if !PATH_SLOT_USED[idx].swap(true, Ordering::AcqRel) {
            PATH_SLOT_IS_DIR[idx].store(is_dir, Ordering::Release);
            PATH_SLOT_SIZE[idx].store(0, Ordering::Release);
            PATH_SLOT_ATTRS[idx].store(attrs, Ordering::Release);
            unsafe {
                PATH_SLOT_NAME[idx] = [0; PATH_NAME_BYTES];
                let len = path.len().min(PATH_NAME_BYTES.saturating_sub(1));
                PATH_SLOT_NAME[idx][..len].copy_from_slice(&path[..len]);
                PATH_SLOT_CONTENT[idx] = [0; PATH_CONTENT_BYTES];
            }
            return Some(idx);
        }
    }
    None
}

fn ensure_default_share() {
    if find_path(b"/").is_none() {
        let _ = alloc_path(b"/", true, SMB_SRCH_DIRECTORY as u32);
    }
    if find_path(b"/docs").is_none() {
        let _ = alloc_path(b"/docs", true, SMB_SRCH_DIRECTORY as u32);
    }
    if let Some(idx) =
        find_path(b"/boot.dol").or_else(|| alloc_path(b"/boot.dol", false, SMB_SRCH_ARCHIVE as u32))
    {
        PATH_SLOT_SIZE[idx].store(8, Ordering::Release);
        unsafe {
            PATH_SLOT_CONTENT[idx][..8].copy_from_slice(b"BOOTDOL!");
        }
    }
    if let Some(idx) = find_path(b"/docs/readme.txt")
        .or_else(|| alloc_path(b"/docs/readme.txt", false, SMB_SRCH_ARCHIVE as u32))
    {
        PATH_SLOT_SIZE[idx].store(6, Ordering::Release);
        unsafe {
            PATH_SLOT_CONTENT[idx][..6].copy_from_slice(b"README");
        }
    }
}

fn has_text(ptr: *const c_char) -> bool {
    !ptr.is_null() && unsafe { !CStr::from_ptr(ptr).to_bytes().is_empty() }
}

fn valid_conn(conn: SmbConn) -> bool {
    conn != 0 && conn == SMB_LAST_CONN.load(Ordering::Acquire)
}

fn file_slot(file: SmbFile) -> Option<usize> {
    let handle = file as usize as u32;
    (0..FILE_SLOT_USED.len()).find(|idx| {
        FILE_SLOT_USED[*idx].load(Ordering::Acquire)
            && FILE_SLOT_HANDLE[*idx].load(Ordering::Acquire) == handle
            && valid_conn(FILE_SLOT_CONN[*idx].load(Ordering::Acquire))
    })
}

fn alloc_file(conn: SmbConn, path_idx: usize, access: u16) -> SmbFile {
    let handle = SMB_NEXT_FILE.fetch_add(1, Ordering::AcqRel).max(1);
    for idx in 0..FILE_SLOT_USED.len() {
        if !FILE_SLOT_USED[idx].swap(true, Ordering::AcqRel) {
            FILE_SLOT_HANDLE[idx].store(handle, Ordering::Release);
            FILE_SLOT_CONN[idx].store(conn, Ordering::Release);
            FILE_SLOT_PATH[idx].store(path_idx as u32 + 1, Ordering::Release);
            FILE_SLOT_ACCESS[idx].store(access, Ordering::Release);
            return handle as usize as SmbFile;
        }
    }
    core::ptr::null_mut()
}

fn release_file(file: SmbFile) {
    let handle = file as usize as u32;
    for idx in 0..FILE_SLOT_USED.len() {
        if FILE_SLOT_USED[idx].load(Ordering::Acquire)
            && FILE_SLOT_HANDLE[idx].load(Ordering::Acquire) == handle
        {
            FILE_SLOT_USED[idx].store(false, Ordering::Release);
            FILE_SLOT_HANDLE[idx].store(0, Ordering::Release);
            FILE_SLOT_CONN[idx].store(0, Ordering::Release);
            FILE_SLOT_PATH[idx].store(0, Ordering::Release);
            FILE_SLOT_ACCESS[idx].store(0, Ordering::Release);
        }
    }
}

fn invalidate_conn(conn: SmbConn) {
    if SMB_SEARCH_CONN.load(Ordering::Acquire) == conn {
        SMB_SEARCH_STEP.store(0, Ordering::Release);
        SMB_SEARCH_CONN.store(0, Ordering::Release);
    }
    for idx in 0..FILE_SLOT_USED.len() {
        if FILE_SLOT_USED[idx].load(Ordering::Acquire)
            && FILE_SLOT_CONN[idx].load(Ordering::Acquire) == conn
        {
            FILE_SLOT_USED[idx].store(false, Ordering::Release);
            FILE_SLOT_HANDLE[idx].store(0, Ordering::Release);
            FILE_SLOT_CONN[idx].store(0, Ordering::Release);
            FILE_SLOT_PATH[idx].store(0, Ordering::Release);
            FILE_SLOT_ACCESS[idx].store(0, Ordering::Release);
        }
    }
}

fn fill_path_entry(entry: *mut SmbDirEntry, path_idx: usize) {
    if entry.is_null() {
        return;
    }
    unsafe {
        *entry = SmbDirEntry::default();
        (*entry).attributes = PATH_SLOT_ATTRS[path_idx].load(Ordering::Acquire);
        (*entry).size = PATH_SLOT_SIZE[path_idx].load(Ordering::Acquire) as u64;
        let end = PATH_SLOT_NAME[path_idx]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(PATH_NAME_BYTES);
        for (idx, byte) in PATH_SLOT_NAME[path_idx][..end].iter().enumerate() {
            (*entry).name[idx] = *byte as c_char;
        }
    }
}

fn search_prefix_matches(path_idx: usize) -> bool {
    unsafe {
        let prefix = core::slice::from_raw_parts(
            core::ptr::addr_of!(SMB_SEARCH_PREFIX).cast::<u8>(),
            PATH_NAME_BYTES,
        );
        let end = prefix
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(PATH_NAME_BYTES);
        if end == 0 {
            return true;
        }
        PATH_SLOT_NAME[path_idx].starts_with(&prefix[..end])
    }
}

fn search_match(path_idx: usize, filter: u32) -> bool {
    PATH_SLOT_USED[path_idx].load(Ordering::Acquire)
        && search_prefix_matches(path_idx)
        && (filter == 0 || (PATH_SLOT_ATTRS[path_idx].load(Ordering::Acquire) & filter) != 0)
}

/// Initializes the host-compat SMB device entry point used by libogc callers.
///
/// # Safety
/// All string pointers follow the C ABI. Each non-null pointer must reference a
/// valid NUL-terminated string for the duration of this call.
pub unsafe fn init_device(
    _name: *const c_char,
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::smb_init_device(_name, user, password, share, ip);
        }
    }
    if !smb_host_compatibility_enabled() {
        return false;
    }
    let mounted = has_text(_name);
    SMB_DEVICE_MOUNTED.store(mounted, Ordering::Release);
    if mounted {
        unsafe {
            DEVICE_NAME = [0; 32];
            let bytes = CStr::from_ptr(_name).to_bytes();
            let len = bytes.len().min(31);
            DEVICE_NAME[..len].copy_from_slice(&bytes[..len]);
        }
    }
    unsafe { init(user, password, share, ip) }
}

/// Initializes the host-compat SMB client state.
///
/// # Safety
/// All string pointers follow the C ABI. Each non-null pointer must reference a
/// valid NUL-terminated string for the duration of this call.
pub unsafe fn init(
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::smb_init(user, password, share, ip);
        }
    }
    if !smb_host_compatibility_enabled() {
        return false;
    }
    let _ = password;
    let ready = has_text(user) && has_text(share) && has_text(ip);
    SMB_INITED.store(ready, Ordering::Release);
    if ready {
        clear_path_slots();
        ensure_default_share();
    }
    ready
}

/// Shuts down the host-compat SMB device state.
///
/// # Safety
/// `_name` follows the original C ABI and, when non-null, must point to a
/// valid NUL-terminated string.
pub unsafe fn close_device(_name: *const c_char) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            powerpc_backend::smb_close(_name);
            return;
        }
    }
    if !smb_host_compatibility_enabled() {
        return;
    }
    SMB_INITED.store(false, Ordering::Release);
    SMB_DEVICE_MOUNTED.store(false, Ordering::Release);
    SMB_LAST_CONN.store(0, Ordering::Release);
    SMB_SEARCH_STEP.store(0, Ordering::Release);
    SMB_SEARCH_CONN.store(0, Ordering::Release);
    clear_path_slots();
    for idx in 0..FILE_SLOT_USED.len() {
        FILE_SLOT_USED[idx].store(false, Ordering::Release);
        FILE_SLOT_HANDLE[idx].store(0, Ordering::Release);
        FILE_SLOT_CONN[idx].store(0, Ordering::Release);
        FILE_SLOT_PATH[idx].store(0, Ordering::Release);
        FILE_SLOT_ACCESS[idx].store(0, Ordering::Release);
    }
}

/// Reports whether the host-compat SMB device is initialized.
///
/// # Safety
/// `_name` follows the original C ABI and, when non-null, must point to a
/// valid NUL-terminated string.
pub unsafe fn check_connection(_name: *const c_char) -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::smb_check_connection(_name);
        }
    }
    if !smb_host_compatibility_enabled() {
        return false;
    }
    if !SMB_INITED.load(Ordering::Acquire) {
        return false;
    }
    if _name.is_null() {
        return true;
    }
    if !SMB_DEVICE_MOUNTED.load(Ordering::Acquire) {
        return false;
    }
    let bytes = unsafe { CStr::from_ptr(_name) }.to_bytes();
    let device_name =
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(DEVICE_NAME).cast::<u8>(), 32) };
    let end = device_name.iter().position(|byte| *byte == 0).unwrap_or(32);
    &device_name[..end] == bytes
}

pub fn set_search_flags(flags: u16) {
    if cfg!(target_arch = "powerpc") {
        powerpc_backend::smb_set_search_flags(flags);
        return;
    }
    SMB_SEARCH_FLAGS.store(flags, Ordering::Release);
}

/// Creates a host-compat SMB connection handle.
///
/// # Safety
/// `smbhndl` must be a valid, writable pointer to one `SmbConn`. String
/// pointers, when non-null, must reference valid NUL-terminated strings.
pub unsafe fn connect(
    smbhndl: *mut SmbConn,
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::connect(smbhndl, user, password, share, ip);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    let _ = password;
    if smbhndl.is_null() {
        return SMB_BAD_LOGINDATA;
    }
    if !(SMB_INITED.load(Ordering::Acquire) || (has_text(user) && has_text(share) && has_text(ip)))
    {
        return SMB_BAD_LOGINDATA;
    }
    let conn = SMB_NEXT_CONN.fetch_add(1, Ordering::AcqRel);
    SMB_INITED.store(true, Ordering::Release);
    ensure_default_share();
    SMB_LAST_CONN.store(conn, Ordering::Release);
    unsafe {
        *smbhndl = conn;
    }
    SMB_SUCCESS
}

pub fn close_connection(_smbhndl: SmbConn) {
    if cfg!(target_arch = "powerpc") {
        powerpc_backend::close_connection(_smbhndl);
        return;
    }
    if !smb_host_compatibility_enabled() {
        return;
    }
    invalidate_conn(_smbhndl);
    if SMB_LAST_CONN.load(Ordering::Acquire) == _smbhndl {
        SMB_LAST_CONN.store(0, Ordering::Release);
    }
}

/// Reconnects or refreshes a host-compat SMB connection handle.
///
/// # Safety
/// `smbhndl` must be a valid, writable pointer to one `SmbConn`.
pub unsafe fn reconnect(smbhndl: *mut SmbConn, test_conn: bool) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::reconnect(smbhndl, if test_conn { 1 } else { 0 });
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if smbhndl.is_null() {
        return SMB_ERROR;
    }
    let current = unsafe { *smbhndl };
    if test_conn && valid_conn(current) {
        return SMB_SUCCESS;
    }
    if !SMB_INITED.load(Ordering::Acquire) {
        return SMB_ERROR;
    }
    invalidate_conn(current);
    let conn = SMB_NEXT_CONN.fetch_add(1, Ordering::AcqRel);
    SMB_LAST_CONN.store(conn, Ordering::Release);
    unsafe {
        *smbhndl = conn;
    }
    SMB_SUCCESS
}

/// Queries host-compat metadata for a path.
///
/// # Safety
/// `filename` must point to a valid NUL-terminated string when non-null.
/// `sdir` must be a valid, writable pointer to one `SmbDirEntry`.
pub unsafe fn path_info(filename: *const c_char, sdir: *mut SmbDirEntry, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::path_info(filename, sdir, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if !valid_conn(smbhndl) || filename.is_null() || sdir.is_null() {
        return SMB_ERROR;
    }
    let bytes = unsafe { CStr::from_ptr(filename) }.to_bytes();
    if let Some(path_idx) = find_path(bytes) {
        fill_path_entry(sdir, path_idx);
    } else {
        return SMB_ERROR;
    }
    SMB_SUCCESS
}

/// Begins a host-compat directory search.
///
/// # Safety
/// `filename` must point to a valid NUL-terminated string when non-null.
/// `sdir` must be a valid, writable pointer to one `SmbDirEntry`.
pub unsafe fn find_first(
    filename: *const c_char,
    flags: u16,
    sdir: *mut SmbDirEntry,
    smbhndl: SmbConn,
) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::find_first(filename, flags, sdir, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if !valid_conn(smbhndl) || filename.is_null() || sdir.is_null() {
        return SMB_ERROR;
    }
    SMB_SEARCH_FLAGS.store(flags, Ordering::Release);
    let bytes = unsafe { CStr::from_ptr(filename) }.to_bytes();
    let len = bytes.len().min(PATH_NAME_BYTES.saturating_sub(1));
    unsafe {
        SMB_SEARCH_PREFIX = [0; PATH_NAME_BYTES];
        SMB_SEARCH_PREFIX[..len].copy_from_slice(&bytes[..len]);
    }
    SMB_SEARCH_CONN.store(smbhndl, Ordering::Release);
    let filter = SMB_SEARCH_FLAGS.load(Ordering::Acquire) as u32;
    if let Some(path_idx) = (0..PATH_SLOTS).find(|idx| search_match(*idx, filter)) {
        SMB_SEARCH_STEP.store(path_idx as u32 + 1, Ordering::Release);
        fill_path_entry(sdir, path_idx);
        return SMB_SUCCESS;
    }
    SMB_ERROR
}

/// Advances a host-compat directory search result.
///
/// # Safety
/// `sdir` must be a valid, writable pointer to one `SmbDirEntry`.
pub unsafe fn find_next(sdir: *mut SmbDirEntry, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::find_next(sdir, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if !valid_conn(smbhndl)
        || sdir.is_null()
        || SMB_SEARCH_CONN.load(Ordering::Acquire) != smbhndl
        || SMB_SEARCH_STEP.load(Ordering::Acquire) == 0
    {
        return SMB_ERROR;
    }
    let filter = SMB_SEARCH_FLAGS.load(Ordering::Acquire) as u32;
    let start = SMB_SEARCH_STEP.fetch_add(1, Ordering::AcqRel) as usize;
    if let Some(path_idx) = (start..PATH_SLOTS).find(|idx| search_match(*idx, filter)) {
        fill_path_entry(sdir, path_idx);
        return SMB_SUCCESS;
    }
    SMB_ERROR
}

/// Closes a host-compat directory search result.
///
/// # Safety
/// `sdir` must be a valid pointer consistent with the original C ABI.
pub unsafe fn find_close(sdir: *mut SmbDirEntry, smbhndl: SmbConn) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::find_close(sdir, smbhndl);
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if !valid_conn(smbhndl) || sdir.is_null() {
        return SMB_ERROR;
    }
    SMB_SEARCH_STEP.store(0, Ordering::Release);
    SMB_SEARCH_CONN.store(0, Ordering::Release);
    SMB_SUCCESS
}

/// Opens a host-compat SMB file handle.
///
/// # Safety
/// `filename` must point to a valid NUL-terminated string when non-null.
pub unsafe fn open_file(
    filename: *const c_char,
    _access: u16,
    _creation: u16,
    smbhndl: SmbConn,
) -> SmbFile {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::open_file(filename, _access, _creation, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return core::ptr::null_mut();
    }
    if !valid_conn(smbhndl) || filename.is_null() {
        return core::ptr::null_mut();
    }
    let path = unsafe { CStr::from_ptr(filename) }.to_bytes();
    let path_idx = if let Some(path_idx) = find_path(path) {
        if PATH_SLOT_IS_DIR[path_idx].load(Ordering::Acquire) {
            return core::ptr::null_mut();
        }
        if (_creation & SMB_OF_TRUNCATE) != 0 {
            PATH_SLOT_SIZE[path_idx].store(0, Ordering::Release);
            unsafe { PATH_SLOT_CONTENT[path_idx] = [0; PATH_CONTENT_BYTES] };
        }
        path_idx
    } else if (_creation & SMB_OF_CREATE) != 0 || (_creation & SMB_OF_OPEN) != 0 {
        if !parent_exists(path) {
            return core::ptr::null_mut();
        }
        match alloc_path(path, false, SMB_SRCH_ARCHIVE as u32) {
            Some(idx) => idx,
            None => return core::ptr::null_mut(),
        }
    } else {
        return core::ptr::null_mut();
    };
    alloc_file(smbhndl, path_idx, _access)
}

pub fn close_file(sfid: SmbFile) {
    if cfg!(target_arch = "powerpc") {
        powerpc_backend::close_file(sfid);
        return;
    }
    if !smb_host_compatibility_enabled() {
        return;
    }
    release_file(sfid);
}

/// Reads zeros into a caller-owned buffer for the host-compat SMB file path.
///
/// # Safety
/// `buffer` must point to at least `size` writable bytes. `sfid` must be a
/// valid host-compat file handle from this ABI surface.
pub unsafe fn read_file(buffer: *mut c_char, size: usize, _offset: isize, sfid: SmbFile) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::read_file(buffer, size, _offset, sfid);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    let Some(file_idx) = file_slot(sfid) else {
        return SMB_ERROR;
    };
    if buffer.is_null() {
        return SMB_ERROR;
    }
    let access = FILE_SLOT_ACCESS[file_idx].load(Ordering::Acquire);
    if access == SMB_OPEN_WRITING {
        return SMB_ERROR;
    }
    let path_idx = FILE_SLOT_PATH[file_idx]
        .load(Ordering::Acquire)
        .saturating_sub(1) as usize;
    let file_size = PATH_SLOT_SIZE[path_idx].load(Ordering::Acquire) as usize;
    let start = _offset.max(0) as usize;
    let count = size.min(file_size.saturating_sub(start));
    unsafe {
        core::ptr::write_bytes(buffer.cast::<u8>(), 0, size);
        if count != 0 {
            core::ptr::copy_nonoverlapping(
                PATH_SLOT_CONTENT[path_idx][start..start + count].as_ptr(),
                buffer.cast::<u8>(),
                count,
            );
        }
    }
    count as i32
}

/// Accepts caller-owned bytes for the host-compat SMB file path.
///
/// # Safety
/// `buffer` must point to at least `size` readable bytes. `sfid` must be a
/// valid host-compat file handle from this ABI surface.
pub unsafe fn write_file(buffer: *const c_char, size: usize, _offset: isize, sfid: SmbFile) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::write_file(buffer, size, _offset, sfid);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    let Some(file_idx) = file_slot(sfid) else {
        return SMB_ERROR;
    };
    if buffer.is_null() {
        return SMB_ERROR;
    }
    let access = FILE_SLOT_ACCESS[file_idx].load(Ordering::Acquire);
    if access == SMB_OPEN_READING {
        return SMB_ERROR;
    }
    let path_idx = FILE_SLOT_PATH[file_idx]
        .load(Ordering::Acquire)
        .saturating_sub(1) as usize;
    let start = _offset.max(0) as usize;
    let count = size.min(PATH_CONTENT_BYTES.saturating_sub(start));
    unsafe {
        core::ptr::copy_nonoverlapping(
            buffer.cast::<u8>(),
            PATH_SLOT_CONTENT[path_idx][start..start + count].as_mut_ptr(),
            count,
        );
    }
    PATH_SLOT_SIZE[path_idx].store((start + count) as u32, Ordering::Release);
    count as i32
}

/// Creates a host-compat SMB directory.
///
/// # Safety
/// `dirname` must point to a valid NUL-terminated string when non-null.
pub unsafe fn create_directory(dirname: *const c_char, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::create_directory(dirname, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if dirname.is_null() || !valid_conn(smbhndl) {
        return SMB_ERROR;
    }
    let path = unsafe { CStr::from_ptr(dirname) }.to_bytes();
    if !parent_exists(path) {
        return SMB_ERROR;
    }
    if find_path(path).is_none() && alloc_path(path, true, SMB_SRCH_DIRECTORY as u32).is_none() {
        return SMB_ERROR;
    }
    SMB_SUCCESS
}

/// Deletes a host-compat SMB directory.
///
/// # Safety
/// `dirname` must point to a valid NUL-terminated string when non-null.
pub unsafe fn delete_directory(dirname: *const c_char, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::delete_directory(dirname, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if dirname.is_null() || !valid_conn(smbhndl) {
        return SMB_ERROR;
    }
    let Some(path_idx) = find_path(unsafe { CStr::from_ptr(dirname) }.to_bytes()) else {
        return SMB_ERROR;
    };
    if !PATH_SLOT_IS_DIR[path_idx].load(Ordering::Acquire) {
        return SMB_ERROR;
    }
    if has_children(path_idx) {
        return SMB_ERROR;
    }
    PATH_SLOT_USED[path_idx].store(false, Ordering::Release);
    SMB_SUCCESS
}

/// Deletes a host-compat SMB file entry.
///
/// # Safety
/// `filename` must point to a valid NUL-terminated string when non-null.
pub unsafe fn delete_file(filename: *const c_char, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::delete_file(filename, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if filename.is_null() || !valid_conn(smbhndl) {
        return SMB_ERROR;
    }
    let Some(path_idx) = find_path(unsafe { CStr::from_ptr(filename) }.to_bytes()) else {
        return SMB_ERROR;
    };
    if PATH_SLOT_IS_DIR[path_idx].load(Ordering::Acquire) {
        return SMB_ERROR;
    }
    PATH_SLOT_USED[path_idx].store(false, Ordering::Release);
    SMB_SUCCESS
}

/// Renames a host-compat SMB file entry.
///
/// # Safety
/// `filename` and `newname` must point to valid NUL-terminated strings when
/// non-null.
pub unsafe fn rename(filename: *const c_char, newname: *const c_char, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::rename(filename, newname, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if filename.is_null() || newname.is_null() || !valid_conn(smbhndl) {
        return SMB_ERROR;
    }
    let Some(path_idx) = find_path(unsafe { CStr::from_ptr(filename) }.to_bytes()) else {
        return SMB_ERROR;
    };
    let old_path_idx = path_idx;
    let new_bytes = unsafe { CStr::from_ptr(newname) }.to_bytes();
    if find_path(new_bytes).is_some() || !parent_exists(new_bytes) {
        return SMB_ERROR;
    }
    unsafe {
        PATH_SLOT_NAME[old_path_idx] = [0; PATH_NAME_BYTES];
        let len = new_bytes.len().min(PATH_NAME_BYTES.saturating_sub(1));
        PATH_SLOT_NAME[old_path_idx][..len].copy_from_slice(&new_bytes[..len]);
    }
    SMB_SUCCESS
}

/// Returns host-compat disk information state for an SMB connection.
///
/// # Safety
/// `_buf` follows the original C ABI. When non-null it must point to caller
/// storage suitable for the eventual real implementation.
pub unsafe fn disk_information(_buf: *mut c_void, smbhndl: SmbConn) -> i32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if smb_powerpc_backend_enabled() {
            return powerpc_backend::disk_information(_buf, smbhndl);
        }
    }
    if !smb_host_compatibility_enabled() {
        return SMB_ERROR;
    }
    if !valid_conn(smbhndl) || _buf.is_null() {
        return SMB_ERROR;
    }
    let mut files = 0_u32;
    let mut blocks = 0_u32;
    for idx in 0..PATH_SLOTS {
        if PATH_SLOT_USED[idx].load(Ordering::Acquire) {
            files += 1;
            blocks += PATH_SLOT_SIZE[idx]
                .load(Ordering::Acquire)
                .div_ceil(512)
                .max(1);
        }
    }
    unsafe {
        *(_buf.cast::<HostStatVfs>()) = HostStatVfs {
            f_bsize: 512,
            f_frsize: 512,
            f_blocks: blocks.max(64),
            f_bfree: 1024_u32.saturating_sub(blocks),
            f_bavail: 1024_u32.saturating_sub(blocks),
            f_files: files.max(16),
            f_ffree: 256_u32.saturating_sub(files),
            f_favail: 256_u32.saturating_sub(files),
            f_fsid: smbhndl,
            f_flag: 0,
            f_namemax: PATH_NAME_BYTES as u32 - 1,
        };
    }
    SMB_SUCCESS
}

pub(crate) fn utf16le_to_utf8(dst: &mut [u8], src: &[u8]) {
    let mut input = 0usize;
    let mut output = 0usize;

    while input + 1 < src.len() {
        let first = u16::from_le_bytes([src[input], src[input + 1]]);
        input += 2;
        if first == 0 {
            break;
        }

        let codepoint = if (0xD800..=0xDBFF).contains(&first) {
            if input + 1 < src.len() {
                let second = u16::from_le_bytes([src[input], src[input + 1]]);
                if (0xDC00..=0xDFFF).contains(&second) {
                    input += 2;
                    0x10000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00))
                } else {
                    0xFFFD
                }
            } else {
                0xFFFD
            }
        } else if (0xDC00..=0xDFFF).contains(&first) {
            0xFFFD
        } else {
            first as u32
        };

        let mut encoded = [0u8; 4];
        let encoded_len = match codepoint {
            0x0000..=0x007F => {
                encoded[0] = codepoint as u8;
                1
            }
            0x0080..=0x07FF => {
                encoded[0] = 0xC0 | (codepoint >> 6) as u8;
                encoded[1] = 0x80 | (codepoint & 0x3F) as u8;
                2
            }
            0x0800..=0xFFFF => {
                encoded[0] = 0xE0 | (codepoint >> 12) as u8;
                encoded[1] = 0x80 | ((codepoint >> 6) & 0x3F) as u8;
                encoded[2] = 0x80 | (codepoint & 0x3F) as u8;
                3
            }
            _ => {
                encoded[0] = 0xF0 | (codepoint >> 18) as u8;
                encoded[1] = 0x80 | ((codepoint >> 12) & 0x3F) as u8;
                encoded[2] = 0x80 | ((codepoint >> 6) & 0x3F) as u8;
                encoded[3] = 0x80 | (codepoint & 0x3F) as u8;
                4
            }
        };
        if output + encoded_len >= dst.len() {
            break;
        }
        dst[output..output + encoded_len].copy_from_slice(&encoded[..encoded_len]);
        output += encoded_len;
    }
}

#[cfg(test)]
mod utf16_tests {
    use super::utf16le_to_utf8;

    #[test]
    fn decodes_non_ascii_and_surrogate_pairs_as_utf8() {
        let source = [0xE9, 0x00, 0x3D, 0xD8, 0x00, 0xDE, 0x00, 0x00];
        let mut output = [0u8; 16];
        utf16le_to_utf8(&mut output, &source);
        assert_eq!(&output[..6], "é😀".as_bytes());
        assert_eq!(output[6], 0);
    }

    #[test]
    fn replaces_invalid_surrogates_and_keeps_output_terminated() {
        let source = [0x00, 0xD8, 0x41, 0x00, 0x00, 0x00];
        let mut output = [0u8; 8];
        utf16le_to_utf8(&mut output, &source);
        assert_eq!(&output[..4], [0xEF, 0xBF, 0xBD, b'A']);
        assert_eq!(output[4], 0);
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::size_of;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn constants_and_layout_match_checkpoint() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(SMB_MAXPATH, 4096);
        assert_eq!(SMB_SUCCESS, 0);
        assert_eq!(SMB_BAD_LOGINDATA, -8);
        assert_eq!(SMB_OF_CREATE, 16);
        assert_eq!(SMB_DENY_NONE, 0x40);
        assert_eq!(size_of::<SmbDirEntry>(), 808);
    }

    #[test]
    fn exports_exist() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _ =
            init as unsafe fn(*const c_char, *const c_char, *const c_char, *const c_char) -> bool;
        let _ = connect
            as unsafe fn(
                *mut SmbConn,
                *const c_char,
                *const c_char,
                *const c_char,
                *const c_char,
            ) -> i32;
        let _ = open_file as unsafe fn(*const c_char, u16, u16, SmbConn) -> SmbFile;
    }

    #[test]
    fn invalid_arguments_are_rejected_by_the_target_backend() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut conn = 0;
        unsafe {
            assert_eq!(
                connect(
                    &mut conn,
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null()
                ),
                SMB_BAD_LOGINDATA
            );
            assert!(open_file(core::ptr::null(), 0, 0, 1).is_null());
            assert_eq!(delete_file(core::ptr::null(), 1), SMB_ERROR);
        }
    }

    /// Requires an isolated, writable SMB1 test share and a new unique directory
    /// name. Missing configuration fails the test; no fake backend is selected.
    #[test]
    fn target_smb_server_file_search_and_reconnect_lifecycle() {
        use std::ffi::CString;
        use std::format;
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let configured = |key: &str| {
            CString::new(
                std::env::var(key)
                    .unwrap_or_else(|_| panic!("missing required SMB test setting {key}")),
            )
            .expect("SMB test setting contains NUL")
        };
        let user = configured("WII_SMB_TEST_USER");
        let password = configured("WII_SMB_TEST_PASSWORD");
        let share = configured("WII_SMB_TEST_SHARE");
        let server = configured("WII_SMB_TEST_SERVER");
        let name = std::env::var("WII_SMB_TEST_DIRECTORY").expect("missing WII_SMB_TEST_DIRECTORY");
        assert!(
            name.starts_with("libogc-test-")
                && name.len() <= 80
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "directory must be a unique libogc-test-* name"
        );
        let directory = CString::new(format!("/{name}")).unwrap();
        let original = CString::new(format!("/{name}/data.bin")).unwrap();
        let renamed = CString::new(format!("/{name}/renamed.bin")).unwrap();
        let second_path = CString::new(format!("/{name}/second.bin")).unwrap();
        let pattern = CString::new(format!("/{name}/*")).unwrap();
        let mut conn = 0;
        unsafe {
            assert!(init(
                user.as_ptr(),
                password.as_ptr(),
                share.as_ptr(),
                server.as_ptr()
            ));
            assert!(check_connection(c"smb".as_ptr()));
            close_device(c"smb".as_ptr());
            assert!(!check_connection(c"smb".as_ptr()));
            assert!(init_device(
                c"smbtest".as_ptr(),
                user.as_ptr(),
                password.as_ptr(),
                share.as_ptr(),
                server.as_ptr()
            ));
            assert!(check_connection(c"smbtest:/".as_ptr()));
            assert!(!init_device(
                c"smbtest".as_ptr(),
                user.as_ptr(),
                password.as_ptr(),
                share.as_ptr(),
                server.as_ptr()
            ));
            close_device(c"smbtest".as_ptr());
            assert!(!check_connection(c"smbtest".as_ptr()));
            assert_eq!(
                connect(
                    &mut conn,
                    user.as_ptr(),
                    password.as_ptr(),
                    share.as_ptr(),
                    server.as_ptr()
                ),
                SMB_SUCCESS
            );
            assert_ne!(conn, 0);
            // Only continue when a new directory was created. This prevents any
            // existing user file from being truncated or removed by the test.
            assert_eq!(
                create_directory(directory.as_ptr(), conn),
                SMB_SUCCESS,
                "test directory must not already exist"
            );
            let missing = CString::new(format!("/{name}/absent/file.bin")).unwrap();
            let mut missing_info = SmbDirEntry::default();
            assert_eq!(
                path_info(missing.as_ptr(), &mut missing_info, conn),
                SMB_ERROR
            );
            assert!(open_file(missing.as_ptr(), SMB_OPEN_READWRITE, SMB_OF_CREATE, conn).is_null());
            let file = open_file(original.as_ptr(), SMB_OPEN_READWRITE, SMB_OF_CREATE, conn);
            assert!(!file.is_null(), "create test file");
            let bytes = b"SMB-probe-51a7";
            assert_eq!(
                write_file(bytes.as_ptr().cast(), bytes.len(), 0, file),
                bytes.len() as i32
            );
            let mut readback = [0u8; 14];
            assert_eq!(
                read_file(readback.as_mut_ptr().cast(), readback.len(), 0, file),
                bytes.len() as i32
            );
            assert_eq!(&readback, bytes);
            assert_eq!(write_file(b"X".as_ptr().cast(), 1, 4, file), 1);
            assert_eq!(
                read_file(readback.as_mut_ptr().cast(), readback.len(), 0, file),
                bytes.len() as i32
            );
            let mut expected = *bytes;
            expected[4] = b'X';
            assert_eq!(readback, expected);
            let second_file =
                open_file(second_path.as_ptr(), SMB_OPEN_WRITING, SMB_OF_CREATE, conn);
            assert!(!second_file.is_null(), "create second test file");
            assert_eq!(write_file(b"second".as_ptr().cast(), 6, 0, second_file), 6);
            close_file(second_file);
            assert_eq!(delete_directory(directory.as_ptr(), conn), SMB_ERROR);
            close_file(file);
            assert_eq!(
                read_file(readback.as_mut_ptr().cast(), readback.len(), 0, file),
                SMB_ERROR
            );
            let first = open_file(original.as_ptr(), SMB_OPEN_READING, SMB_OF_OPEN, conn);
            let second = open_file(original.as_ptr(), SMB_OPEN_READING, SMB_OF_OPEN, conn);
            assert!(!first.is_null() && !second.is_null() && first != second);
            assert_eq!(write_file(b"Z".as_ptr().cast(), 1, 0, first), SMB_ERROR);
            close_file(first);
            assert_eq!(
                read_file(readback.as_mut_ptr().cast(), readback.len(), 0, second),
                bytes.len() as i32
            );
            close_file(second);
            assert_eq!(
                rename(original.as_ptr(), renamed.as_ptr(), conn),
                SMB_SUCCESS
            );
            let mut info = SmbDirEntry::default();
            assert_eq!(path_info(renamed.as_ptr(), &mut info, conn), SMB_SUCCESS);
            assert_eq!(info.size, bytes.len() as u64);
            let mut entry = SmbDirEntry::default();
            assert_eq!(
                find_first(pattern.as_ptr(), SMB_SRCH_ARCHIVE, &mut entry, conn),
                SMB_SUCCESS
            );
            let first_name = entry.name;
            assert_eq!(find_next(&mut entry, conn), SMB_SUCCESS);
            assert_ne!(
                entry.name, first_name,
                "search must return two distinct files"
            );
            assert_eq!(find_close(&mut entry, conn), SMB_SUCCESS);
            let mut disk = [0u32; 11];
            assert_eq!(
                disk_information(disk.as_mut_ptr().cast(), conn),
                SMB_SUCCESS
            );
            assert_ne!(disk[0], 0, "server must report a nonzero block size");
            assert_eq!(reconnect(&mut conn, false), SMB_SUCCESS);
            let reopened = open_file(renamed.as_ptr(), SMB_OPEN_READING, SMB_OF_OPEN, conn);
            assert!(!reopened.is_null());
            assert_eq!(
                read_file(readback.as_mut_ptr().cast(), readback.len(), 0, reopened),
                bytes.len() as i32
            );
            assert_eq!(readback, expected);
            close_file(reopened);
            assert_eq!(
                delete_file(renamed.as_ptr(), conn),
                SMB_SUCCESS,
                "cleanup file"
            );
            assert_eq!(
                delete_file(second_path.as_ptr(), conn),
                SMB_SUCCESS,
                "cleanup second file"
            );
            assert_eq!(
                delete_directory(directory.as_ptr(), conn),
                SMB_SUCCESS,
                "cleanup directory"
            );
            close_connection(conn);
        }
    }
}
