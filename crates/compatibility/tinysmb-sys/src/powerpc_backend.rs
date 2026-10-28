extern crate alloc;

use alloc::vec::Vec;
use core::ffi::{CStr, c_char, c_void};
use core::mem::{MaybeUninit, size_of};
use core::sync::atomic::{AtomicU16, Ordering};

use lwip_sys as lwip;
#[cfg(target_arch = "powerpc")]
use ogc_network_sys as network;

use crate::{
    Bool, HostStatVfs, SMB_BAD_COMMAND, SMB_BAD_KEYLEN, SMB_BAD_LOGINDATA, SMB_BAD_PROTOCOL,
    SMB_ERROR, SMB_MAXPATH, SMB_OF_CREATE, SMB_OF_OPEN, SMB_OF_TRUNCATE, SMB_OPEN_READING,
    SMB_OPEN_READWRITE, SMB_OPEN_WRITING, SMB_SRCH_DIRECTORY, SMB_SUCCESS, SmbConn, SmbDirEntry,
    SmbFile, ntlm,
};
use ogc_sysbase::{Devoptab, DirIter, Reent, Stat, StatVfs};

const MAX_MOUNTS: usize = 10;
const MAX_DEVICE_NAME: usize = 9;
const SMB_BUF_BYTES: usize = 4096;
const RECV_TIMEOUT_MS: u64 = 3_000;
const CONN_TIMEOUT_MS: u64 = 6_000;
const WII_TIMER_CLOCK_HZ: u64 = 60_750_000;
const S_IFDIR: u32 = 0o040000;

const SMB_OFFSET_PROTO: usize = 0;
const SMB_OFFSET_CMD: usize = 4;
const SMB_OFFSET_NTSTATUS: usize = 5;
const SMB_OFFSET_FLAGS: usize = 9;
const SMB_OFFSET_FLAGS2: usize = 10;
const SMB_OFFSET_TID: usize = 24;
const SMB_OFFSET_PID: usize = 26;
const SMB_OFFSET_UID: usize = 28;
const SMB_OFFSET_MID: usize = 30;
const SMB_HEADER_SIZE: usize = 32;

const NBT_SESSION_MSG: u8 = 0x00;
const SESS_REQ: u8 = 0x81;
const SESS_POS_RESP: u8 = 0x82;

const SMB_NEG_PROTOCOL: u8 = 0x72;
const SMB_SETUP_ANDX: u8 = 0x73;
const SMB_TREEC_ANDX: u8 = 0x75;
const SMB_TRANS2: u8 = 0x32;
const SMB_OPEN_ANDX: u8 = 0x2d;
const SMB_READ_ANDX: u8 = 0x2e;
const SMB_WRITE_ANDX: u8 = 0x2f;
const SMB_CLOSE: u8 = 0x04;
const SMB_COM_CREATE_DIRECTORY: u8 = 0x00;
const SMB_COM_DELETE_DIRECTORY: u8 = 0x01;
const SMB_COM_DELETE: u8 = 0x06;
const SMB_COM_RENAME: u8 = 0x07;
const SMB_COM_QUERY_INFORMATION_DISK: u8 = 0x80;
const SMB_QUERY_PATH_INFO: u8 = 5;
const SMB_FIND_FIRST2: u8 = 1;
const SMB_FIND_NEXT2: u8 = 2;
const SMB_QUERY_FILE_ALL_INFO: u16 = 0x107;
const SMB_FIND_CLOSE2: u8 = 0x34;

const T2_WORD_CNT: usize = SMB_HEADER_SIZE;
const T2_PRM_CNT: usize = T2_WORD_CNT + 1;
const T2_DATA_CNT: usize = T2_PRM_CNT + 2;
const T2_MAXPRM_CNT: usize = T2_DATA_CNT + 2;
const T2_MAXBUFFER: usize = T2_MAXPRM_CNT + 2;
const T2_SETUP_CNT: usize = T2_MAXBUFFER + 2;
const T2_SPRM_CNT: usize = T2_SETUP_CNT + 10;
const T2_SPRM_OFS: usize = T2_SPRM_CNT + 2;
const T2_SDATA_CNT: usize = T2_SPRM_OFS + 2;
const T2_SDATA_OFS: usize = T2_SDATA_CNT + 2;
const T2_SSETUP_CNT: usize = T2_SDATA_OFS + 2;
const T2_SUB_CMD: usize = T2_SSETUP_CNT + 2;
const T2_BYTE_CNT: usize = T2_SUB_CMD + 2;

const SMB_PROTO: u32 = 0x424d53ff;
const SMB_MAX_TRANSMIT_SIZE: u32 = 65472;
const CAP_UNICODE: u32 = 0x00000004;
const CIFS_FLAGS1: u8 = 0x08;
const CIFS_FLAGS2: u16 = 0x0001;
const CIFS_FLAGS2_UNICODE: u16 = 0x8001;
const TCP_NODELAY: i32 = 0x01;
const EAGAIN: i32 = 11;
const EISCONN: i32 = 106;
const EALREADY: i32 = 114;
const EINPROGRESS: i32 = 115;

static SMB_SEARCH_FLAGS: AtomicU16 = AtomicU16::new(0);

const MAX_OPEN_FILES: usize = 256;

struct Session {
    conn_valid: bool,
    socket: i32,
    port_be: u16,
    server_ip: u32,
    unicode: bool,
    user: Vec<u8>,
    password: Vec<u8>,
    share: Vec<u8>,
    server: Vec<u8>,
    tid: u16,
    pid: u16,
    uid: u16,
    mid: u16,
    skey: u32,
    capabilities: u32,
    max_buffer: u32,
    max_mpx: u16,
    max_vcs: u16,
    challenge: [u8; 8],
    p_domain: [u8; 64],
    time_offset: i64,
    challenge_used: bool,
    search_eos: bool,
}

impl Session {
    fn new() -> Self {
        Self {
            conn_valid: false,
            socket: -1,
            port_be: 0,
            server_ip: 0,
            unicode: false,
            user: Vec::new(),
            password: Vec::new(),
            share: Vec::new(),
            server: Vec::new(),
            tid: 0,
            pid: 0xdead,
            uid: 0,
            mid: 1,
            skey: 0,
            capabilities: 0,
            max_buffer: 0,
            max_mpx: 0,
            max_vcs: 0,
            challenge: [0; 8],
            p_domain: [0; 64],
            time_offset: 0,
            challenge_used: false,
            search_eos: true,
        }
    }
}

#[derive(Copy, Clone)]
struct Mount {
    used: bool,
    registered: bool,
    name: [u8; MAX_DEVICE_NAME],
    conn: SmbConn,
    current_path: [u8; SMB_MAXPATH],
    first_item_dir: bool,
    diropen_root: bool,
}

impl Mount {
    const fn new() -> Self {
        let mut current_path = [0; SMB_MAXPATH];
        current_path[0] = b'\\';
        Self {
            used: false,
            registered: false,
            name: [0; MAX_DEVICE_NAME],
            conn: 0,
            current_path,
            first_item_dir: false,
            diropen_root: false,
        }
    }
}

#[repr(C)]
struct SmbFileState {
    handle: SmbFile,
    offset: i64,
    len: i64,
    filename: [c_char; SMB_MAXPATH],
    access: u16,
    env: i32,
    attributes: u32,
}

#[repr(C)]
struct SmbDirState {
    smbdir: SmbDirEntry,
    env: i32,
    dir: [c_char; SMB_MAXPATH],
}

fn valid_device_name(name: &[u8]) -> bool {
    !name.is_empty() && name.len() < MAX_DEVICE_NAME
}

fn mount_name_exists(mounts: &[Mount], name: &[u8]) -> bool {
    mounts
        .iter()
        .any(|mount| mount.used && read_c_buf(&mount.name) == name)
}

fn mount_record(name: &[u8], conn: SmbConn) -> Option<Mount> {
    if !valid_device_name(name) {
        return None;
    }
    let mut mount = Mount::new();
    mount.used = true;
    mount.name[..name.len()].copy_from_slice(name);
    mount.conn = conn;
    Some(mount)
}

fn device_name_for_find_device(name: &[u8]) -> Option<[c_char; MAX_DEVICE_NAME + 1]> {
    if !valid_device_name(name) {
        return None;
    }
    let mut out = [0; MAX_DEVICE_NAME + 1];
    for (dst, src) in out.iter_mut().zip(name) {
        *dst = *src as c_char;
    }
    out[name.len()] = b':' as c_char;
    Some(out)
}

static mut SESSIONS: [Option<Session>; MAX_MOUNTS] = [const { None }; MAX_MOUNTS];
static mut MOUNTS: [Mount; MAX_MOUNTS] = [const { Mount::new() }; MAX_MOUNTS];
static mut DEVOPTABS: [MaybeUninit<Devoptab>; MAX_MOUNTS] =
    [const { MaybeUninit::uninit() }; MAX_MOUNTS];

#[derive(Copy, Clone)]
struct FileHandle {
    used: bool,
    conn: SmbConn,
    sfid: u16,
}

impl FileHandle {
    const fn new() -> Self {
        Self {
            used: false,
            conn: 0,
            sfid: 0,
        }
    }
}

static mut FILE_HANDLES: [FileHandle; MAX_OPEN_FILES] =
    [const { FileHandle::new() }; MAX_OPEN_FILES];

