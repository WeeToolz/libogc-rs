#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::cmp::min;
use core::ffi::{CStr, c_char, c_void};
use core::hint::spin_loop;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};

pub use ogc_filesystem_sys as sys;
use ogc_sysbase::{self as sysbase, Devoptab, DirIter, Stat, StatVfs};
pub use sys::{DiscInterface, ISO_MAXPATHLEN};

const SECTOR_SIZE: usize = 0x800;
const VOLUME_ID_SIZE: usize = 32;
const ISO9660_DESCRIPTOR_START: u32 = 16;
const ISO9660_DESCRIPTOR_END: u32 = 32;
const ISO9660_STANDARD_IDENTIFIER: &[u8; 5] = b"CD001";
const ISO9660_DESCRIPTOR_VERSION: u8 = 1;
const FLAG_DIR: u8 = 2;
const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;
const S_IRUSR: u32 = 0o0400;
const S_IRGRP: u32 = 0o0040;
const S_IROTH: u32 = 0o0004;
const ST_RDONLY: u32 = 0x0001;
const ST_NOSUID: u32 = 0x0002;

struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> SpinGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> core::ops::Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[derive(Copy, Clone)]
struct Entry {
    name: [u8; ISO_MAXPATHLEN],
    sector: u32,
    size: u32,
    flags: u8,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            name: [0; ISO_MAXPATHLEN],
            sector: 0,
            size: 0,
            flags: 0,
        }
    }
}

impl Entry {
    fn is_dir(&self) -> bool {
        self.flags & FLAG_DIR != 0
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct FileState {
    entry: Entry,
    offset: i64,
    in_use: bool,
    mount: *const Mount,
}

#[repr(C)]
#[derive(Default)]
struct DirState {
    entry: Entry,
    entries: Vec<Entry>,
    offset: u32,
    in_use: bool,
    mount: *const Mount,
}

struct Mount {
    volume_id: [u8; VOLUME_ID_SIZE],
    disc_interface: usize,
    unicode: bool,
    root: Entry,
    device_name: Vec<u8>,
    devoptab: Box<Devoptab>,
}

unsafe impl Send for Mount {}

static MOUNTS: SpinLock<BTreeMap<Vec<u8>, Box<Mount>>> = SpinLock::new(BTreeMap::new());

fn normalize_mount_name(name: &CStr) -> Option<Vec<u8>> {
    let bytes = name.to_bytes();
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }

    let mut normalized = Vec::with_capacity(bytes.len() + 1);
    normalized.extend_from_slice(bytes);
    if bytes.last().copied() != Some(b':') {
        normalized.push(b':');
    }
    Some(normalized)
}

fn normalize_lookup_name(name: &CStr) -> Option<Vec<u8>> {
    let bytes = name.to_bytes();
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }

    let mut normalized = Vec::with_capacity(bytes.len() + 1);
    normalized.extend_from_slice(bytes);
    if bytes.last().copied() != Some(b':') {
        normalized.push(b':');
    }
    Some(normalized)
}

fn read_sector(disc: &DiscInterface, sector: u32, buffer: &mut [u8; SECTOR_SIZE]) -> bool {
    let Some(read_sectors) = disc.read_sectors else {
        return false;
    };

    // SAFETY: `buffer` is a valid writable sector-sized allocation and the
    // callback is invoked for exactly one sector.
    unsafe { read_sectors(sector, 1, buffer.as_mut_ptr().cast::<c_void>()) }
}

fn read_range(disc: &DiscInterface, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; len];
    let mut copied = 0usize;

    while copied < len {
        let absolute = offset.checked_add(copied as u64)?;
        let sector = u32::try_from(absolute / SECTOR_SIZE as u64).ok()?;
        let sector_offset = (absolute % SECTOR_SIZE as u64) as usize;
        let chunk = min(len - copied, SECTOR_SIZE - sector_offset);
        let mut sector_buf = [0u8; SECTOR_SIZE];

        if !read_sector(disc, sector, &mut sector_buf) {
            return None;
        }

        out[copied..copied + chunk]
            .copy_from_slice(&sector_buf[sector_offset..sector_offset + chunk]);
        copied += chunk;
    }

    Some(out)
}

struct VolumeDescriptor {
    volume_id: [u8; VOLUME_ID_SIZE],
    path_table_len: u32,
    path_table_sector: u32,
    root: Entry,
    unicode: bool,
}

fn parse_volume_descriptor(bytes: &[u8; SECTOR_SIZE]) -> Option<VolumeDescriptor> {
    if &bytes[1..6] != ISO9660_STANDARD_IDENTIFIER || bytes[6] != ISO9660_DESCRIPTOR_VERSION {
        return None;
    }

    let descriptor_type = bytes[0];
    if descriptor_type != 1 && descriptor_type != 2 {
        return None;
    }

    let mut volume_id = [0u8; VOLUME_ID_SIZE];
    volume_id.copy_from_slice(&bytes[40..72]);

    let path_table_len = u32::from_be_bytes(bytes[136..140].try_into().ok()?);
    let path_table_sector = u32::from_be_bytes(bytes[148..152].try_into().ok()?);
    let root_record = &bytes[156..190];
    let root = Entry {
        sector: u32::from_be_bytes(root_record[6..10].try_into().ok()?),
        size: u32::from_be_bytes(root_record[14..18].try_into().ok()?),
        flags: root_record[25],
        ..Entry::default()
    };

    Some(VolumeDescriptor {
        volume_id,
        path_table_len,
        path_table_sector,
        root,
        unicode: descriptor_type == 2,
    })
}