fn empty_devoptab() -> Devoptab {
    Devoptab {
        name: core::ptr::null(),
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

fn normalized_lookup_name(name: &[u8]) -> &[u8] {
    match name.iter().position(|byte| *byte == b':') {
        Some(pos) => &name[..pos],
        None => name,
    }
}

fn mount_index_by_name(name: &[u8]) -> Option<usize> {
    let name = normalized_lookup_name(name);
    let mounts = core::ptr::addr_of!(MOUNTS).cast::<Mount>();
    (0..MAX_MOUNTS).find(|index| {
        let mount = unsafe { &*mounts.add(*index) };
        mount.used && read_c_buf(&mount.name) == name
    })
}

fn mount_path(path: *const c_char, env: usize, out: &mut [c_char; SMB_MAXPATH]) -> bool {
    if path.is_null() || env >= MAX_MOUNTS {
        return false;
    }
    let raw = unsafe { CStr::from_ptr(path) }.to_bytes();
    let tail = raw
        .iter()
        .position(|b| *b == b':')
        .map_or(raw, |i| &raw[i + 1..]);
    let mount = unsafe { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
    let absolute = tail.first().is_some_and(|b| *b == b'/' || *b == b'\\');
    let mut pos = 0usize;
    if absolute {
        if pos + 1 >= out.len() {
            return false;
        }
        out[pos] = b'\\' as c_char;
        pos += 1;
    } else {
        for &b in read_c_buf(&mount.current_path) {
            if pos + 1 >= out.len() {
                return false;
            }
            out[pos] = b as c_char;
            pos += 1;
        }
        if pos == 0 {
            out[pos] = b'\\' as c_char;
            pos += 1;
        }
        if pos > 0 && out[pos - 1] != b'\\' {
            out[pos] = b'\\' as c_char;
            pos += 1;
        }
    }
    for (i, &b) in tail.iter().enumerate() {
        if absolute && i == 0 {
            continue;
        }
        if pos + 1 >= out.len() {
            return false;
        }
        out[pos] = if b == b'/' { b'\\' } else { b } as c_char;
        pos += 1;
    }
    out[pos] = 0;
    true
}

fn filetime_to_unix_seconds(value: u64) -> i64 {
    (value / 10_000_000) as i64 - 11_644_473_600i64
}

fn dentry_to_stat(dentry: &SmbDirEntry, st: *mut c_void) -> bool {
    if st.is_null() {
        return false;
    }
    let stat = unsafe { &mut *st.cast::<Stat>() };
    *stat = Stat::default();
    stat.st_mode = if dentry.attributes & SMB_SRCH_DIRECTORY as u32 != 0 {
        S_IFDIR
    } else {
        ogc_sysbase::S_IFREG
    };
    stat.st_nlink = 1;
    stat.st_uid = 1;
    stat.st_gid = 2;
    stat.st_size = dentry.size as i64;
    stat.st_atim.tv_sec = filetime_to_unix_seconds(dentry.atime);
    stat.st_mtim.tv_sec = filetime_to_unix_seconds(dentry.mtime);
    stat.st_ctim.tv_sec = filetime_to_unix_seconds(dentry.ctime);
    stat.st_blksize = 1024;
    stat.st_blocks = dentry.size.div_ceil(1024) as i32;
    true
}

unsafe fn copy_c_chars(dst: &mut [c_char], src: *const c_char) -> bool {
    if src.is_null() {
        return false;
    }
    let bytes = unsafe { CStr::from_ptr(src) }.to_bytes_with_nul();
    if bytes.len() > dst.len() {
        return false;
    }
    for (d, b) in dst.iter_mut().zip(bytes.iter().copied()) {
        *d = b as c_char;
    }
    true
}

unsafe extern "C" fn smb_dev_open(
    r: *mut Reent,
    fs: *mut c_void,
    path: *const c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    unsafe {
        if fs.is_null() || path.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &mut *core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>().add(env) };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        if { reconnect(&mut mount.conn, 1) } != SMB_SUCCESS {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        }
        let mut dentry = SmbDirEntry::default();
        let exists = { path_info(fixed.as_ptr(), &mut dentry, mount.conn) } == SMB_SUCCESS;
        let access = match flags & ogc_sysbase::O_ACCMODE {
            ogc_sysbase::O_RDONLY => SMB_OPEN_READING,
            ogc_sysbase::O_WRONLY => SMB_OPEN_WRITING,
            ogc_sysbase::O_RDWR => SMB_OPEN_READWRITE,
            _ => {
                ogc_sysbase::set_reent_errno(r, ogc_sysbase::EACCES);
                return -1;
            }
        };
        let mut creation = if exists { SMB_OF_OPEN } else { SMB_OF_CREATE };
        if flags & ogc_sysbase::O_CREAT != 0 && !exists {
            creation = SMB_OF_CREATE;
        }
        if flags & ogc_sysbase::O_TRUNC != 0 && exists && access != SMB_OPEN_READING {
            creation = SMB_OF_TRUNCATE;
        }
        let handle = { open_file(fixed.as_ptr(), access, creation, mount.conn) };
        if handle.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOENT);
            return -1;
        }
        let state = { &mut *fs.cast::<SmbFileState>() };
        core::ptr::write_bytes(state as *mut SmbFileState, 0, 1);
        state.handle = handle;
        state.env = env as i32;
        state.access = access;
        if exists {
            state.len = dentry.size as i64;
            state.attributes = dentry.attributes;
        }
        state.offset = if flags & ogc_sysbase::O_APPEND != 0 {
            state.len
        } else {
            0
        };
        if !{ copy_c_chars(&mut state.filename, fixed.as_ptr()) } {
            close_file(handle);
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        0
    }
}

unsafe extern "C" fn smb_dev_close(_r: *mut Reent, fs: *mut c_void) -> i32 {
    if fs.is_null() {
        return -1;
    }
    let state = unsafe { &mut *fs.cast::<SmbFileState>() };
    close_file(state.handle);
    state.handle = core::ptr::null_mut();
    0
}
unsafe extern "C" fn smb_dev_seek(r: *mut Reent, fs: *mut c_void, pos: i64, dir: i32) -> i64 {
    unsafe {
        if fs.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EBADF);
            return -1;
        }
        let state = { &mut *fs.cast::<SmbFileState>() };
        let next = match dir {
            ogc_sysbase::SEEK_SET => pos,
            ogc_sysbase::SEEK_CUR => state.offset.saturating_add(pos),
            ogc_sysbase::SEEK_END => state.len.saturating_add(pos),
            _ => {
                ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
                return -1;
            }
        };
        if next < 0 {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        state.offset = next;
        next
    }
}
unsafe extern "C" fn smb_dev_read(
    r: *mut Reent,
    fs: *mut c_void,
    ptr: *mut c_char,
    len: usize,
) -> isize {
    unsafe {
        if fs.is_null() || ptr.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EBADF);
            return -1;
        }
        let state = { &mut *fs.cast::<SmbFileState>() };
        if len == 0 || state.offset >= state.len {
            return 0;
        }
        let amount = len.min((state.len - state.offset) as usize);
        let got = { read_file(ptr, amount, state.offset as isize, state.handle) };
        if got < 0 {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EIO);
            -1
        } else {
            state.offset += got as i64;
            got as isize
        }
    }
}
unsafe extern "C" fn smb_dev_write(
    r: *mut Reent,
    fs: *mut c_void,
    ptr: *const c_char,
    len: usize,
) -> isize {
    unsafe {
        if fs.is_null() || ptr.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EBADF);
            return -1;
        }
        let state = { &mut *fs.cast::<SmbFileState>() };
        if state.offset > state.len {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EOVERFLOW);
            return -1;
        }
        if len == 0 {
            return 0;
        }
        let wrote = { write_file(ptr, len, state.offset as isize, state.handle) };
        if wrote <= 0 {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EIO);
            -1
        } else {
            state.offset += wrote as i64;
            state.len = state.len.max(state.offset);
            wrote as isize
        }
    }
}
unsafe extern "C" fn smb_dev_fstat(r: *mut Reent, fs: *mut c_void, st: *mut c_void) -> i32 {
    unsafe {
        if fs.is_null() || st.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EBADF);
            return -1;
        }
        let state = { &*fs.cast::<SmbFileState>() };
        let stat = { &mut *st.cast::<Stat>() };
        *stat = Stat::default();
        stat.st_size = state.len;
        stat.st_mode = if state.attributes & SMB_SRCH_DIRECTORY as u32 != 0 {
            S_IFDIR
        } else {
            ogc_sysbase::S_IFREG
        };
        0
    }
}
unsafe extern "C" fn smb_dev_stat(r: *mut Reent, path: *const c_char, st: *mut c_void) -> i32 {
    unsafe {
        if path.is_null() || st.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        if raw == b"." || raw == b".." {
            let stat = { &mut *st.cast::<Stat>() };
            *stat = Stat::default();
            stat.st_mode = S_IFDIR;
            return 0;
        }
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let mut d = SmbDirEntry::default();
        if { path_info(fixed.as_ptr(), &mut d, mount.conn) } != SMB_SUCCESS {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOENT);
            return -1;
        }
        if dentry_to_stat(&d, st) { 0 } else { -1 }
    }
}
unsafe extern "C" fn smb_dev_mkdir(r: *mut Reent, path: *const c_char, _mode: i32) -> i32 {
    unsafe { smb_path_action(r, path, create_directory) }
}
unsafe extern "C" fn smb_dev_unlink(r: *mut Reent, path: *const c_char) -> i32 {
    unsafe {
        if path.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            return -1;
        }
        let mut d = SmbDirEntry::default();
        let is_dir = { path_info(fixed.as_ptr(), &mut d, mount.conn) } == SMB_SUCCESS
            && d.attributes & SMB_SRCH_DIRECTORY as u32 != 0;
        let rc = if is_dir {
            delete_directory(fixed.as_ptr(), mount.conn)
        } else {
            delete_file(fixed.as_ptr(), mount.conn)
        };
        if rc == SMB_SUCCESS { 0 } else { -1 }
    }
}
unsafe fn smb_path_action(
    r: *mut Reent,
    path: *const c_char,
    op: unsafe fn(*const c_char, SmbConn) -> i32,
) -> i32 {
    unsafe {
        if path.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            return -1;
        }
        if { op(fixed.as_ptr(), mount.conn) } == SMB_SUCCESS {
            0
        } else {
            -1
        }
    }
}
unsafe extern "C" fn smb_dev_rename(r: *mut Reent, old: *const c_char, new: *const c_char) -> i32 {
    unsafe {
        if old.is_null() || new.is_null() {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EINVAL);
            return -1;
        }
        let ob = { CStr::from_ptr(old) }.to_bytes();
        let Some(env) = mount_index_by_name(ob) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
        let mut a = [0 as c_char; SMB_MAXPATH];
        let mut b = [0 as c_char; SMB_MAXPATH];
        if !mount_path(old, env, &mut a) || !mount_path(new, env, &mut b) {
            return -1;
        }
        if { rename(a.as_ptr(), b.as_ptr(), mount.conn) } == SMB_SUCCESS {
            0
        } else {
            -1
        }
    }
}
unsafe extern "C" fn smb_dev_chdir(r: *mut Reent, path: *const c_char) -> i32 {
    unsafe {
        if path.is_null() {
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &mut *core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>().add(env) };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            return -1;
        }
        let mut d = SmbDirEntry::default();
        if { path_info(fixed.as_ptr(), &mut d, mount.conn) } != SMB_SUCCESS {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOENT);
            return -1;
        }
        if d.attributes & SMB_SRCH_DIRECTORY as u32 == 0 {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOTDIR);
            return -1;
        }
        mount.current_path = [0; SMB_MAXPATH];
        let bytes = { CStr::from_ptr(fixed.as_ptr()) }.to_bytes();
        if bytes.len() + 2 > SMB_MAXPATH {
            return -1;
        }
        mount.current_path[..bytes.len()].copy_from_slice(bytes);
        let mut n = bytes.len();
        if n == 0 || mount.current_path[n - 1] != b'\\' {
            mount.current_path[n] = b'\\';
            n += 1;
        }
        mount.current_path[n] = 0;
        0
    }
}
unsafe extern "C" fn smb_dev_diropen(
    r: *mut Reent,
    iter: *mut DirIter,
    path: *const c_char,
) -> *mut DirIter {
    unsafe {
        if iter.is_null() || path.is_null() {
            return core::ptr::null_mut();
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return core::ptr::null_mut();
        };
        let mount = { &mut *core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>().add(env) };
        let state = { &mut *(*iter).dir_struct.cast::<SmbDirState>() };
        let mut fixed = [0 as c_char; SMB_MAXPATH];
        if !mount_path(path, env, &mut fixed) {
            return core::ptr::null_mut();
        }
        let bytes = { CStr::from_ptr(fixed.as_ptr()) }.to_bytes();
        mount.diropen_root = bytes == b"\\";
        if !mount.diropen_root {
            let mut d = SmbDirEntry::default();
            if { path_info(fixed.as_ptr(), &mut d, mount.conn) } != SMB_SUCCESS
                || d.attributes & SMB_SRCH_DIRECTORY as u32 == 0
            {
                return core::ptr::null_mut();
            }
        }
        let mut pattern = [0 as c_char; SMB_MAXPATH];
        let plen = bytes.len();
        if plen + 3 > SMB_MAXPATH {
            return core::ptr::null_mut();
        }
        for (i, b) in bytes.iter().enumerate() {
            pattern[i] = *b as c_char;
        }
        let mut n = plen;
        if n == 0 || pattern[n - 1] != b'\\' {
            pattern[n] = b'\\' as c_char;
            n += 1;
        }
        pattern[n] = b'*' as c_char;
        pattern[n + 1] = 0;
        let flags = SMB_SEARCH_FLAGS.load(Ordering::Acquire);
        if { find_first(pattern.as_ptr(), flags, &mut state.smbdir, mount.conn) } != SMB_SUCCESS {
            return core::ptr::null_mut();
        }
        state.env = env as i32;
        state.dir = [0; SMB_MAXPATH];
        for (i, b) in bytes.iter().enumerate() {
            state.dir[i] = *b as c_char;
        }
        mount.first_item_dir = true;
        iter
    }
}
unsafe extern "C" fn smb_dev_dirreset(r: *mut Reent, iter: *mut DirIter) -> i32 {
    unsafe {
        if iter.is_null() {
            return -1;
        }
        let state = { &mut *(*iter).dir_struct.cast::<SmbDirState>() };
        if find_close(&mut state.smbdir, {
            (*core::ptr::addr_of!(MOUNTS)
                .cast::<Mount>()
                .add(state.env as usize))
            .conn
        }) != SMB_SUCCESS
        {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::EIO);
            return -1;
        }
        let mut path = [0 as c_char; SMB_MAXPATH];
        let bytes = { CStr::from_ptr(state.dir.as_ptr()) }.to_bytes();
        for (i, b) in bytes.iter().enumerate() {
            path[i] = *b as c_char;
        }
        let mut n = bytes.len();
        if n == 0 || path[n - 1] != b'\\' {
            path[n] = b'\\' as c_char;
            n += 1;
        }
        path[n] = b'*' as c_char;
        path[n + 1] = 0;
        let mount = {
            &mut *core::ptr::addr_of_mut!(MOUNTS)
                .cast::<Mount>()
                .add(state.env as usize)
        };
        if {
            find_first(
                path.as_ptr(),
                SMB_SEARCH_FLAGS.load(Ordering::Acquire),
                &mut state.smbdir,
                mount.conn,
            )
        } != SMB_SUCCESS
        {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOENT);
            return -1;
        }
        mount.first_item_dir = true;
        0
    }
}
unsafe extern "C" fn smb_dev_dirnext(
    r: *mut Reent,
    iter: *mut DirIter,
    filename: *mut c_char,
    st: *mut c_void,
) -> i32 {
    unsafe {
        if iter.is_null() || filename.is_null() || st.is_null() {
            return -1;
        }
        let state = { &mut *(*iter).dir_struct.cast::<SmbDirState>() };
        let mount = {
            &mut *core::ptr::addr_of_mut!(MOUNTS)
                .cast::<Mount>()
                .add(state.env as usize)
        };
        let mut d = state.smbdir;
        if mount.first_item_dir {
            mount.first_item_dir = false;
        } else {
            if { find_next(&mut d, mount.conn) } != SMB_SUCCESS {
                ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENOENT);
                return -1;
            }
            state.smbdir = d;
        }
        let name = { CStr::from_ptr(d.name.as_ptr()) }.to_bytes_with_nul();
        {
            core::ptr::copy_nonoverlapping(name.as_ptr().cast::<c_char>(), filename, name.len());
        }
        if dentry_to_stat(&d, st) { 0 } else { -1 }
    }
}
unsafe extern "C" fn smb_dev_dirclose(_r: *mut Reent, iter: *mut DirIter) -> i32 {
    unsafe {
        if iter.is_null() {
            return -1;
        }
        let state = { &mut *(*iter).dir_struct.cast::<SmbDirState>() };
        let mount = {
            &*core::ptr::addr_of!(MOUNTS)
                .cast::<Mount>()
                .add(state.env as usize)
        };
        if find_close(&mut state.smbdir, mount.conn) != SMB_SUCCESS {
            return -1;
        }
        core::ptr::write_bytes(state as *mut SmbDirState, 0, 1);
        0
    }
}
unsafe extern "C" fn smb_dev_statvfs(r: *mut Reent, path: *const c_char, buf: *mut c_void) -> i32 {
    unsafe {
        if path.is_null() || buf.is_null() {
            return -1;
        }
        let raw = { CStr::from_ptr(path) }.to_bytes();
        let Some(env) = mount_index_by_name(raw) else {
            ogc_sysbase::set_reent_errno(r, ogc_sysbase::ENODEV);
            return -1;
        };
        let mount = { &*core::ptr::addr_of!(MOUNTS).cast::<Mount>().add(env) };
        let mut host = HostStatVfs::default();
        if { disk_information((&mut host as *mut HostStatVfs).cast(), mount.conn) } != SMB_SUCCESS {
            return -1;
        }
        let out = { &mut *buf.cast::<StatVfs>() };
        *out = StatVfs::default();
        out.f_bsize = host.f_bsize;
        out.f_frsize = host.f_frsize;
        out.f_blocks = host.f_blocks as u64;
        out.f_bfree = host.f_bfree as u64;
        out.f_bavail = host.f_bavail as u64;
        out.f_files = host.f_files;
        out.f_ffree = host.f_ffree;
        out.f_favail = host.f_favail;
        out.f_fsid = host.f_fsid;
        out.f_flag = host.f_flag;
        out.f_namemax = host.f_namemax;
        0
    }
}

unsafe fn register_mount_devoptab(index: usize) -> bool {
    let mount = unsafe { &mut *core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>().add(index) };
    let mut d = empty_devoptab();
    d.name = mount.name.as_ptr().cast();
    d.struct_size = size_of::<SmbFileState>();
    d.open_r = Some(smb_dev_open);
    d.close_r = Some(smb_dev_close);
    d.write_r = Some(smb_dev_write);
    d.read_r = Some(smb_dev_read);
    d.seek_r = Some(smb_dev_seek);
    d.fstat_r = Some(smb_dev_fstat);
    d.stat_r = Some(smb_dev_stat);
    d.unlink_r = Some(smb_dev_unlink);
    d.chdir_r = Some(smb_dev_chdir);
    d.rename_r = Some(smb_dev_rename);
    d.mkdir_r = Some(smb_dev_mkdir);
    d.dir_state_size = size_of::<SmbDirState>();
    d.diropen_r = Some(smb_dev_diropen);
    d.dirreset_r = Some(smb_dev_dirreset);
    d.dirnext_r = Some(smb_dev_dirnext);
    d.dirclose_r = Some(smb_dev_dirclose);
    d.statvfs_r = Some(smb_dev_statvfs);
    let slot = unsafe {
        &mut *core::ptr::addr_of_mut!(DEVOPTABS)
            .cast::<MaybeUninit<Devoptab>>()
            .add(index)
    };
    slot.write(d);
    let rc = unsafe { ogc_sysbase::add_device(slot.as_ptr()) };
    if rc < 0 {
        return false;
    }
    mount.registered = true;
    true
}

fn now_ticks() -> u64 {
    unsafe { ogc_system_sys::sys_time() }
}

fn elapsed_ms_since(start: u64) -> u64 {
    now_ticks()
        .wrapping_sub(start)
        .saturating_mul(1_000)
        .saturating_div(WII_TIMER_CLOCK_HZ)
}

unsafe fn copy_c_string(ptr: *const c_char) -> Option<Vec<u8>> {
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(ptr) }.to_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.extend_from_slice(bytes);
    out.push(0);
    Some(out)
}

fn parse_ipv4(ptr: *const c_char) -> Option<u32> {
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(ptr) }.to_bytes();
    let mut parts = [0u32; 4];
    let mut part_idx = 0usize;
    let mut value = 0u32;
    let mut saw_digit = false;
    for &byte in bytes {
        match byte {
            b'0'..=b'9' => {
                saw_digit = true;
                value = value
                    .saturating_mul(10)
                    .saturating_add((byte - b'0') as u32);
                if value > 255 {
                    return None;
                }
            }
            b'.' => {
                if !saw_digit || part_idx >= 3 {
                    return None;
                }
                parts[part_idx] = value;
                part_idx += 1;
                value = 0;
                saw_digit = false;
            }
            _ => return None,
        }
    }
    if !saw_digit || part_idx != 3 {
        return None;
    }
    parts[3] = value;
    Some(
        ((parts[0] & 0xff) << 24)
            | ((parts[1] & 0xff) << 16)
            | ((parts[2] & 0xff) << 8)
            | (parts[3] & 0xff),
    )
}

#[cfg(target_arch = "powerpc")]
fn resolve_server_ip(ptr: *const c_char) -> Option<u32> {
    if let Some(ipv4) = parse_ipv4(ptr) {
        return Some(ipv4);
    }
    let host = unsafe { network::net_gethostbyname(ptr) };
    if host.is_null() {
        return None;
    }
    unsafe {
        let host = &*host;
        if host.h_addrtype != network::PF_INET as i16
            || host.h_length < 4
            || host.h_addr_list.is_null()
        {
            return None;
        }
        let first = *host.h_addr_list;
        if first.is_null() {
            return None;
        }
        let mut bytes = [0u8; 4];
        core::ptr::copy_nonoverlapping(first.cast::<u8>(), bytes.as_mut_ptr(), 4);
        Some(u32::from_be_bytes(bytes))
    }
}

#[cfg(not(target_arch = "powerpc"))]
fn resolve_server_ip(ptr: *const c_char) -> Option<u32> {
    parse_ipv4(ptr)
}

fn read_c_buf(buf: &[u8]) -> &[u8] {
    let len = buf.iter().position(|byte| *byte == 0).unwrap_or(buf.len());
    &buf[..len]
}

fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn read_i16_le(buf: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn read_u64_le(buf: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ])
}

fn apply_time_offset(value: u64, offset: i64) -> u64 {
    if offset >= 0 {
        value.saturating_sub(offset as u64)
    } else {
        value.saturating_add(offset.unsigned_abs())
    }
}