fn read_volume_descriptor(disc: &DiscInterface, descriptor_type: u8) -> Option<VolumeDescriptor> {
    let mut sector_buf = [0u8; SECTOR_SIZE];

    for sector in ISO9660_DESCRIPTOR_START..ISO9660_DESCRIPTOR_END {
        if !read_sector(disc, sector, &mut sector_buf) {
            return None;
        }

        if &sector_buf[1..6] != ISO9660_STANDARD_IDENTIFIER
            || sector_buf[6] != ISO9660_DESCRIPTOR_VERSION
        {
            continue;
        }

        if sector_buf[0] == descriptor_type {
            return parse_volume_descriptor(&sector_buf);
        }

        if sector_buf[0] == 0xff {
            return None;
        }
    }

    None
}

fn validate_path_table_bytes(table: &[u8]) -> bool {
    let mut offset = 0usize;
    let mut entry_index = 1u16;
    while offset < table.len() {
        if table.len() - offset < 8 {
            return false;
        }

        let name_len = table[offset] as usize;
        let entry_len = 8 + name_len + (name_len & 1);
        if name_len >= ISO_MAXPATHLEN || offset + entry_len > table.len() {
            return false;
        }

        let parent = u16::from_be_bytes([table[offset + 6], table[offset + 7]]);
        if entry_index == 1 {
            if parent != 1 {
                return false;
            }
        } else if parent == 0 || parent >= entry_index {
            return false;
        }

        entry_index = match entry_index.checked_add(1) {
            Some(index) => index,
            None => return false,
        };
        offset += entry_len;
    }
    true
}

fn validate_path_table(disc: &DiscInterface, descriptor: &VolumeDescriptor) -> bool {
    if descriptor.path_table_len == 0 {
        return true;
    }

    let Some(table) = read_range(
        disc,
        u64::from(descriptor.path_table_sector) * SECTOR_SIZE as u64,
        descriptor.path_table_len as usize,
    ) else {
        return false;
    };

    validate_path_table_bytes(&table)
}

fn parse_iso9660(disc: &DiscInterface) -> Option<VolumeDescriptor> {
    let descriptor = read_volume_descriptor(disc, 2).or_else(|| read_volume_descriptor(disc, 1))?;
    if !validate_path_table(disc, &descriptor) {
        return None;
    }
    Some(descriptor)
}

fn mount_disc(mount: &Mount) -> &DiscInterface {
    unsafe { &*(mount.disc_interface as *const DiscInterface) }
}

fn decode_record_name(record: &[u8], unicode: bool, out: &mut [u8; ISO_MAXPATHLEN]) -> usize {
    let name_len = record[32] as usize;
    if name_len == 1 && record[33] <= 1 {
        if record[33] == 1 {
            out[..2].copy_from_slice(b"..");
            return 2;
        }
        return 0;
    }
    let mut len = 0usize;
    if unicode {
        for idx in 0..(name_len / 2) {
            if len + 1 >= ISO_MAXPATHLEN {
                break;
            }
            out[len] = record[33 + idx * 2 + 1];
            len += 1;
        }
    } else {
        len = name_len.min(ISO_MAXPATHLEN - 1);
        out[..len].copy_from_slice(&record[33..33 + len]);
    }
    if record[25] & FLAG_DIR == 0 && len >= 2 && out[len - 2] == b';' {
        len -= 2;
    }
    out[len] = 0;
    len
}

fn directory_entries(mount: &Mount, directory: &Entry) -> Option<Vec<Entry>> {
    if !directory.is_dir() || directory.size == 0 {
        return Some(Vec::new());
    }
    let data = read_range(
        mount_disc(mount),
        u64::from(directory.sector) * SECTOR_SIZE as u64,
        directory.size as usize,
    )?;
    let mut entries = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let record_len = data[offset] as usize;
        if record_len == 0 {
            offset = ((offset / SECTOR_SIZE) + 1) * SECTOR_SIZE;
            continue;
        }
        if record_len < 34 || offset + record_len > data.len() {
            return None;
        }
        let record = &data[offset..offset + record_len];
        let record_name_len = record[32] as usize;
        let record_name_end = 33usize.checked_add(record_name_len)?;
        if record_name_end > record_len {
            return None;
        }
        let mut name = [0u8; ISO_MAXPATHLEN];
        let name_len = decode_record_name(record, mount.unicode, &mut name);
        let is_dot = name_len == 0 && record[33] == 0;
        let is_root_dotdot =
            name_len == 2 && &name[..2] == b".." && directory.sector == mount.root.sector;
        if !is_dot && !is_root_dotdot {
            entries.push(Entry {
                name,
                sector: u32::from_be_bytes(record[6..10].try_into().ok()?)
                    .checked_add(u32::from(record[1]))?,
                size: u32::from_be_bytes(record[14..18].try_into().ok()?),
                flags: record[25],
            });
        }
        offset += record_len;
    }
    Some(entries)
}

fn eq_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn entry_name(entry: &Entry) -> &[u8] {
    let len = entry
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(entry.name.len());
    &entry.name[..len]
}