fn make_header(buf: &mut [u8], command: u8, session: &Session) {
    buf.fill(0);
    write_u32_le(buf, SMB_OFFSET_PROTO, SMB_PROTO);
    buf[SMB_OFFSET_CMD] = command;
    buf[SMB_OFFSET_FLAGS] = CIFS_FLAGS1;
    write_u16_le(
        buf,
        SMB_OFFSET_FLAGS2,
        if session.unicode {
            CIFS_FLAGS2_UNICODE
        } else {
            CIFS_FLAGS2
        },
    );
    write_u16_le(buf, SMB_OFFSET_TID, session.tid);
    write_u16_le(buf, SMB_OFFSET_PID, session.pid);
    write_u16_le(buf, SMB_OFFSET_UID, session.uid);
    write_u16_le(buf, SMB_OFFSET_MID, session.mid);
}

fn make_trans2_header(buf: &mut [u8], subcommand: u8) {
    buf[T2_WORD_CNT] = 15;
    write_u16_le(buf, T2_MAXPRM_CNT, 10);
    write_u16_le(buf, T2_MAXBUFFER, 16_384);
    buf[T2_SSETUP_CNT] = 1;
    write_u16_le(buf, T2_SUB_CMD, subcommand as u16);
}

fn session_slot(conn: SmbConn) -> Option<usize> {
    let idx = conn.checked_sub(1)? as usize;
    (idx < MAX_MOUNTS).then_some(idx)
}

unsafe fn session_mut(conn: SmbConn) -> Option<&'static mut Session> {
    let idx = session_slot(conn)?;
    unsafe { SESSIONS[idx].as_mut() }
}

fn alloc_session() -> Option<SmbConn> {
    let sessions = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(SESSIONS).cast::<Option<Session>>(),
            MAX_MOUNTS,
        )
    };
    for (idx, slot) in sessions.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(Session::new());
            return Some(idx as u32 + 1);
        }
    }
    None
}

fn close_socket(session: &mut Session) {
    if session.socket >= 0 {
        let _ = lwip::close(session.socket);
        session.socket = -1;
    }
}

fn free_session(conn: SmbConn) {
    if let Some(idx) = session_slot(conn) {
        let slot = unsafe { &mut SESSIONS[idx] };
        if let Some(session) = slot.as_mut() {
            close_socket(session);
        }
        *slot = None;
    }
    let files = core::ptr::addr_of_mut!(FILE_HANDLES).cast::<FileHandle>();
    for index in 0..MAX_OPEN_FILES {
        let file = unsafe { &mut *files.add(index) };
        if file.used && file.conn == conn {
            *file = FileHandle::new();
        }
    }
}

fn send_all(socket: i32, data: &[u8]) -> bool {
    let start = now_ticks();
    let mut sent = 0usize;
    while sent < data.len() {
        let rc = unsafe {
            lwip::send(
                socket,
                data[sent..].as_ptr().cast(),
                (data.len() - sent) as i32,
                0,
            )
        };
        if rc > 0 {
            sent += rc as usize;
            continue;
        }
        if rc == -EAGAIN {
            if elapsed_ms_since(start) > RECV_TIMEOUT_MS {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn recv_exact(socket: i32, data: &mut [u8], timeout_ms: u64) -> bool {
    let start = now_ticks();
    let mut read = 0usize;
    while read < data.len() {
        let rc = unsafe {
            lwip::recv(
                socket,
                data[read..].as_mut_ptr().cast(),
                (data.len() - read) as i32,
                0,
            )
        };
        if rc > 0 {
            read += rc as usize;
            continue;
        }
        if rc == -EAGAIN {
            if elapsed_ms_since(start) > timeout_ms {
                return false;
            }
            continue;
        }
        return false;
    }
    true
}

fn discard_exact(socket: i32, mut len: usize, timeout_ms: u64) -> bool {
    let mut discard = [0u8; 128];
    while len != 0 {
        let chunk = len.min(discard.len());
        if !recv_exact(socket, &mut discard[..chunk], timeout_ms) {
            return false;
        }
        len -= chunk;
    }
    true
}

fn minimum_response_words(command: u8) -> u8 {
    match command {
        SMB_NEG_PROTOCOL => 17,
        SMB_SETUP_ANDX | SMB_TREEC_ANDX => 3,
        SMB_TRANS2 => 10,
        SMB_OPEN_ANDX => 15,
        SMB_READ_ANDX => 12,
        SMB_WRITE_ANDX => 6,
        SMB_COM_QUERY_INFORMATION_DISK => 5,
        _ => 0,
    }
}

fn smb_check_with_len(socket: i32, command: u8, buf: &mut [u8], response_len: &mut usize) -> i32 {
    *response_len = 0;
    let mut nbt = [0u8; 4];
    let start = now_ticks();
    loop {
        if !recv_exact(socket, &mut nbt, RECV_TIMEOUT_MS) {
            return SMB_ERROR;
        }
        let read_len = ((nbt[1] as usize) << 16) | u16::from_be_bytes([nbt[2], nbt[3]]) as usize;
        if nbt[0] == NBT_SESSION_MSG {
            if read_len > buf.len() {
                return SMB_ERROR;
            }
            buf.fill(0);
            if !recv_exact(socket, &mut buf[..read_len], RECV_TIMEOUT_MS) {
                return SMB_ERROR;
            }
            if read_len < SMB_HEADER_SIZE + 3 {
                return SMB_ERROR;
            }
            if buf[SMB_HEADER_SIZE] < minimum_response_words(command) {
                return SMB_ERROR;
            }
            let words_end =
                match (SMB_HEADER_SIZE + 1).checked_add(buf[SMB_HEADER_SIZE] as usize * 2) {
                    Some(end) if end + 2 <= read_len => end,
                    _ => return SMB_ERROR,
                };
            let byte_count = read_u16_le(buf, words_end) as usize;
            if words_end + 2 + byte_count > read_len {
                return SMB_ERROR;
            }
            *response_len = read_len;
            break;
        }
        if read_len != 0 && !discard_exact(socket, read_len, RECV_TIMEOUT_MS) {
            return SMB_ERROR;
        }
        if elapsed_ms_since(start) > RECV_TIMEOUT_MS * 2 {
            return SMB_ERROR;
        }
    }

    if read_u32_le(buf, SMB_OFFSET_PROTO) != SMB_PROTO {
        return SMB_BAD_PROTOCOL;
    }
    if buf[SMB_OFFSET_CMD] != command {
        return SMB_BAD_COMMAND;
    }
    if read_u32_le(buf, SMB_OFFSET_NTSTATUS) != 0 {
        return SMB_ERROR;
    }
    SMB_SUCCESS
}

fn smb_check(socket: i32, command: u8, buf: &mut [u8]) -> i32 {
    let mut response_len = 0usize;
    smb_check_with_len(socket, command, buf, &mut response_len)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Utf8ToUtf16Error {
    InvalidInput,
    DestinationTooSmall,
}

fn decode_utf8_scalar(src: &[u8]) -> Option<(u32, usize)> {
    let first = *src.first()?;
    let width = match first {
        0x01..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return None,
    };
    let bytes = src.get(..width)?;
    let text = core::str::from_utf8(bytes).ok()?;
    Some((text.chars().next()? as u32, width))
}

fn utf8_to_utf16le(dst: &mut [u8], src: &[u8]) -> Result<usize, Utf8ToUtf16Error> {
    if dst.len() < 2 {
        return Err(Utf8ToUtf16Error::DestinationTooSmall);
    }

    // C passes SMB_MAXPATH - 2 as the conversion limit, then accounts for
    // the two-byte terminator separately. Keep that logical limit while also
    // reserving space inside the Rust destination slice.
    let max_data_bytes = (SMB_MAXPATH - 2).min(dst.len() - 2);
    let mut input = 0usize;
    let mut output = 0usize;

    while input < src.len() && src[input] != 0 {
        if output + 2 > max_data_bytes {
            break;
        }
        let Some((codepoint, width)) = decode_utf8_scalar(&src[input..]) else {
            dst[output..output + 2].fill(0);
            return Err(Utf8ToUtf16Error::InvalidInput);
        };

        // libogc stores the low 16 bits of wchar_t as one UTF-16 unit. This
        // also preserves its behavior for supplementary code points.
        let unit = (codepoint as u16).to_le_bytes();
        dst[output..output + 2].copy_from_slice(&unit);
        input += width;
        output += 2;
    }

    dst[output..output + 2].fill(0);
    Ok(output + 2)
}

macro_rules! utf8_to_utf16le_or_error {
    ($dst:expr_2021, $src:expr_2021) => {
        utf8_to_utf16le_or_error!($dst, $src, SMB_ERROR)
    };
    ($dst:expr_2021, $src:expr_2021, $error:expr_2021) => {
        match utf8_to_utf16le($dst, $src) {
            Ok(written) => written,
            Err(_) => return $error,
        }
    };
}

#[cfg(all(test, target_arch = "powerpc"))]
mod utf8_conversion_tests {
    use super::{Utf8ToUtf16Error, utf8_to_utf16le};

    #[test]
    fn encodes_ascii_bmp_and_supplementary_codepoints_like_c() {
        let mut ascii = [0xff; 8];
        assert_eq!(utf8_to_utf16le(&mut ascii, b"A"), Ok(4));
        assert_eq!(&ascii[..4], [b'A', 0, 0, 0]);

        let mut bmp = [0xff; 8];
        assert_eq!(utf8_to_utf16le(&mut bmp, "é€".as_bytes()), Ok(6));
        assert_eq!(&bmp[..6], [0xe9, 0, 0xac, 0x20, 0, 0]);

        let mut supplementary = [0xff; 8];
        assert_eq!(utf8_to_utf16le(&mut supplementary, "😀".as_bytes()), Ok(4));
        assert_eq!(&supplementary[..4], [0, 0xf6, 0, 0]);
    }

    #[test]
    fn invalid_and_incomplete_sequences_write_a_zero_unit_and_fail() {
        for source in [&b"A\xff"[..], &[b'A', 0xe2, 0x82][..]] {
            let mut output = [0xff; 8];
            assert_eq!(
                utf8_to_utf16le(&mut output, source),
                Err(Utf8ToUtf16Error::InvalidInput)
            );
            assert_eq!(&output[..4], [b'A', 0, 0, 0]);
        }
    }

    #[test]
    fn truncates_at_a_complete_utf16_unit_and_keeps_terminator() {
        let mut output = [0xff; 8];
        assert_eq!(utf8_to_utf16le(&mut output, b"ABCD"), Ok(8));
        assert_eq!(output, [b'A', 0, b'B', 0, b'C', 0, 0, 0]);

        let source = [b'A'; crate::SMB_MAXPATH + 4];
        let mut c_limited = [0xff; crate::SMB_MAXPATH + 8];
        assert_eq!(
            utf8_to_utf16le(&mut c_limited, &source),
            Ok(crate::SMB_MAXPATH)
        );
        assert_eq!(
            &c_limited[crate::SMB_MAXPATH - 4..crate::SMB_MAXPATH],
            [b'A', 0, 0, 0]
        );
        assert_eq!(c_limited[crate::SMB_MAXPATH], 0xff);

        let mut too_small = [0xff; 1];
        assert_eq!(
            utf8_to_utf16le(&mut too_small, b"A"),
            Err(Utf8ToUtf16Error::DestinationTooSmall)
        );
    }
}

fn uppercase_ascii_in_place(buf: &mut [u8]) {
    for byte in buf.iter_mut().take_while(|byte| **byte != 0) {
        *byte = byte.to_ascii_uppercase();
    }
}

fn prefixed_path(path: *const c_char, out: &mut [u8]) -> Option<usize> {
    if path.is_null() || out.len() < 2 {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
    let needs_prefix = bytes.first().copied() != Some(b'\\');
    let total = bytes.len() + usize::from(needs_prefix);
    if total >= out.len() {
        return None;
    }
    let mut pos = 0usize;
    if needs_prefix {
        out[pos] = b'\\';
        pos += 1;
    }
    out[pos..pos + bytes.len()].copy_from_slice(bytes);
    pos += bytes.len();
    out[pos] = 0;
    Some(pos)
}

fn fill_dir_entry(
    session: &Session,
    packet: &[u8],
    mut pos: usize,
    sid: u16,
    sdir: *mut SmbDirEntry,
    unicode: bool,
) -> i32 {
    if sdir.is_null() || pos + 70 > packet.len() {
        return SMB_ERROR;
    }
    unsafe {
        (*sdir).sid = sid;
        pos += 4;
        pos += 4;
        (*sdir).ctime = apply_time_offset(read_u64_le(packet, pos), session.time_offset);
        pos += 8;
        (*sdir).atime = apply_time_offset(read_u64_le(packet, pos), session.time_offset);
        pos += 8;
        (*sdir).mtime = apply_time_offset(read_u64_le(packet, pos), session.time_offset);
        pos += 8;
        pos += 8;
        (*sdir).size = read_u64_le(packet, pos);
        pos += 8;
        pos += 8;
        (*sdir).attributes = read_u32_le(packet, pos);
        pos += 4;
        let name_len = read_u32_le(packet, pos) as usize;
        pos += 34;
        (*sdir).name = [0; 768];
        if pos > packet.len() {
            return SMB_ERROR;
        }
        if unicode {
            let available = packet.len().saturating_sub(pos).min(name_len);
            crate::utf16le_to_utf8(&mut (*sdir).name, &packet[pos..pos + available]);
        } else {
            let available = packet.len().saturating_sub(pos).min(name_len);
            let copy_len = available.min((*sdir).name.len().saturating_sub(1));
            for idx in 0..copy_len {
                let byte = packet[pos + idx];
                if byte == 0 {
                    break;
                }
                (*sdir).name[idx] = byte as c_char;
            }
        }
    }
    SMB_SUCCESS
}

fn alloc_file_handle(conn: SmbConn, sfid: u16) -> SmbFile {
    let files = core::ptr::addr_of_mut!(FILE_HANDLES).cast::<FileHandle>();
    for index in 0..MAX_OPEN_FILES {
        let file = unsafe { &mut *files.add(index) };
        if !file.used {
            *file = FileHandle {
                used: true,
                conn,
                sfid,
            };
            return (file as *mut FileHandle).cast::<c_void>();
        }
    }
    core::ptr::null_mut()
}

fn file_handle_mut(sfid: SmbFile) -> Option<&'static mut FileHandle> {
    if sfid.is_null() {
        return None;
    }
    let handle = unsafe { &mut *sfid.cast::<FileHandle>() };
    handle.used.then_some(handle)
}

fn negotiate_protocol(session: &mut Session, packet: &mut [u8]) -> i32 {
    make_header(packet, SMB_NEG_PROTOCOL, session);
    let mut pos = SMB_HEADER_SIZE + 3;
    packet[SMB_HEADER_SIZE] = 0;
    write_u16_le(packet, SMB_HEADER_SIZE + 1, 11);
    packet[pos] = 0x02;
    pos += 1;
    packet[pos..pos + 10].copy_from_slice(b"NT LM 0.12");
    pos += 10;
    packet[pos] = 0;
    pos += 1;

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    let len = pos as u16;
    nbt[2..4].copy_from_slice(&len.to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_NEG_PROTOCOL, packet);
    if rc != SMB_SUCCESS {
        return rc;
    }

    let mut pos = SMB_HEADER_SIZE;
    if packet[pos] != 17 {
        return SMB_ERROR;
    }
    pos += 1;
    if read_u16_le(packet, pos) != 0 {
        return SMB_ERROR;
    }
    pos += 2;
    pos += 1;
    session.max_mpx = read_u16_le(packet, pos);
    pos += 2;
    session.max_vcs = read_u16_le(packet, pos);
    pos += 2;
    let server_max_buffer = read_u32_le(packet, pos);
    session.max_buffer = server_max_buffer.min(SMB_MAX_TRANSMIT_SIZE);
    pos += 4;
    pos += 4;
    session.skey = read_u32_le(packet, pos);
    pos += 4;
    let server_cap = read_u32_le(packet, pos);
    pos += 4;
    pos += 8;
    session.time_offset = read_i16_le(packet, pos) as i64 * 600_000_000_i64;
    pos += 2;

    let key_len = packet[pos];
    pos += 1;
    match key_len {
        0 => session.challenge_used = false,
        8 => session.challenge_used = true,
        _ => return SMB_BAD_KEYLEN,
    }
    let _byte_count = read_u16_le(packet, pos);
    pos += 2;
    if session.challenge_used {
        session.challenge.copy_from_slice(&packet[pos..pos + 8]);
        pos += 8;
    }
    if (server_cap & CAP_UNICODE) != 0 {
        session.capabilities |= CAP_UNICODE;
        session.unicode = true;
        let mut domain_len = 0usize;
        while pos + domain_len + 1 < packet.len()
            && packet[pos + domain_len] != 0
            && packet[pos + domain_len + 1] == 0
            && domain_len / 2 < session.p_domain.len()
        {
            session.p_domain[domain_len / 2] = packet[pos + domain_len];
            domain_len += 2;
        }
    }
    SMB_SUCCESS
}

fn setup_andx(session: &mut Session, packet: &mut [u8]) -> i32 {
    make_header(packet, SMB_SETUP_ANDX, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 13;
    pos += 1;
    packet[pos] = 0xff;
    pos += 1;
    packet[pos] = 0;
    pos += 1;
    pos += 2;
    write_u16_le(packet, pos, session.max_buffer as u16);
    pos += 2;
    write_u16_le(packet, pos, session.max_mpx);
    pos += 2;
    write_u16_le(packet, pos, session.max_vcs);
    pos += 2;
    write_u32_le(packet, pos, session.skey);
    pos += 4;
    write_u16_le(packet, pos, 24);
    pos += 2;
    write_u16_le(packet, pos, 24);
    pos += 2;
    write_u32_le(packet, pos, 0);
    pos += 4;
    write_u32_le(packet, pos, session.capabilities);
    pos += 4;
    let bcpos = pos;
    pos += 2;

    if pos + 24 + 1 > packet.len() {
        return SMB_ERROR;
    }
    packet[pos..pos + 24].fill(0);
    pos += 24;
    let nt_response = if session.challenge_used {
        ntlm::ntlm_response(read_c_buf(&session.password), &session.challenge)
    } else {
        [0u8; 24]
    };
    packet[pos..pos + nt_response.len()].copy_from_slice(&nt_response);
    pos += 24;
    packet[pos] = 0;
    pos += 1;

    let mut account = session.user.clone();
    uppercase_ascii_in_place(&mut account);
    let account_bytes = read_c_buf(&account);
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], account_bytes);
    } else {
        if pos + account_bytes.len() + 1 > packet.len() {
            return SMB_ERROR;
        }
        packet[pos..pos + account_bytes.len()].copy_from_slice(account_bytes);
        pos += account_bytes.len();
        packet[pos] = 0;
        pos += 1;
    }

    let domain_bytes = read_c_buf(&session.p_domain);
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], domain_bytes);
    } else {
        if pos + domain_bytes.len() + 1 > packet.len() {
            return SMB_ERROR;
        }
        packet[pos..pos + domain_bytes.len()].copy_from_slice(domain_bytes);
        pos += domain_bytes.len();
        packet[pos] = 0;
        pos += 1;
    }

    for label in [b"Unix (libOGC)".as_slice(), b"Nintendo Wii".as_slice()] {
        if session.unicode {
            pos += utf8_to_utf16le_or_error!(&mut packet[pos..], label);
        } else {
            if pos + label.len() + 1 > packet.len() {
                return SMB_ERROR;
            }
            packet[pos..pos + label.len()].copy_from_slice(label);
            pos += label.len();
            packet[pos] = 0;
            pos += 1;
        }
    }

    write_u16_le(packet, bcpos, (pos - bcpos - 2) as u16);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_SETUP_ANDX, packet);
    if rc == SMB_SUCCESS {
        session.uid = read_u16_le(packet, SMB_OFFSET_UID);
    }
    rc
}