fn resolve_entry(mount: &Mount, path: &CStr) -> Option<Entry> {
    let mut bytes = path.to_bytes();
    if let Some(colon) = bytes.iter().position(|byte| *byte == b':') {
        bytes = &bytes[colon + 1..];
    }
    let mut current = mount.root;
    for component in bytes
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty())
    {
        if component == b"." {
            continue;
        }
        let entries = directory_entries(mount, &current)?;
        current = entries
            .into_iter()
            .find(|entry| eq_ascii_case(entry_name(entry), component))?;
    }
    Some(current)
}

fn mount_from_path(path: &CStr) -> Option<&'static Mount> {
    let dev = sysbase::device_op_tab(path)?;
    let data = unsafe { (*dev).device_data };
    if data.is_null() {
        return None;
    }
    Some(unsafe { &*data.cast::<Mount>() })
}

fn fill_stat(entry: &Entry, st: *mut c_void) {
    if st.is_null() {
        return;
    }
    let mode = (if entry.is_dir() { S_IFDIR } else { S_IFREG }) | S_IRUSR | S_IRGRP | S_IROTH;
    unsafe {
        *st.cast::<Stat>() = Stat {
            st_dev: 69,
            st_ino: entry.sector,
            st_mode: mode,
            st_nlink: 1,
            st_uid: 1,
            st_gid: 2,
            st_rdev: 69,
            st_size: i64::from(entry.size),
            st_blksize: SECTOR_SIZE as i32,
            st_blocks: entry.size.div_ceil(SECTOR_SIZE as u32) as i32,
            ..Stat::default()
        };
    }
}

#[inline]
unsafe fn set_errno(r: *mut sysbase::Reent, errno: i32) {
    unsafe { sysbase::set_reent_errno(r, errno) };
}

unsafe extern "C" fn iso_open(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    path: *const c_char,
    _flags: i32,
    _mode: i32,
) -> i32 {
    if path.is_null() {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    }
    let path = unsafe { CStr::from_ptr(path) };
    let Some(mount) = mount_from_path(path) else {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    };
    let Some(entry) = resolve_entry(mount, path) else {
        unsafe { set_errno(r, sysbase::ENOENT) };
        return -1;
    };
    if entry.is_dir() {
        unsafe { set_errno(r, sysbase::EISDIR) };
        return -1;
    }
    if file_struct.is_null() {
        unsafe { set_errno(r, sysbase::EINVAL) };
        return -1;
    }
    unsafe {
        *file_struct.cast::<FileState>() = FileState {
            entry,
            offset: 0,
            in_use: true,
            mount,
        };
    }
    file_struct as usize as i32
}

unsafe extern "C" fn iso_close(r: *mut sysbase::Reent, file_struct: *mut c_void) -> i32 {
    if file_struct.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<FileState>() };
    if !file.in_use {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    file.in_use = false;
    0
}

unsafe extern "C" fn iso_read(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    ptr: *mut c_char,
    len: usize,
) -> isize {
    if file_struct.is_null() || (ptr.is_null() && len != 0) {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<FileState>() };
    if !file.in_use || file.mount.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    if file.offset as u32 >= file.entry.size {
        unsafe { set_errno(r, sysbase::EOVERFLOW) };
        return 0;
    }
    let remaining = file.entry.size.wrapping_sub(file.offset as u32) as usize;
    let count = len.min(remaining);
    if count < len {
        unsafe { set_errno(r, sysbase::EOVERFLOW) };
    }
    if count == 0 {
        return 0;
    }
    let mount = unsafe { &*file.mount };
    let offset = u64::from(file.entry.sector) * SECTOR_SIZE as u64 + file.offset as u64;
    let Some(data) = read_range(mount_disc(mount), offset, count) else {
        unsafe { set_errno(r, sysbase::EIO) };
        return -1;
    };
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), ptr.cast::<u8>(), count) };
    file.offset = file.offset.wrapping_add(count as i64);
    count as isize
}