fn tree_andx(session: &mut Session, packet: &mut [u8]) -> i32 {
    make_header(packet, SMB_TREEC_ANDX, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 4;
    pos += 1;
    packet[pos] = 0xff;
    pos += 1;
    packet[pos] = 0;
    pos += 1;
    pos += 2;
    pos += 2;
    write_u16_le(packet, pos, 1);
    pos += 2;
    let bcpos = pos;
    pos += 2;
    packet[pos] = 0;
    pos += 1;

    let mut path = [0u8; SMB_MAXPATH];
    let mut path_len = 0usize;
    path[path_len] = b'\\';
    path[path_len + 1] = b'\\';
    path_len += 2;
    for bytes in [
        read_c_buf(&session.server),
        b"\\".as_slice(),
        read_c_buf(&session.share),
    ] {
        if path_len + bytes.len() > path.len() {
            return SMB_ERROR;
        }
        path[path_len..path_len + bytes.len()].copy_from_slice(bytes);
        path_len += bytes.len();
    }
    for byte in path.iter_mut().take(path_len) {
        *byte = byte.to_ascii_uppercase();
    }

    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &path[..path_len]);
    } else {
        if pos + path_len + 7 > packet.len() {
            return SMB_ERROR;
        }
        packet[pos..pos + path_len].copy_from_slice(&path[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
    }
    if pos + 6 > packet.len() {
        return SMB_ERROR;
    }
    packet[pos..pos + 5].copy_from_slice(b"?????");
    pos += 5;
    packet[pos] = 0;
    pos += 1;
    write_u16_le(packet, bcpos, (pos - bcpos - 2) as u16);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_TREEC_ANDX, packet);
    if rc == SMB_SUCCESS {
        session.tid = read_u16_le(packet, SMB_OFFSET_TID);
        session.conn_valid = true;
    }
    rc
}

fn l1_encode(dst: &mut [u8], name: &[u8], pad: u8, sfx: u8) {
    let mut src_idx = 0usize;
    let mut dst_idx = 0usize;
    while src_idx < name.len() && src_idx < 15 {
        let byte = name[src_idx].to_ascii_uppercase();
        src_idx += 1;
        dst[dst_idx] = b'A' + ((byte & 0xf0) >> 4);
        dst[dst_idx + 1] = b'A' + (byte & 0x0f);
        dst_idx += 2;
    }
    let pad_hi = b'A' + ((pad & 0xf0) >> 4);
    let pad_lo = b'A' + (pad & 0x0f);
    while dst_idx < 30 {
        dst[dst_idx] = pad_hi;
        dst[dst_idx + 1] = pad_lo;
        dst_idx += 2;
    }
    dst[30] = b'A' + ((sfx & 0xf0) >> 4);
    dst[31] = b'A' + (sfx & 0x0f);
    dst[32] = 0;
}

fn l2_encode(dst: &mut [u8], name: &[u8], pad: u8, sfx: u8) -> usize {
    dst[0] = 0x20;
    l1_encode(&mut dst[1..34], name, pad, sfx);
    dst[34] = 0;
    35
}

fn request_nbt_session(session: &mut Session) -> bool {
    let mut called = [0u8; 35];
    let mut calling = [0u8; 35];
    let mut packet = [0u8; 72];
    let _ = l2_encode(&mut called, b"*SMBSERVER", 0x20, 0x20);
    let _ = l2_encode(&mut calling, b"SMBCLIENT", 0x20, 0x00);
    packet[0] = SESS_REQ;
    packet[3] = 68;
    packet[4..38].copy_from_slice(&called[..34]);
    packet[38..72].copy_from_slice(&calling[..34]);
    if !send_all(session.socket, &packet) {
        return false;
    }
    let mut response = [0u8; 128];
    if !recv_exact(session.socket, &mut response, RECV_TIMEOUT_MS) {
        return false;
    }
    response[0] == SESS_POS_RESP
}

fn do_netconnect(session: &mut Session) -> bool {
    close_socket(session);
    let socket = lwip::socket(lwip::AF_INET, lwip::SOCK_STREAM, lwip::IPPROTO_IP);
    if socket < 0 {
        return false;
    }
    let one = 1i32;
    let _ = unsafe {
        lwip::set_socket_option(
            socket,
            lwip::IPPROTO_TCP,
            TCP_NODELAY,
            (&one as *const i32).cast(),
            size_of::<i32>() as lwip::SockLenT,
        )
    };
    let mut nonblock = 1u32;
    if unsafe { lwip::ioctl(socket, lwip::FIONBIO, (&mut nonblock as *mut u32).cast()) } < 0 {
        let _ = lwip::close(socket);
        return false;
    }

    let mut addr = lwip::SockAddrIn {
        sin_len: size_of::<lwip::SockAddrIn>() as u8,
        sin_family: lwip::AF_INET as u8,
        sin_port: session.port_be,
        sin_addr: lwip::InAddr {
            s_addr: session.server_ip,
        },
        sin_zero: [0; 8],
    };
    let start = now_ticks();
    loop {
        let rc = unsafe {
            lwip::connect(
                socket,
                (&mut addr as *mut lwip::SockAddrIn).cast(),
                size_of::<lwip::SockAddrIn>() as lwip::SockLenT,
            )
        };
        if rc == 0 || rc == -EISCONN {
            session.socket = socket;
            return true;
        }
        if rc != -EINPROGRESS && rc != -EALREADY && rc != -EAGAIN {
            break;
        }
        if elapsed_ms_since(start) > CONN_TIMEOUT_MS {
            break;
        }
    }
    let _ = lwip::close(socket);
    false
}

fn do_smbconnect(session: &mut Session) -> bool {
    let mut packet = [0u8; SMB_BUF_BYTES];
    if negotiate_protocol(session, &mut packet) != SMB_SUCCESS {
        return false;
    }
    if setup_andx(session, &mut packet) != SMB_SUCCESS {
        return false;
    }
    tree_andx(session, &mut packet) == SMB_SUCCESS
}

fn connect_session(session: &mut Session) -> bool {
    session.port_be = 445u16.to_be();
    if do_netconnect(session) && do_smbconnect(session) {
        return true;
    }
    session.port_be = 139u16.to_be();
    if do_netconnect(session) && request_nbt_session(session) && do_smbconnect(session) {
        return true;
    }
    close_socket(session);
    false
}

pub unsafe fn smb_init_device(
    name: *const c_char,
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> bool {
    if name.is_null() {
        return false;
    }
    let name_bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    if !valid_device_name(name_bytes) {
        return false;
    }
    let Some(devname) = device_name_for_find_device(name_bytes) else {
        return false;
    };
    let devname = unsafe { CStr::from_ptr(devname.as_ptr()) };
    if ogc_sysbase::find_device(devname) >= 0 {
        return false;
    }
    let mounts = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(MOUNTS).cast::<Mount>(), MAX_MOUNTS)
    };
    if mount_name_exists(mounts, name_bytes) {
        return false;
    }
    let mut conn = 0;
    if unsafe { connect(&mut conn, user, password, share, ip) } != SMB_SUCCESS {
        return false;
    }
    let Some(record) = mount_record(name_bytes, conn) else {
        close_connection(conn);
        return false;
    };
    let mounts = core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>();
    for index in 0..MAX_MOUNTS {
        let mount = unsafe { &mut *mounts.add(index) };
        if !mount.used {
            *mount = record;
            if unsafe { register_mount_devoptab(index) } {
                return true;
            }
            close_connection(conn);
            *mount = Mount::new();
            return false;
        }
    }
    close_connection(conn);
    false
}

pub unsafe fn smb_init(
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> bool {
    unsafe { smb_init_device(c"smb".as_ptr(), user, password, share, ip) }
}

pub unsafe fn smb_close(name: *const c_char) {
    if name.is_null() {
        return;
    }
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    let lookup = bytes.strip_suffix(b":").unwrap_or(bytes);
    let mounts = core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>();
    for index in 0..MAX_MOUNTS {
        let mount = unsafe { &mut *mounts.add(index) };
        if mount.used && read_c_buf(&mount.name) == lookup {
            if mount.registered {
                let Some(devname) = device_name_for_find_device(read_c_buf(&mount.name)) else {
                    return;
                };
                let devname = unsafe { CStr::from_ptr(devname.as_ptr()) };
                if ogc_sysbase::remove_device(devname) < 0 {
                    return;
                }
                mount.registered = false;
            }
            close_connection(mount.conn);
            *mount = Mount::new();
            return;
        }
    }
}

pub unsafe fn smb_check_connection(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    let lookup = normalized_lookup_name(bytes);
    let mounts = core::ptr::addr_of_mut!(MOUNTS).cast::<Mount>();
    for index in 0..MAX_MOUNTS {
        let mount = unsafe { &mut *mounts.add(index) };
        if mount.used && read_c_buf(&mount.name) == lookup {
            return unsafe { reconnect(&mut mount.conn, 1) } == SMB_SUCCESS;
        }
    }
    false
}

pub fn smb_set_search_flags(flags: u16) {
    SMB_SEARCH_FLAGS.store(flags, Ordering::Release);
}

pub unsafe fn connect(
    smbhndl: *mut SmbConn,
    user: *const c_char,
    password: *const c_char,
    share: *const c_char,
    ip: *const c_char,
) -> i32 {
    if smbhndl.is_null() {
        return SMB_BAD_LOGINDATA;
    }
    let Some(user) = (unsafe { copy_c_string(user) }) else {
        return SMB_BAD_LOGINDATA;
    };
    let Some(password) = (unsafe { copy_c_string(password) }) else {
        return SMB_BAD_LOGINDATA;
    };
    let Some(share) = (unsafe { copy_c_string(share) }) else {
        return SMB_BAD_LOGINDATA;
    };
    let Some(server) = (unsafe { copy_c_string(ip) }) else {
        return SMB_BAD_LOGINDATA;
    };
    let Some(server_ip) = resolve_server_ip(ip) else {
        return SMB_BAD_LOGINDATA;
    };
    let conn = match alloc_session() {
        Some(conn) => conn,
        None => return SMB_ERROR,
    };
    let Some(session) = (unsafe { session_mut(conn) }) else {
        free_session(conn);
        return SMB_ERROR;
    };
    session.user = user;
    session.password = password;
    session.share = share;
    session.server = server;
    session.server_ip = server_ip;
    if !connect_session(session) {
        free_session(conn);
        return SMB_ERROR;
    }
    unsafe {
        *smbhndl = conn;
    }
    SMB_SUCCESS
}

pub fn close_connection(smbhndl: SmbConn) {
    free_session(smbhndl);
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::{Mount, device_name_for_find_device, mount_name_exists, mount_record};

    #[test]
    fn find_device_name_adds_c_colon_suffix_without_truncation() {
        let short = device_name_for_find_device(b"smb").unwrap();
        assert_eq!(&short[..5], b"smb:\0");
        let maximum = device_name_for_find_device(b"12345678").unwrap();
        assert_eq!(&maximum[..10], b"12345678:\0");
        assert!(device_name_for_find_device(b"123456789").is_none());
    }

    #[test]
    fn default_mount_record_retains_connection() {
        let mount = mount_record(b"smb", 17).expect("valid C default device name");
        assert!(mount.used);
        assert_eq!(&mount.name[..3], b"smb");
        assert_eq!(mount.conn, 17);
    }

    #[test]
    fn mount_record_rejects_names_longer_than_c_limit() {
        assert!(mount_record(b"123456789", 17).is_none());
    }

    #[test]
    fn mount_record_accepts_short_and_maximum_length_names() {
        assert!(mount_record(b"x", 17).is_some());
        assert!(mount_record(b"12345678", 17).is_some());
    }

    #[test]
    fn mount_record_rejects_empty_names() {
        assert!(mount_record(b"", 17).is_none());
    }

    #[test]
    fn connection_lookup_stops_at_first_colon_like_c() {
        assert_eq!(super::normalized_lookup_name(b"smb"), b"smb");
        assert_eq!(super::normalized_lookup_name(b"smb:"), b"smb");
        assert_eq!(super::normalized_lookup_name(b"smb:/path"), b"smb");
    }

    #[test]
    fn duplicate_mount_names_are_detected_only_for_used_mounts() {
        let mut mounts = [Mount::new(); super::MAX_MOUNTS];
        mounts[0] = mount_record(b"smb", 17).unwrap();
        assert!(mount_name_exists(&mounts, b"smb"));
        assert!(!mount_name_exists(&mounts, b"other"));
        mounts[0] = Mount::new();
        assert!(!mount_name_exists(&mounts, b"smb"));
    }
}

pub unsafe fn reconnect(smbhndl: *mut SmbConn, test_conn: Bool) -> i32 {
    if smbhndl.is_null() {
        return SMB_ERROR;
    }
    let conn = unsafe { *smbhndl };
    let Some(session) = (unsafe { session_mut(conn) }) else {
        return SMB_ERROR;
    };
    if session.conn_valid && test_conn != 0 {
        return SMB_SUCCESS;
    }
    session.conn_valid = false;
    close_socket(session);
    if connect_session(session) {
        SMB_SUCCESS
    } else {
        SMB_ERROR
    }
}

pub unsafe fn path_info(
    _filename: *const c_char,
    _sdir: *mut SmbDirEntry,
    mut _smbhndl: SmbConn,
) -> i32 {
    if _filename.is_null() || _sdir.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };

    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut realfile = [0u8; 512];
    let Some(path_len) = prefixed_path(_filename, &mut realfile) else {
        return SMB_ERROR;
    };

    make_header(&mut packet, SMB_TRANS2, session);
    make_trans2_header(&mut packet, SMB_QUERY_PATH_INFO);

    let bpos = T2_BYTE_CNT + 2;
    let mut pos = bpos;
    pos += 3;
    write_u16_le(&mut packet, pos, SMB_QUERY_FILE_ALL_INFO);
    pos += 2;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;

    let encoded_len = if session.unicode {
        let written = utf8_to_utf16le_or_error!(&mut packet[pos..], &realfile[..path_len]);
        pos += written;
        written / 2
    } else {
        packet[pos..pos + path_len].copy_from_slice(&realfile[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
        path_len
    };

    write_u16_le(&mut packet, T2_PRM_CNT, (7 + encoded_len) as u16);
    write_u16_le(&mut packet, T2_SPRM_CNT, (7 + encoded_len) as u16);
    write_u16_le(&mut packet, T2_SPRM_OFS, 68);
    write_u16_le(&mut packet, T2_BYTE_CNT, (pos - bpos) as u16);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }

    let mut response_len = 0usize;
    let rc = smb_check_with_len(session.socket, SMB_TRANS2, &mut packet, &mut response_len);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
        return rc;
    }

    let Some(mut pos) = (read_u16_le(&packet, SMB_HEADER_SIZE + 9) as usize).checked_add(4) else {
        return SMB_ERROR;
    };
    if pos.checked_add(56).is_none_or(|end| end > response_len) {
        return SMB_ERROR;
    }
    unsafe {
        (*_sdir).ctime = apply_time_offset(read_u64_le(&packet, pos), session.time_offset);
        pos += 8;
        (*_sdir).atime = apply_time_offset(read_u64_le(&packet, pos), session.time_offset);
        pos += 8;
        (*_sdir).mtime = apply_time_offset(read_u64_le(&packet, pos), session.time_offset);
        pos += 8;
        pos += 8;
        (*_sdir).attributes = read_u32_le(&packet, pos);
        pos += 4;
        pos += 4;
        pos += 8;
        (*_sdir).size = read_u64_le(&packet, pos);
        (*_sdir).sid = 0;
        (*_sdir).name = [0; 768];
        let copy_len = path_len.min((*_sdir).name.len() - 1);
        for (idx, byte) in realfile[..copy_len].iter().enumerate() {
            (*_sdir).name[idx] = *byte as c_char;
        }
    }
    SMB_SUCCESS
}