unsafe extern "C" fn iso_seek(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    pos: i64,
    dir: i32,
) -> i64 {
    if file_struct.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let file = unsafe { &mut *file_struct.cast::<FileState>() };
    if !file.in_use {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let position = match dir {
        sysbase::SEEK_SET => pos,
        sysbase::SEEK_CUR => file.offset.wrapping_add(pos),
        sysbase::SEEK_END => i64::from(file.entry.size).wrapping_add(pos),
        _ => {
            unsafe { set_errno(r, sysbase::EINVAL) };
            return -1;
        }
    };
    if pos > 0 && position < 0 {
        unsafe { set_errno(r, sysbase::EOVERFLOW) };
        return -1;
    }
    if position < 0 || u32::try_from(position).map_or(true, |value| value > file.entry.size) {
        unsafe { set_errno(r, sysbase::EINVAL) };
        return -1;
    }
    file.offset = position;
    position
}

unsafe extern "C" fn iso_fstat(
    r: *mut sysbase::Reent,
    file_struct: *mut c_void,
    st: *mut c_void,
) -> i32 {
    if file_struct.is_null() || st.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let file = unsafe { &*file_struct.cast::<FileState>() };
    if !file.in_use {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    fill_stat(&file.entry, st);
    0
}

unsafe extern "C" fn iso_stat(r: *mut sysbase::Reent, path: *const c_char, st: *mut c_void) -> i32 {
    if path.is_null() {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    }
    if st.is_null() {
        unsafe { set_errno(r, sysbase::EINVAL) };
        return -1;
    }
    let path = unsafe { CStr::from_ptr(path) };
    let Some(mount) = mount_from_path(path) else {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    };
    let Some(entry) = resolve_entry(mount, path) else {
        unsafe { set_errno(r, sysbase::ENOENT) };
        return -1;
    };
    fill_stat(&entry, st);
    0
}

unsafe extern "C" fn iso_chdir(r: *mut sysbase::Reent, path: *const c_char) -> i32 {
    if path.is_null() {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    }
    let path = unsafe { CStr::from_ptr(path) };
    let Some(mount) = mount_from_path(path) else {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return -1;
    };
    match resolve_entry(mount, path) {
        Some(entry) if entry.is_dir() => 0,
        Some(_) => {
            unsafe { set_errno(r, sysbase::ENOTDIR) };
            -1
        }
        None => {
            unsafe { set_errno(r, sysbase::ENOENT) };
            -1
        }
    }
}

unsafe extern "C" fn iso_diropen(
    r: *mut sysbase::Reent,
    dir_iter: *mut DirIter,
    path: *const c_char,
) -> *mut DirIter {
    if path.is_null() {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return core::ptr::null_mut();
    }
    if dir_iter.is_null() {
        unsafe { set_errno(r, sysbase::EINVAL) };
        return core::ptr::null_mut();
    }
    let path_ref = unsafe { CStr::from_ptr(path) };
    let Some(mount) = mount_from_path(path_ref) else {
        unsafe { set_errno(r, sysbase::ENODEV) };
        return core::ptr::null_mut();
    };
    let Some(entry) = resolve_entry(mount, path_ref) else {
        unsafe { set_errno(r, sysbase::ENOENT) };
        return core::ptr::null_mut();
    };
    if !entry.is_dir() {
        unsafe { set_errno(r, sysbase::ENOTDIR) };
        return core::ptr::null_mut();
    }
    let Some(entries) = directory_entries(mount, &entry) else {
        unsafe { set_errno(r, sysbase::EIO) };
        return core::ptr::null_mut();
    };
    let state_ptr = unsafe { (*dir_iter).dir_struct };
    if state_ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        core::ptr::write(
            state_ptr.cast::<DirState>(),
            DirState {
                entry,
                entries,
                offset: 0,
                in_use: true,
                mount,
            },
        );
    }
    dir_iter
}

unsafe extern "C" fn iso_dirreset(r: *mut sysbase::Reent, dir_iter: *mut DirIter) -> i32 {
    if dir_iter.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state_ptr = unsafe { (*dir_iter).dir_struct };
    if state_ptr.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state = unsafe { &mut *state_ptr.cast::<DirState>() };
    if !state.in_use {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    state.offset = 0;
    0
}

unsafe extern "C" fn iso_dirnext(
    r: *mut sysbase::Reent,
    dir_iter: *mut DirIter,
    filename: *mut c_char,
    st: *mut c_void,
) -> i32 {
    if dir_iter.is_null() || filename.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state_ptr = unsafe { (*dir_iter).dir_struct };
    if state_ptr.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state = unsafe { &mut *state_ptr.cast::<DirState>() };
    if !state.in_use || state.mount.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let index = state.offset as usize;
    let Some(entry) = state.entries.get(index) else {
        unsafe { set_errno(r, sysbase::ENOENT) };
        return -1;
    };
    state.offset = state.offset.saturating_add(1);
    let name = entry_name(entry);
    unsafe {
        core::ptr::copy_nonoverlapping(name.as_ptr(), filename.cast::<u8>(), name.len());
        *filename.add(name.len()) = 0;
    }
    fill_stat(entry, st);
    0
}

unsafe extern "C" fn iso_dirclose(r: *mut sysbase::Reent, dir_iter: *mut DirIter) -> i32 {
    if dir_iter.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state_ptr = unsafe { (*dir_iter).dir_struct };
    if state_ptr.is_null() {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    let state = unsafe { &mut *state_ptr.cast::<DirState>() };
    if !state.in_use {
        unsafe { set_errno(r, sysbase::EBADF) };
        return -1;
    }
    state.in_use = false;
    drop(core::mem::take(&mut state.entries));
    0
}

unsafe extern "C" fn iso_statvfs(
    _r: *mut sysbase::Reent,
    _path: *const c_char,
    buf: *mut c_void,
) -> i32 {
    if buf.is_null() {
        return -1;
    }
    unsafe {
        let stat = &mut *buf.cast::<StatVfs>();
        stat.f_bsize = SECTOR_SIZE as u32;
        stat.f_frsize = SECTOR_SIZE as u32;
        // C intentionally leaves f_blocks and f_files untouched.
        stat.f_bfree = 0;
        stat.f_bavail = 0;
        stat.f_ffree = 0;
        stat.f_favail = 0;
        stat.f_fsid = 0;
        stat.f_flag = ST_NOSUID | ST_RDONLY;
        stat.f_namemax = 208;
    }
    0
}

fn make_devoptab(name: *const c_char, mount: *mut c_void) -> Devoptab {
    Devoptab {
        name,
        struct_size: size_of::<FileState>(),
        open_r: Some(iso_open),
        close_r: Some(iso_close),
        write_r: None,
        read_r: Some(iso_read),
        seek_r: Some(iso_seek),
        fstat_r: Some(iso_fstat),
        stat_r: Some(iso_stat),
        link_r: None,
        unlink_r: None,
        chdir_r: Some(iso_chdir),
        rename_r: None,
        mkdir_r: None,
        dir_state_size: size_of::<DirState>(),
        diropen_r: Some(iso_diropen),
        dirreset_r: Some(iso_dirreset),
        dirnext_r: Some(iso_dirnext),
        dirclose_r: Some(iso_dirclose),
        statvfs_r: Some(iso_statvfs),
        ftruncate_r: None,
        fsync_r: None,
        device_data: mount,
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

fn mount_impl(name: &CStr, disc_interface: &DiscInterface) -> bool {
    let Some(name_key) = normalize_mount_name(name) else {
        return false;
    };
    let Some(startup) = disc_interface.startup else {
        return false;
    };
    let Some(is_inserted) = disc_interface.is_inserted else {
        return false;
    };
    unsafe {
        if !startup() || !is_inserted() {
            return false;
        }
    }
    let mut device_name = name_key.clone();
    device_name.push(0);
    let device_cstr = unsafe { CStr::from_bytes_with_nul_unchecked(&device_name) };
    if sysbase::find_device(device_cstr) >= 0 {
        return false;
    }
    let Some(descriptor) = parse_iso9660(disc_interface) else {
        return false;
    };
    let mut volume_id = descriptor.volume_id;
    volume_id[VOLUME_ID_SIZE - 1] = 0;

    let mut mount = Box::new(Mount {
        volume_id,
        disc_interface: disc_interface as *const DiscInterface as usize,
        unicode: descriptor.unicode,
        root: descriptor.root,
        device_name,
        devoptab: Box::new(make_devoptab(core::ptr::null(), core::ptr::null_mut())),
    });
    let mount_ptr: *mut Mount = &mut *mount;
    mount.devoptab.name = mount.device_name.as_ptr().cast();
    mount.devoptab.device_data = mount_ptr.cast();
    if unsafe { sysbase::add_device(&*mount.devoptab) } < 0 {
        return false;
    }

    let mut mounts = MOUNTS.lock();
    if mounts.contains_key(&name_key) {
        let c_name = unsafe { CStr::from_bytes_with_nul_unchecked(&mount.device_name) };
        let _ = sysbase::remove_device(c_name);
        return false;
    }
    mounts.insert(name_key, mount);
    true
}

fn unmount_impl(name: &CStr) -> bool {
    let Some(name_key) = normalize_lookup_name(name) else {
        return false;
    };
    let mut mounts = MOUNTS.lock();
    let Some(mount) = mounts.get(&name_key) else {
        return false;
    };
    let c_name = unsafe { CStr::from_bytes_with_nul_unchecked(&mount.device_name) };
    if sysbase::remove_device(c_name) < 0 {
        return false;
    }
    mounts.remove(&name_key);
    true
}

fn volume_label_impl(name: &CStr) -> *const c_char {
    let Some(name_key) = normalize_lookup_name(name) else {
        return core::ptr::null();
    };

    let mounts = MOUNTS.lock();
    mounts.get(&name_key).map_or(core::ptr::null(), |mount| {
        mount.volume_id.as_ptr().cast::<c_char>()
    })
}

pub fn mount(name: &CStr, disc_interface: &DiscInterface) -> bool {
    mount_impl(name, disc_interface)
}

pub fn unmount(name: &CStr) -> bool {
    unmount_impl(name)
}

pub fn volume_label(name: &CStr) -> *const c_char {
    volume_label_impl(name)
}

/// # Safety
///
/// `name` must point to a valid NUL-terminated device name and `disc_interface`
/// must point to a valid `DiscInterface` for the duration of the call.
pub unsafe fn iso9660_mount(name: *const c_char, disc_interface: *const DiscInterface) -> bool {
    if name.is_null() || disc_interface.is_null() {
        return false;
    }

    let name = unsafe { CStr::from_ptr(name) };
    let disc_interface = unsafe { &*disc_interface };
    mount_impl(name, disc_interface)
}

/// # Safety
///
/// `name` must point to a valid NUL-terminated device name for the duration
/// of the call.
pub unsafe fn iso9660_unmount(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name = unsafe { CStr::from_ptr(name) };
    unmount_impl(name)
}

/// # Safety
///
/// `name` must point to a valid NUL-terminated device name for the duration
/// of the call.
pub unsafe fn iso9660_get_volume_label(name: *const c_char) -> *const c_char {
    if name.is_null() {
        return core::ptr::null();
    }

    let name = unsafe { CStr::from_ptr(name) };
    volume_label_impl(name)
}

/// C-compatible `ISO9660_Mount` entry point.
///
/// # Safety
///
/// `name` must point to a valid NUL-terminated device name and `disc_interface`
/// must point to a valid `DiscInterface` for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ISO9660_Mount(
    name: *const c_char,
    disc_interface: *const DiscInterface,
) -> bool {
    unsafe { iso9660_mount(name, disc_interface) }
}

/// C-compatible `ISO9660_Unmount` entry point.
///
/// # Safety
///
/// `name` must point to a valid NUL-terminated device name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ISO9660_Unmount(name: *const c_char) -> bool {
    unsafe { iso9660_unmount(name) }
}

/// C-compatible `ISO9660_GetVolumeLabel` entry point.
///
/// # Safety
///
/// `name` must point to a valid NUL-terminated device name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ISO9660_GetVolumeLabel(name: *const c_char) -> *const c_char {
    unsafe { iso9660_get_volume_label(name) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use std::sync::Mutex;

    static IMAGE: Mutex<[u8; SECTOR_SIZE * 32]> = Mutex::new([0; SECTOR_SIZE * 32]);
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn c_entry_points_have_c_abi_signatures() {
        let _mount: unsafe extern "C" fn(*const c_char, *const DiscInterface) -> bool =
            ISO9660_Mount;
        let _unmount: unsafe extern "C" fn(*const c_char) -> bool = ISO9660_Unmount;
        let _label: unsafe extern "C" fn(*const c_char) -> *const c_char = ISO9660_GetVolumeLabel;
    }

    #[test]
    fn statvfs_uses_target_unsigned_long_fields() {
        let mut stat = StatVfs::default();
        assert_eq!(
            unsafe {
                iso_statvfs(
                    core::ptr::null_mut(),
                    core::ptr::null(),
                    (&mut stat as *mut StatVfs).cast(),
                )
            },
            0
        );
        assert_eq!(stat.f_bsize, 0x800);
        assert_eq!(stat.f_frsize, 0x800);
        assert_eq!(stat.f_flag, ST_NOSUID | ST_RDONLY);
        assert_eq!(stat.f_namemax, 208);
    }

    #[test]
    fn devoptab_seek_accepts_target_off_t_width() {
        let mut entry = Entry::default();
        entry.size = u32::MAX;
        let mut file = FileState {
            entry,
            offset: 0,
            in_use: true,
            mount: core::ptr::null(),
        };
        let position = i64::from(i32::MAX) + 17;
        assert_eq!(
            unsafe {
                iso_seek(
                    core::ptr::null_mut(),
                    (&mut file as *mut FileState).cast(),
                    position,
                    sysbase::SEEK_SET,
                )
            },
            position
        );
        assert_eq!(file.offset, position);
    }

    unsafe extern "C" fn startup_ok() -> bool {
        true
    }

    unsafe extern "C" fn inserted_ok() -> bool {
        true
    }

    unsafe extern "C" fn read_sectors(
        sector: u32,
        num_sectors: u32,
        buffer: *mut core::ffi::c_void,
    ) -> bool {
        let start = sector as usize * SECTOR_SIZE;
        let len = num_sectors as usize * SECTOR_SIZE;
        let end = start + len;
        let image = match IMAGE.lock() {
            Ok(image) => image,
            Err(poisoned) => poisoned.into_inner(),
        };
        if end > image.len() {
            return false;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(image.as_ptr().add(start), buffer.cast::<u8>(), len);
        }
        true
    }

    fn test_disc() -> DiscInterface {
        DiscInterface {
            io_type: 0,
            features: 0,
            startup: Some(startup_ok),
            is_inserted: Some(inserted_ok),
            read_sectors: Some(read_sectors),
            write_sectors: None,
            clear_status: None,
            shutdown: None,
        }
    }

    fn clear_mounts() {
        let mut mounts = MOUNTS.lock();
        for mount in mounts.values() {
            let name = unsafe { CStr::from_bytes_with_nul_unchecked(&mount.device_name) };
            let _ = sysbase::remove_device(name);
        }
        mounts.clear();
    }

    fn prepare_image(volume_id: &[u8]) {
        let mut image = match IMAGE.lock() {
            Ok(image) => image,
            Err(poisoned) => poisoned.into_inner(),
        };
        *image = [0; SECTOR_SIZE * 32];
        image[16 * SECTOR_SIZE] = 1;
        image[16 * SECTOR_SIZE + 1..16 * SECTOR_SIZE + 6].copy_from_slice(b"CD001");
        image[16 * SECTOR_SIZE + 6] = 1;
        image[16 * SECTOR_SIZE + 40..16 * SECTOR_SIZE + 40 + volume_id.len()]
            .copy_from_slice(volume_id);
        image[17 * SECTOR_SIZE] = 0xff;
        image[17 * SECTOR_SIZE + 1..17 * SECTOR_SIZE + 6].copy_from_slice(b"CD001");
        image[17 * SECTOR_SIZE + 6] = 1;
    }

    fn path_table_record(name: &[u8], parent: u16) -> Vec<u8> {
        let mut record = vec![0u8; 8 + name.len() + (name.len() & 1)];
        record[0] = name.len() as u8;
        record[6..8].copy_from_slice(&parent.to_be_bytes());
        record[8..8 + name.len()].copy_from_slice(name);
        record
    }

    fn write_directory_record(
        image: &mut [u8],
        offset: usize,
        sector: u32,
        size: u32,
        flags: u8,
        name: &[u8],
    ) -> usize {
        let record_len = 33 + name.len() + usize::from(name.len() % 2 == 0);
        image[offset] = record_len as u8;
        image[offset + 2..offset + 6].copy_from_slice(&sector.to_le_bytes());
        image[offset + 6..offset + 10].copy_from_slice(&sector.to_be_bytes());
        image[offset + 10..offset + 14].copy_from_slice(&size.to_le_bytes());
        image[offset + 14..offset + 18].copy_from_slice(&size.to_be_bytes());
        image[offset + 25] = flags;
        image[offset + 28..offset + 30].copy_from_slice(&1u16.to_le_bytes());
        image[offset + 30..offset + 32].copy_from_slice(&1u16.to_be_bytes());
        image[offset + 32] = name.len() as u8;
        image[offset + 33..offset + 33 + name.len()].copy_from_slice(name);
        record_len
    }

    fn prepare_file_image(volume_id: &[u8]) {
        prepare_image(volume_id);
        let mut image = match IMAGE.lock() {
            Ok(image) => image,
            Err(poisoned) => poisoned.into_inner(),
        };
        let root_sector = 20u32;
        let file_sector = 21u32;
        let root_size = SECTOR_SIZE as u32;
        let root_record = 16 * SECTOR_SIZE + 156;
        let _ = write_directory_record(
            &mut image[..],
            root_record,
            root_sector,
            root_size,
            FLAG_DIR,
            &[0],
        );

        let mut offset = root_sector as usize * SECTOR_SIZE;
        offset += write_directory_record(
            &mut image[..],
            offset,
            root_sector,
            root_size,
            FLAG_DIR,
            &[0],
        );
        offset += write_directory_record(
            &mut image[..],
            offset,
            root_sector,
            root_size,
            FLAG_DIR,
            &[1],
        );
        let _ = write_directory_record(&mut image[..], offset, file_sector, 5, 0, b"HELLO.TXT;1");
        image[file_sector as usize * SECTOR_SIZE..file_sector as usize * SECTOR_SIZE + 5]
            .copy_from_slice(b"hello");
    }

    #[test]
    fn path_table_rejects_missing_or_forward_parents() {
        let mut table = path_table_record(&[0], 1);
        table.extend_from_slice(&path_table_record(b"A", 1));
        table.extend_from_slice(&path_table_record(b"B", 2));
        assert!(validate_path_table_bytes(&table));

        let mut forward = path_table_record(&[0], 1);
        forward.extend_from_slice(&path_table_record(b"A", 3));
        assert!(!validate_path_table_bytes(&forward));

        let mut zero_parent = path_table_record(&[0], 1);
        zero_parent.extend_from_slice(&path_table_record(b"A", 0));
        assert!(!validate_path_table_bytes(&zero_parent));
    }

    #[test]
    fn diropen_snapshots_entries_before_iteration() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_file_image(b"DIR-SNAPSHOT");
        let disc = test_disc();
        assert!(mount(c"disc", &disc));

        let mut reent_storage = core::mem::MaybeUninit::<sysbase::Reent>::zeroed();
        let reent = reent_storage.as_mut_ptr();
        let mut dir_state = DirState::default();
        let mut dir_iter = DirIter {
            device: 0,
            dir_struct: (&mut dir_state as *mut DirState).cast(),
        };
        assert!(!unsafe { iso_diropen(reent, &mut dir_iter, c"disc:/".as_ptr()) }.is_null());

        // Mutate the on-disc name after open. C continues from the child list
        // captured by diropen, so dirnext must still return the original name.
        {
            let mut image = match IMAGE.lock() {
                Ok(image) => image,
                Err(poisoned) => poisoned.into_inner(),
            };
            let root_sector = 20usize * SECTOR_SIZE;
            let first = image[root_sector] as usize;
            let second = image[root_sector + first] as usize;
            let file_record = root_sector + first + second;
            image[file_record + 33..file_record + 38].copy_from_slice(b"OTHER");
        }

        let mut name = [0 as c_char; ISO_MAXPATHLEN];
        let mut stat = Stat::default();
        assert_eq!(
            unsafe {
                iso_dirnext(
                    reent,
                    &mut dir_iter,
                    name.as_mut_ptr(),
                    (&mut stat as *mut Stat).cast(),
                )
            },
            0
        );
        assert_eq!(
            unsafe { CStr::from_ptr(name.as_ptr()).to_bytes() },
            b"HELLO.TXT"
        );
        assert_eq!(unsafe { iso_dirclose(reent, &mut dir_iter) }, 0);
        assert!(unmount(c"disc"));
    }

    #[test]
    fn mount_returns_label_and_unmount_clears_registry() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_image(b"TESTVOL");
        let name = c"disc";
        let disc = test_disc();

        assert!(mount(name, &disc));
        let label = volume_label(name);
        assert!(!label.is_null());
        assert_eq!(unsafe { CStr::from_ptr(label).to_bytes() }, b"TESTVOL");
        assert!(unmount(name));
        assert!(volume_label(name).is_null());
    }

    #[test]
    fn mount_normalizes_trailing_colon_only_once() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_image(b"TESTVOL");
        let disc = test_disc();

        assert!(mount(c"disc:", &disc));
        let label = volume_label(c"disc");
        assert!(!label.is_null());
        assert_eq!(unsafe { CStr::from_ptr(label).to_bytes() }, b"TESTVOL");
        assert!(!mount(c"disc", &disc));
    }

    #[test]
    fn mount_rejects_duplicate_names() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_image(b"FIRST");
        let name = c"disc";
        let disc = test_disc();

        assert!(mount(name, &disc));
        assert!(!mount(name, &disc));
    }

    #[test]
    fn c_exports_are_callable() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_image(b"EXPORTS");
        let name = c"iso";
        let disc = test_disc();

        assert!(unsafe { iso9660_mount(name.as_ptr(), &disc) });
        let label = unsafe { iso9660_get_volume_label(name.as_ptr()) };
        assert_eq!(unsafe { CStr::from_ptr(label).to_bytes() }, b"EXPORTS");
        assert!(unsafe { iso9660_unmount(name.as_ptr()) });
    }
    #[test]
    fn devoptab_callbacks_set_c_compatible_errno() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_file_image(b"ERRNO");
        let disc = test_disc();
        assert!(mount(c"disc", &disc));

        let mut reent_storage = core::mem::MaybeUninit::<sysbase::Reent>::zeroed();
        let reent = reent_storage.as_mut_ptr();
        let mut file = FileState::default();
        let fd = unsafe {
            iso_open(
                reent,
                (&mut file as *mut FileState).cast(),
                c"disc:/HELLO.TXT".as_ptr(),
                0,
                0,
            )
        };
        assert_ne!(fd, -1);
        file.offset = i64::from(file.entry.size);
        let mut byte = 0u8;
        assert_eq!(
            unsafe {
                iso_read(
                    reent,
                    (&mut file as *mut FileState).cast(),
                    (&mut byte as *mut u8).cast(),
                    1,
                )
            },
            0
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::EOVERFLOW);

        unsafe { (*reent)._errno = 0 };
        let mut stat = Stat::default();
        assert_eq!(
            unsafe {
                iso_stat(
                    reent,
                    c"disc:/MISSING".as_ptr(),
                    (&mut stat as *mut Stat).cast(),
                )
            },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENOENT);
        assert!(unmount(c"disc"));
    }

    #[test]
    fn null_callback_paths_report_unresolved_mount() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        let mut reent_storage = core::mem::MaybeUninit::<sysbase::Reent>::zeroed();
        let reent = reent_storage.as_mut_ptr();
        let null_path = core::ptr::null();
        let mut file = FileState::default();
        let mut stat = Stat::default();
        let mut dir_state = DirState::default();
        let mut dir_iter = DirIter {
            device: 0,
            dir_struct: (&mut dir_state as *mut DirState).cast(),
        };

        assert_eq!(
            unsafe { iso_open(reent, (&mut file as *mut FileState).cast(), null_path, 0, 0,) },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENODEV);

        unsafe { (*reent)._errno = 0 };
        assert_eq!(
            unsafe { iso_stat(reent, null_path, (&mut stat as *mut Stat).cast()) },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENODEV);

        unsafe { (*reent)._errno = 0 };
        assert_eq!(unsafe { iso_chdir(reent, null_path) }, -1);
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENODEV);

        unsafe { (*reent)._errno = 0 };
        assert!(unsafe { iso_diropen(reent, &mut dir_iter, null_path) }.is_null());
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENODEV);
    }

    #[test]
    fn null_file_pointer_preserves_open_error_precedence() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_file_image(b"OPEN-ERRORS");
        let disc = test_disc();
        assert!(mount(c"disc", &disc));

        let mut reent_storage = core::mem::MaybeUninit::<sysbase::Reent>::zeroed();
        let reent = reent_storage.as_mut_ptr();
        let null_file = core::ptr::null_mut();

        assert_eq!(
            unsafe { iso_open(reent, null_file, c"missing:/HELLO.TXT".as_ptr(), 0, 0) },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENODEV);

        unsafe { (*reent)._errno = 0 };
        assert_eq!(
            unsafe { iso_open(reent, null_file, c"disc:/MISSING".as_ptr(), 0, 0) },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::ENOENT);

        unsafe { (*reent)._errno = 0 };
        assert_eq!(
            unsafe { iso_open(reent, null_file, c"disc:/".as_ptr(), 0, 0) },
            -1
        );
        assert_eq!(unsafe { (*reent)._errno }, sysbase::EISDIR);

        assert!(unmount(c"disc"));
    }

    #[test]
    fn file_reads_start_after_extended_attribute_sectors() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_file_image(b"EXT-ATTR");
        {
            let mut image = match IMAGE.lock() {
                Ok(image) => image,
                Err(poisoned) => poisoned.into_inner(),
            };
            let file_record = 20 * SECTOR_SIZE + 2 * 34;
            image[file_record + 1] = 2;
            image[21 * SECTOR_SIZE..21 * SECTOR_SIZE + 5].copy_from_slice(b"wrong");
            image[23 * SECTOR_SIZE..23 * SECTOR_SIZE + 5].copy_from_slice(b"right");
        }
        let disc = test_disc();
        assert!(mount(c"disc", &disc));

        let mut reent_storage = core::mem::MaybeUninit::<sysbase::Reent>::zeroed();
        let reent = reent_storage.as_mut_ptr();
        let mut file = FileState::default();
        assert_ne!(
            unsafe {
                iso_open(
                    reent,
                    (&mut file as *mut FileState).cast(),
                    c"disc:/HELLO.TXT".as_ptr(),
                    0,
                    0,
                )
            },
            -1
        );

        let mut data = [0u8; 5];
        assert_eq!(
            unsafe {
                iso_read(
                    reent,
                    (&mut file as *mut FileState).cast(),
                    data.as_mut_ptr().cast(),
                    data.len(),
                )
            },
            data.len() as isize
        );
        assert_eq!(&data, b"right");
        assert!(unmount(c"disc"));
    }

    #[test]
    fn mounted_devoptab_reads_and_seeks_files() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        clear_mounts();
        prepare_file_image(b"FILES");
        let disc = test_disc();
        assert!(mount(c"disc", &disc));

        let file = sysbase::DeviceFile::open_read(c"disc:/HELLO.TXT").expect("ISO file opens");
        let mut first = [0u8; 2];
        assert_eq!(file.read(&mut first), Ok(2));
        assert_eq!(&first, b"he");
        assert_eq!(file.seek(1, sysbase::SEEK_SET), Ok(1));
        let mut tail = [0u8; 4];
        assert_eq!(file.read(&mut tail), Ok(4));
        assert_eq!(&tail, b"ello");

        drop(file);
        assert!(unmount(c"disc"));
    }
}