pub unsafe fn find_first(
    _filename: *const c_char,
    _flags: u16,
    _sdir: *mut SmbDirEntry,
    mut _smbhndl: SmbConn,
) -> i32 {
    if _filename.is_null() || _sdir.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_TRANS2, session);
    make_trans2_header(&mut packet, SMB_FIND_FIRST2);

    let bpos = T2_BYTE_CNT + 2;
    let mut pos = bpos;
    pos += 3;
    write_u16_le(&mut packet, pos, _flags);
    pos += 2;
    write_u16_le(&mut packet, pos, 1);
    pos += 2;
    write_u16_le(&mut packet, pos, 6);
    pos += 2;
    write_u16_le(&mut packet, pos, 260);
    pos += 2;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;

    let filename = unsafe { CStr::from_ptr(_filename) }.to_bytes();
    let name_units = if session.unicode {
        let written = utf8_to_utf16le_or_error!(&mut packet[pos..], filename);
        pos += written;
        written / 2
    } else {
        packet[pos..pos + filename.len()].copy_from_slice(filename);
        pos += filename.len();
        packet[pos] = 0;
        pos += 1;
        filename.len()
    };

    write_u16_le(&mut packet, T2_PRM_CNT, (13 + name_units) as u16);
    write_u16_le(&mut packet, T2_SPRM_CNT, (13 + name_units) as u16);
    write_u16_le(&mut packet, T2_SPRM_OFS, 68);
    write_u16_le(&mut packet, T2_BYTE_CNT, (pos - bpos) as u16);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let mut response_len = 0usize;
    if smb_check_with_len(session.socket, SMB_TRANS2, &mut packet, &mut response_len) != SMB_SUCCESS
    {
        session.conn_valid = false;
        return SMB_ERROR;
    }

    let mut parse = read_u16_le(&packet, SMB_HEADER_SIZE + 9) as usize;
    if parse.checked_add(12).is_none_or(|end| end > response_len) {
        return SMB_ERROR;
    }
    let sid = read_u16_le(&packet, parse);
    parse += 2;
    let count = read_u16_le(&packet, parse);
    parse += 2;
    session.search_eos = read_u16_le(&packet, parse) != 0;
    parse += 2;
    parse += 2;
    parse += 2;
    parse += 2;
    if count == 0 {
        unsafe {
            (*_sdir).sid = sid;
        }
        return SMB_ERROR;
    }
    fill_dir_entry(
        session,
        &packet[..response_len],
        parse,
        sid,
        _sdir,
        session.unicode,
    )
}

pub unsafe fn find_next(_sdir: *mut SmbDirEntry, _smbhndl: SmbConn) -> i32 {
    if _sdir.is_null() {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let sid = unsafe { (*_sdir).sid };
    if sid == 0 || session.search_eos {
        return SMB_ERROR;
    }

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_TRANS2, session);
    make_trans2_header(&mut packet, SMB_FIND_NEXT2);

    let bpos = T2_BYTE_CNT + 2;
    let mut pos = bpos;
    pos += 3;
    write_u16_le(&mut packet, pos, sid);
    pos += 2;
    write_u16_le(&mut packet, pos, 1);
    pos += 2;
    write_u16_le(&mut packet, pos, 260);
    pos += 2;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;
    write_u16_le(&mut packet, pos, 12);
    pos += 2;
    packet[pos] = 0;
    pos += 1;
    if session.unicode {
        pos += 1;
    }

    write_u16_le(
        &mut packet,
        T2_PRM_CNT,
        (13 + usize::from(session.unicode)) as u16,
    );
    write_u16_le(
        &mut packet,
        T2_SPRM_CNT,
        (13 + usize::from(session.unicode)) as u16,
    );
    write_u16_le(&mut packet, T2_SPRM_OFS, 68);
    write_u16_le(&mut packet, T2_BYTE_CNT, (pos - bpos) as u16);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let mut response_len = 0usize;
    if smb_check_with_len(session.socket, SMB_TRANS2, &mut packet, &mut response_len) != SMB_SUCCESS
    {
        session.conn_valid = false;
        return SMB_ERROR;
    }

    let mut parse = read_u16_le(&packet, SMB_HEADER_SIZE + 9) as usize;
    if parse.checked_add(8).is_none_or(|end| end > response_len) {
        return SMB_ERROR;
    }
    let count = read_u16_le(&packet, parse);
    parse += 2;
    session.search_eos = read_u16_le(&packet, parse) != 0;
    parse += 2;
    parse += 2;
    parse += 2;
    if count == 0 {
        return SMB_ERROR;
    }
    fill_dir_entry(
        session,
        &packet[..response_len],
        parse,
        sid,
        _sdir,
        session.unicode,
    )
}

pub fn find_close(_sdir: *mut SmbDirEntry, _smbhndl: SmbConn) -> i32 {
    if _sdir.is_null() {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let sid = unsafe { (*_sdir).sid };
    if sid == 0 {
        return SMB_ERROR;
    }

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_FIND_CLOSE2, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 1;
    pos += 1;
    write_u16_le(&mut packet, pos, sid);
    pos += 2;
    pos += 2;

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_FIND_CLOSE2, &mut packet);
    if rc == SMB_SUCCESS {
        session.search_eos = true;
        unsafe {
            (*_sdir).sid = 0;
        }
    } else {
        session.conn_valid = false;
    }
    rc
}

pub unsafe fn open_file(
    _filename: *const c_char,
    _access: u16,
    _creation: u16,
    mut _smbhndl: SmbConn,
) -> SmbFile {
    if _filename.is_null() {
        return core::ptr::null_mut();
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return core::ptr::null_mut();
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return core::ptr::null_mut();
    };

    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut realfile = [0u8; 512];
    let Some(path_len) = prefixed_path(_filename, &mut realfile) else {
        return core::ptr::null_mut();
    };

    make_header(&mut packet, SMB_OPEN_ANDX, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 15;
    pos += 1;
    packet[pos] = 0xff;
    pos += 1;
    packet[pos] = 0;
    pos += 1;
    pos += 2;
    pos += 2;
    write_u16_le(&mut packet, pos, _access);
    pos += 2;
    write_u16_le(&mut packet, pos, 0x0006);
    pos += 2;
    pos += 2;
    pos += 4;
    write_u16_le(&mut packet, pos, _creation);
    pos += 2;
    pos += 4;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;
    let byte_count_pos = pos;
    pos += 2;
    packet[pos] = 0x04;
    pos += 1;

    if session.unicode {
        pos += utf8_to_utf16le_or_error!(
            &mut packet[pos..],
            &realfile[..path_len],
            core::ptr::null_mut()
        );
    } else {
        packet[pos..pos + path_len].copy_from_slice(&realfile[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
    }
    write_u16_le(
        &mut packet,
        byte_count_pos,
        (pos - byte_count_pos - 2) as u16,
    );

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return core::ptr::null_mut();
    }

    let rc = smb_check(session.socket, SMB_OPEN_ANDX, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
        return core::ptr::null_mut();
    }

    let sfid = read_u16_le(&packet, SMB_HEADER_SIZE + 5);
    alloc_file_handle(_smbhndl, sfid)
}

pub fn close_file(_sfid: SmbFile) {
    let Some(handle) = file_handle_mut(_sfid) else {
        return;
    };
    let Some(session) = (unsafe { session_mut(handle.conn) }) else {
        *handle = FileHandle::new();
        return;
    };

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_CLOSE, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 3;
    pos += 1;
    write_u16_le(&mut packet, pos, handle.sfid);
    pos += 2;
    write_u32_le(&mut packet, pos, 0xffff_ffff);
    pos += 4;
    pos += 2;

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt)
        || !send_all(session.socket, &packet[..pos])
        || smb_check(session.socket, SMB_CLOSE, &mut packet) != SMB_SUCCESS
    {
        session.conn_valid = false;
    }
    *handle = FileHandle::new();
}

pub unsafe fn read_file(_buffer: *mut c_char, _size: usize, _offset: isize, _sfid: SmbFile) -> i32 {
    if _buffer.is_null() || _size == 0 {
        return SMB_ERROR;
    }
    let Some(handle) = file_handle_mut(_sfid) else {
        return SMB_ERROR;
    };
    let Some(session) = (unsafe { session_mut(handle.conn) }) else {
        return SMB_ERROR;
    };

    let mut total_read = 0usize;
    let mut packet = [0u8; SMB_BUF_BYTES];
    while total_read < _size {
        let next_read = (_size - total_read)
            .min(SMB_MAX_TRANSMIT_SIZE as usize)
            .min(SMB_BUF_BYTES - 64);
        make_header(&mut packet, SMB_READ_ANDX, session);
        let mut pos = SMB_HEADER_SIZE;
        packet[pos] = 12;
        pos += 1;
        packet[pos] = 0xff;
        pos += 1;
        packet[pos] = 0;
        pos += 1;
        pos += 2;
        write_u16_le(&mut packet, pos, handle.sfid);
        pos += 2;
        let read_offset = (_offset.max(0) as usize).saturating_add(total_read) as u64;
        write_u32_le(&mut packet, pos, read_offset as u32);
        pos += 4;
        write_u16_le(&mut packet, pos, next_read as u16);
        pos += 2;
        write_u16_le(&mut packet, pos, next_read as u16);
        pos += 2;
        write_u32_le(&mut packet, pos, 0);
        pos += 4;
        write_u16_le(&mut packet, pos, next_read as u16);
        pos += 2;
        write_u32_le(&mut packet, pos, (read_offset >> 32) as u32);
        pos += 4;
        pos += 2;

        let mut nbt = [0u8; 4];
        nbt[0] = NBT_SESSION_MSG;
        nbt[1] = 0;
        nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
        if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
            session.conn_valid = false;
            return SMB_ERROR;
        }
        let mut response_len = 0usize;
        if smb_check_with_len(
            session.socket,
            SMB_READ_ANDX,
            &mut packet,
            &mut response_len,
        ) != SMB_SUCCESS
        {
            session.conn_valid = false;
            return SMB_ERROR;
        }

        let length = read_u16_le(&packet, SMB_HEADER_SIZE + 11) as usize;
        if length == 0 {
            break;
        }
        let data_offset = read_u16_le(&packet, SMB_HEADER_SIZE + 13) as usize;
        if data_offset > response_len
            || length > next_read
            || length > response_len.saturating_sub(data_offset)
        {
            session.conn_valid = false;
            return SMB_ERROR;
        }
        let count = length;
        unsafe {
            core::ptr::copy_nonoverlapping(
                packet[data_offset..data_offset + count].as_ptr(),
                _buffer.cast::<u8>().add(total_read),
                count,
            );
        }
        total_read += count;
        if count < next_read {
            break;
        }
    }
    total_read as i32
}

pub unsafe fn write_file(
    _buffer: *const c_char,
    _size: usize,
    _offset: isize,
    _sfid: SmbFile,
) -> i32 {
    if _buffer.is_null() || _size == 0 {
        return SMB_ERROR;
    }
    let Some(handle) = file_handle_mut(_sfid) else {
        return SMB_ERROR;
    };
    let Some(session) = (unsafe { session_mut(handle.conn) }) else {
        return SMB_ERROR;
    };
    let request_size = _size.min(SMB_MAX_TRANSMIT_SIZE as usize);

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_WRITE_ANDX, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 14;
    pos += 1;
    packet[pos] = 0xff;
    pos += 2;
    pos += 2;
    write_u16_le(&mut packet, pos, handle.sfid);
    pos += 2;
    let write_offset = _offset.max(0) as u64;
    write_u32_le(&mut packet, pos, write_offset as u32);
    pos += 4;
    write_u32_le(&mut packet, pos, 0);
    pos += 4;
    write_u16_le(&mut packet, pos, 0);
    pos += 2;
    pos += 2;
    write_u16_le(&mut packet, pos, (request_size >> 16) as u16);
    pos += 2;
    write_u16_le(&mut packet, pos, request_size as u16);
    pos += 2;
    write_u16_le(&mut packet, pos, 63);
    pos += 2;
    write_u32_le(&mut packet, pos, (write_offset >> 32) as u32);
    pos += 4;
    write_u16_le(&mut packet, pos, request_size as u16);
    pos += 2;

    let copy_len = request_size.min(SMB_BUF_BYTES.saturating_sub(pos));
    unsafe {
        core::ptr::copy_nonoverlapping(_buffer.cast::<u8>(), packet[pos..].as_mut_ptr(), copy_len);
    }
    let send_len = pos + copy_len;
    let remaining = request_size.saturating_sub(copy_len);

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    let Some(frame_len) = request_size
        .checked_add(pos)
        .and_then(|len| u16::try_from(len).ok())
    else {
        return SMB_ERROR;
    };
    nbt[2..4].copy_from_slice(&frame_len.to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..send_len]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    if remaining != 0 {
        let tail =
            unsafe { core::slice::from_raw_parts(_buffer.cast::<u8>().add(copy_len), remaining) };
        if !send_all(session.socket, tail) {
            session.conn_valid = false;
            return SMB_ERROR;
        }
    }
    if smb_check(session.socket, SMB_WRITE_ANDX, &mut packet) != SMB_SUCCESS {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    read_u16_le(&packet, SMB_HEADER_SIZE + 5) as i32
}

pub unsafe fn create_directory(_dirname: *const c_char, mut _smbhndl: SmbConn) -> i32 {
    if _dirname.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut realfile = [0u8; 512];
    let Some(path_len) = prefixed_path(_dirname, &mut realfile) else {
        return SMB_ERROR;
    };
    make_header(&mut packet, SMB_COM_CREATE_DIRECTORY, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 0;
    pos += 1;
    pos += 2;
    let bpos = pos;
    packet[pos] = 0x04;
    pos += 1;
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &realfile[..path_len]);
    } else {
        packet[pos..pos + path_len].copy_from_slice(&realfile[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
    }
    write_u16_le(&mut packet, bpos - 2, (pos - bpos) as u16);
    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_COM_CREATE_DIRECTORY, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
    }
    rc
}

pub unsafe fn delete_directory(_dirname: *const c_char, mut _smbhndl: SmbConn) -> i32 {
    if _dirname.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut realfile = [0u8; 512];
    let Some(path_len) = prefixed_path(_dirname, &mut realfile) else {
        return SMB_ERROR;
    };
    make_header(&mut packet, SMB_COM_DELETE_DIRECTORY, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 0;
    pos += 1;
    pos += 2;
    let bpos = pos;
    packet[pos] = 0x04;
    pos += 1;
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &realfile[..path_len]);
    } else {
        packet[pos..pos + path_len].copy_from_slice(&realfile[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
    }
    write_u16_le(&mut packet, bpos - 2, (pos - bpos) as u16);
    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_COM_DELETE_DIRECTORY, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
    }
    rc
}

pub unsafe fn delete_file(_filename: *const c_char, mut _smbhndl: SmbConn) -> i32 {
    if _filename.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut realfile = [0u8; 512];
    let Some(path_len) = prefixed_path(_filename, &mut realfile) else {
        return SMB_ERROR;
    };
    make_header(&mut packet, SMB_COM_DELETE, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 1;
    pos += 1;
    write_u16_le(&mut packet, pos, 2);
    pos += 2;
    pos += 2;
    let bpos = pos;
    packet[pos] = 0x04;
    pos += 1;
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &realfile[..path_len]);
    } else {
        packet[pos..pos + path_len].copy_from_slice(&realfile[..path_len]);
        pos += path_len;
        packet[pos] = 0;
        pos += 1;
    }
    write_u16_le(&mut packet, bpos - 2, (pos - bpos) as u16);
    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_COM_DELETE, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
    }
    rc
}

pub unsafe fn rename(
    _filename: *const c_char,
    _newname: *const c_char,
    mut _smbhndl: SmbConn,
) -> i32 {
    if _filename.is_null() || _newname.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };
    let mut packet = [0u8; SMB_BUF_BYTES];
    let mut oldfile = [0u8; 512];
    let mut newfile = [0u8; 512];
    let Some(old_len) = prefixed_path(_filename, &mut oldfile) else {
        return SMB_ERROR;
    };
    let Some(new_len) = prefixed_path(_newname, &mut newfile) else {
        return SMB_ERROR;
    };
    make_header(&mut packet, SMB_COM_RENAME, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 1;
    pos += 1;
    write_u16_le(&mut packet, pos, 2);
    pos += 2;
    pos += 2;
    let bpos = pos;
    packet[pos] = 0x04;
    pos += 1;
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &oldfile[..old_len]);
    } else {
        packet[pos..pos + old_len].copy_from_slice(&oldfile[..old_len]);
        pos += old_len;
        packet[pos] = 0;
        pos += 1;
    }
    pos += 1;
    packet[pos] = 0x04;
    pos += 1;
    if session.unicode {
        pos += utf8_to_utf16le_or_error!(&mut packet[pos..], &newfile[..new_len]);
    } else {
        packet[pos..pos + new_len].copy_from_slice(&newfile[..new_len]);
        pos += new_len;
        packet[pos] = 0;
        pos += 1;
    }
    write_u16_le(&mut packet, bpos - 2, (pos - bpos) as u16);
    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }
    let rc = smb_check(session.socket, SMB_COM_RENAME, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
    }
    rc
}

pub unsafe fn disk_information(_buf: *mut c_void, mut _smbhndl: SmbConn) -> i32 {
    if _buf.is_null() {
        return SMB_ERROR;
    }
    let rc = unsafe { reconnect(&mut _smbhndl, 1) };
    if rc != SMB_SUCCESS {
        return SMB_ERROR;
    }
    let Some(session) = (unsafe { session_mut(_smbhndl) }) else {
        return SMB_ERROR;
    };

    let mut packet = [0u8; SMB_BUF_BYTES];
    make_header(&mut packet, SMB_COM_QUERY_INFORMATION_DISK, session);
    let mut pos = SMB_HEADER_SIZE;
    packet[pos] = 0;
    pos += 1;
    write_u16_le(&mut packet, pos, 0);
    pos += 2;

    let mut nbt = [0u8; 4];
    nbt[0] = NBT_SESSION_MSG;
    nbt[1] = 0;
    nbt[2..4].copy_from_slice(&(pos as u16).to_be_bytes());
    if !send_all(session.socket, &nbt) || !send_all(session.socket, &packet[..pos]) {
        session.conn_valid = false;
        return SMB_ERROR;
    }

    let rc = smb_check(session.socket, SMB_COM_QUERY_INFORMATION_DISK, &mut packet);
    if rc != SMB_SUCCESS {
        session.conn_valid = false;
        return rc;
    }

    let mut recv_pos = SMB_HEADER_SIZE + 1;
    let total_units = read_u16_le(&packet, recv_pos) as u32;
    recv_pos += 2;
    let blocks_per_unit = read_u16_le(&packet, recv_pos) as u32;
    recv_pos += 2;
    let block_size = read_u16_le(&packet, recv_pos) as u32;
    recv_pos += 2;
    let free_units = read_u16_le(&packet, recv_pos) as u32;

    unsafe {
        *(_buf.cast::<HostStatVfs>()) = HostStatVfs {
            f_bsize: block_size,
            f_frsize: block_size,
            f_blocks: total_units.saturating_mul(blocks_per_unit),
            f_bfree: free_units.saturating_mul(blocks_per_unit),
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_favail: 0,
            f_fsid: 0,
            f_flag: 0,
            f_namemax: SMB_MAXPATH as u32,
        };
    }

    SMB_SUCCESS
}
