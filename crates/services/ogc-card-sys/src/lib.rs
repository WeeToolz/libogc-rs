#![no_std]

//! Legacy CARD ABI facade backed exclusively by the Wii EXI implementation.
//!
//! Production PowerPC builds contain no synthetic card state. Asynchronous
//! operations are queued to the CARD worker and completion callbacks execute
//! after submission returns.
//! `CardFile::offset` is an opaque mount-generation token in this backend;
//! read and write byte offsets remain explicit function arguments.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_char, c_void};
use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering};

pub use ogc_sys::Bool;

pub const CARD_SLOTA: i32 = 0;
pub const CARD_SLOTB: i32 = 1;

pub const CARD_WORKAREA_SIZE: u32 = 5 * 8 * 1024;
pub const CARD_READSIZE: u32 = 512;
pub const CARD_FILENAMELEN: usize = 32;
pub const CARD_MAXFILES: usize = 127;

pub const CARD_ERROR_UNLOCKED: i32 = 1;
pub const CARD_ERROR_READY: i32 = 0;
pub const CARD_ERROR_BUSY: i32 = -1;
pub const CARD_ERROR_WRONGDEVICE: i32 = -2;
pub const CARD_ERROR_NOCARD: i32 = -3;
pub const CARD_ERROR_NOFILE: i32 = -4;
pub const CARD_ERROR_IOERROR: i32 = -5;
pub const CARD_ERROR_BROKEN: i32 = -6;
pub const CARD_ERROR_EXIST: i32 = -7;
pub const CARD_ERROR_NOENT: i32 = -8;
pub const CARD_ERROR_INSSPACE: i32 = -9;
pub const CARD_ERROR_NOPERM: i32 = -10;
pub const CARD_ERROR_LIMIT: i32 = -11;
pub const CARD_ERROR_NAMETOOLONG: i32 = -12;
pub const CARD_ERROR_ENCODING: i32 = -13;
pub const CARD_ERROR_CANCELED: i32 = -14;
pub const CARD_ERROR_FATAL_ERROR: i32 = -128;

pub const CARD_ATTRIB_PUBLIC: u8 = 0x04;
pub const CARD_ATTRIB_NOCOPY: u8 = 0x08;
pub const CARD_ATTRIB_NOMOVE: u8 = 0x10;

pub const CARD_BANNER_W: u32 = 96;
pub const CARD_BANNER_H: u32 = 32;
pub const CARD_BANNER_NONE: u8 = 0x00;
pub const CARD_BANNER_CI: u8 = 0x01;
pub const CARD_BANNER_RGB: u8 = 0x02;
pub const CARD_BANNER_MASK: u8 = 0x03;

pub const CARD_MAXICONS: usize = 8;
pub const CARD_ICON_W: u32 = 32;
pub const CARD_ICON_H: u32 = 32;
pub const CARD_ICON_NONE: u8 = 0x00;
pub const CARD_ICON_CI: u8 = 0x01;
pub const CARD_ICON_RGB: u8 = 0x02;
pub const CARD_ICON_MASK: u8 = 0x03;

pub const CARD_ANIM_LOOP: u8 = 0x00;
pub const CARD_ANIM_BOUNCE: u8 = 0x04;
pub const CARD_ANIM_MASK: u8 = 0x04;

pub const CARD_SPEED_END: u8 = 0x00;
pub const CARD_SPEED_FAST: u8 = 0x01;
pub const CARD_SPEED_MIDDLE: u8 = 0x02;
pub const CARD_SPEED_SLOW: u8 = 0x03;
pub const CARD_SPEED_MASK: u8 = 0x03;

pub type CardCallback = Option<unsafe extern "C" fn(chan: i32, result: i32)>;

/// Error returned when a CARD identifier contains a reserved byte.
///
/// Identifier bytes may not contain NUL because the original C API treated
/// identifiers as strings. `0xff` is reserved internally for the wildcard
/// state represented by `None` in the typed APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCardIdentifier {
    index: usize,
    byte: u8,
}

impl InvalidCardIdentifier {
    pub const fn index(self) -> usize {
        self.index
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }
}

const fn validate_identifier<const N: usize>(bytes: &[u8; N]) -> Result<(), InvalidCardIdentifier> {
    let mut index = 0;
    while index < N {
        let byte = bytes[index];
        if byte == 0 || byte == 0xff {
            return Err(InvalidCardIdentifier { index, byte });
        }
        index += 1;
    }
    Ok(())
}

/// Exact four-byte CARD game identifier.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardGameCode([u8; 4]);

impl CardGameCode {
    pub const fn new(bytes: [u8; 4]) -> Result<Self, InvalidCardIdentifier> {
        match validate_identifier(&bytes) {
            Ok(()) => Ok(Self(bytes)),
            Err(error) => Err(error),
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 4] {
        self.0
    }
}

/// Exact two-byte CARD company identifier.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardCompanyCode([u8; 2]);

impl CardCompanyCode {
    pub const fn new(bytes: [u8; 2]) -> Result<Self, InvalidCardIdentifier> {
        match validate_identifier(&bytes) {
            Ok(()) => Ok(Self(bytes)),
            Err(error) => Err(error),
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 2] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 2] {
        self.0
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct CardFile {
    pub chn: i32,
    pub filenum: i32,
    pub offset: i32,
    pub len: i32,
    pub iblock: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CardDir {
    pub chn: i32,
    pub fileno: u32,
    pub filelen: u32,
    pub permissions: u8,
    pub filename: [u8; CARD_FILENAMELEN],
    pub gamecode: [u8; 4],
    pub company: [u8; 2],
    pub showall: Bool,
}

impl Default for CardDir {
    fn default() -> Self {
        Self {
            chn: 0,
            fileno: 0,
            filelen: 0,
            permissions: 0,
            filename: [0; CARD_FILENAMELEN],
            gamecode: [0; 4],
            company: [0; 2],
            showall: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CardDirEntry {
    pub gamecode: [u8; 4],
    pub company: [u8; 2],
    pub pad_00: u8,
    pub banner_fmt: u8,
    pub filename: [u8; CARD_FILENAMELEN],
    pub last_modified: u32,
    pub icon_addr: u32,
    pub icon_fmt: u16,
    pub icon_speed: u16,
    pub permission: u8,
    pub copy_times: u8,
    pub block: u16,
    pub length: u16,
    pub pad_01: u16,
    pub comment_addr: u32,
}

impl Default for CardDirEntry {
    fn default() -> Self {
        Self {
            gamecode: [0; 4],
            company: [0; 2],
            pad_00: 0,
            banner_fmt: 0,
            filename: [0; CARD_FILENAMELEN],
            last_modified: 0,
            icon_addr: 0,
            icon_fmt: 0,
            icon_speed: 0,
            permission: 0,
            copy_times: 0,
            block: 0,
            length: 0,
            pad_01: 0,
            comment_addr: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CardStat {
    pub filename: [u8; CARD_FILENAMELEN],
    pub len: u32,
    pub time: u32,
    pub gamecode: [u8; 4],
    pub company: [u8; 2],
    pub banner_fmt: u8,
    pub icon_addr: u32,
    pub icon_fmt: u16,
    pub iconfmt: [u16; CARD_MAXICONS],
    pub icon_speed: u16,
    pub iconspeed: [u16; CARD_MAXICONS],
    pub comment_addr: u32,
    pub offset_banner: u32,
    pub offset_banner_tlut: u32,
    pub offset_icon: [u32; CARD_MAXICONS],
    pub offset_icon_tlut: [u32; CARD_MAXICONS],
    pub offset_data: u32,
}

impl Default for CardStat {
    fn default() -> Self {
        Self {
            filename: [0; CARD_FILENAMELEN],
            len: 0,
            time: 0,
            gamecode: [0; 4],
            company: [0; 2],
            banner_fmt: 0,
            icon_addr: 0,
            icon_fmt: 0,
            iconfmt: [0; CARD_MAXICONS],
            icon_speed: 0,
            iconspeed: [0; CARD_MAXICONS],
            comment_addr: 0,
            offset_banner: 0,
            offset_banner_tlut: 0,
            offset_icon: [0; CARD_MAXICONS],
            offset_icon_tlut: [0; CARD_MAXICONS],
            offset_data: 0,
        }
    }
}

static CARD_GAMECODE: AtomicU32 = AtomicU32::new(u32::MAX);
static CARD_COMPANY: AtomicU16 = AtomicU16::new(u16::MAX);
const CARD_INIT_NOT_STARTED: u8 = 0;
const CARD_INIT_RUNNING: u8 = 1;
const CARD_INIT_COMPLETE: u8 = 2;
static CARD_INIT_STATE: AtomicU8 = AtomicU8::new(CARD_INIT_NOT_STARTED);

fn init_once(state: &AtomicU8, initialize: impl FnOnce() -> i32) -> i32 {
    loop {
        match state.load(Ordering::Acquire) {
            CARD_INIT_COMPLETE => return CARD_ERROR_READY,
            CARD_INIT_RUNNING => core::hint::spin_loop(),
            CARD_INIT_NOT_STARTED => {
                if state
                    .compare_exchange(
                        CARD_INIT_NOT_STARTED,
                        CARD_INIT_RUNNING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_err()
                {
                    continue;
                }

                let result = initialize();
                state.store(
                    if result == CARD_ERROR_READY {
                        CARD_INIT_COMPLETE
                    } else {
                        CARD_INIT_NOT_STARTED
                    },
                    Ordering::Release,
                );
                return result;
            }
            _ => return CARD_ERROR_FATAL_ERROR,
        }
    }
}

fn current_gamecode() -> [u8; 4] {
    CARD_GAMECODE.load(Ordering::Acquire).to_be_bytes()
}

fn current_company() -> [u8; 2] {
    CARD_COMPANY.load(Ordering::Acquire).to_be_bytes()
}

fn encode_gamecode(gamecode: Option<CardGameCode>) -> u32 {
    u32::from_be_bytes(match gamecode {
        Some(value) => value.into_bytes(),
        None => [0xff; 4],
    })
}

fn encode_company(company: Option<CardCompanyCode>) -> u16 {
    u16::from_be_bytes(match company {
        Some(value) => value.into_bytes(),
        None => [0xff; 2],
    })
}

#[cfg(target_arch = "powerpc")]
mod wii_backend;

fn finish_callback(chn: i32, result: i32, callback: CardCallback) {
    if let Some(callback) = callback {
        unsafe { callback(chn, result) };
    }
}

fn entry_name_matches(entry: &CardDirEntry, filename: *const c_char) -> bool {
    if filename.is_null() {
        return false;
    }
    let name = unsafe { CStr::from_ptr(filename) }.to_bytes();
    let compare_len = name.len().min(CARD_FILENAMELEN);
    entry.filename[..compare_len] == name[..compare_len]
        && (compare_len == CARD_FILENAMELEN || entry.filename[compare_len] == 0)
}

/// Initializes CARD with optional exact game and company identifiers.
///
/// `None` selects the wildcard behavior previously represented by a null C
/// pointer. Identifier state is stored atomically, so concurrent readers never
/// race with updates. Callers that require a coherent game/company pair should
/// configure identifiers before starting CARD operations.
pub fn init(gamecode: Option<CardGameCode>, company: Option<CardCompanyCode>) -> i32 {
    // BUG FIX: C CARD_Init is idempotent. Do not reset mounted slots or change
    // identifiers after the backend has initialized successfully.
    init_once(&CARD_INIT_STATE, || {
        let result = wii_backend::init();
        if result == CARD_ERROR_READY {
            set_gamecode(gamecode);
            set_company(company);
        }
        result
    })
}

/// Updates the game identifier used to filter and create CARD entries.
///
/// `None` disables game-code filtering.
pub fn set_gamecode(gamecode: Option<CardGameCode>) {
    CARD_GAMECODE.store(encode_gamecode(gamecode), Ordering::Release);
}

/// Updates the company identifier used to filter and create CARD entries.
///
/// `None` disables company-code filtering.
pub fn set_company(company: Option<CardCompanyCode>) {
    CARD_COMPANY.store(encode_company(company), Ordering::Release);
}

pub fn probe(chn: i32) -> i32 {
    wii_backend::probe(chn)
}

/// # Safety
///
/// Each non-null output pointer must be properly aligned and valid for writes
/// for the duration of the call.
pub unsafe fn probe_ex(chn: i32, mem_size: *mut i32, sect_size: *mut i32) -> i32 {
    unsafe { wii_backend::probe_ex(chn, mem_size, sect_size) }
}

pub fn get_error_code(chn: i32) -> i32 {
    wii_backend::get_error_code(chn)
}

/// # Safety
///
/// If non-null, `sector_size` must be properly aligned and valid for writes
/// for the duration of the call.
pub unsafe fn get_sector_size(chn: i32, sector_size: *mut u32) -> i32 {
    unsafe { wii_backend::get_sector_size(chn, sector_size) }
}

/// # Safety
///
/// If non-null, `block_count` must be properly aligned and valid for writes
/// for the duration of the call.
pub unsafe fn get_block_count(chn: i32, block_count: *mut u32) -> i32 {
    unsafe { wii_backend::get_block_count(chn, block_count) }
}

/// # Safety
///
/// If non-null, `freeblocks` must be properly aligned and valid for writes
/// for the duration of the call.
pub unsafe fn get_free_blocks(chn: i32, freeblocks: *mut u16) -> i32 {
    unsafe { wii_backend::get_free_blocks(chn, freeblocks) }
}

/// # Safety
///
/// `workarea` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn mount_async(
    chn: i32,
    workarea: *mut c_void,
    detach_cb: CardCallback,
    attach_cb: CardCallback,
) -> i32 {
    wii_backend::mount_async(chn, workarea, detach_cb, attach_cb)
}

/// # Safety
///
/// `workarea` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn mount(chn: i32, workarea: *mut c_void, detach_cb: CardCallback) -> i32 {
    wii_backend::mount(chn, workarea, detach_cb)
}

pub fn unmount(chn: i32) -> i32 {
    wii_backend::unmount(chn)
}

/// # Safety
///
/// `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn read_async(
    file: *mut CardFile,
    buffer: *mut c_void,
    len: u32,
    offset: u32,
    callback: CardCallback,
) -> i32 {
    wii_backend::read_async(file, buffer, len, offset, callback)
}

/// # Safety
///
/// `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn read(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    wii_backend::read(file, buffer, len, offset)
}

/// # Safety
///
/// `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn write_async(
    file: *mut CardFile,
    buffer: *mut c_void,
    len: u32,
    offset: u32,
    callback: CardCallback,
) -> i32 {
    wii_backend::write_async(file, buffer, len, offset, callback)
}

/// # Safety
///
/// `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `buffer` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn write(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    wii_backend::write(file, buffer, len, offset)
}

/// # Safety
///
/// `filename` must be null or point to a readable NUL-terminated C string.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn open(chn: i32, filename: *const c_char, file: *mut CardFile) -> i32 {
    wii_backend::open(chn, filename, file)
}

/// # Safety
///
/// `entry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn open_entry(chn: i32, entry: *mut CardDir, file: *mut CardFile) -> i32 {
    wii_backend::open_entry(chn, entry, file)
}

/// # Safety
///
/// `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn close(file: *mut CardFile) -> i32 {
    wii_backend::close(file)
}

/// # Safety
///
/// `filename` must be null or point to a readable NUL-terminated C string.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn create_async(
    chn: i32,
    filename: *const c_char,
    size: u32,
    file: *mut CardFile,
    callback: CardCallback,
) -> i32 {
    wii_backend::create_async(chn, filename, size, file, callback)
}

/// # Safety
///
/// `filename` must be null or point to a readable NUL-terminated C string.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn create(chn: i32, filename: *const c_char, size: u32, file: *mut CardFile) -> i32 {
    wii_backend::create(chn, filename, size, file)
}

/// # Safety
///
/// `direntry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn create_entry_async(
    chn: i32,
    direntry: *mut CardDir,
    file: *mut CardFile,
    callback: CardCallback,
) -> i32 {
    wii_backend::create_entry_async(chn, direntry, file, callback)
}

/// # Safety
///
/// `direntry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `file` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn create_entry(chn: i32, direntry: *mut CardDir, file: *mut CardFile) -> i32 {
    wii_backend::create_entry(chn, direntry, file)
}

/// # Safety
///
/// `filename` must be null or point to a readable NUL-terminated C string.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn delete_async(chn: i32, filename: *const c_char, callback: CardCallback) -> i32 {
    wii_backend::delete_async(chn, filename, callback)
}

/// # Safety
///
/// `filename` must be null or point to a readable NUL-terminated C string.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn delete(chn: i32, filename: *const c_char) -> i32 {
    wii_backend::delete(chn, filename)
}

/// # Safety
///
/// `dir_entry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn delete_entry_async(chn: i32, dir_entry: *mut CardDir, callback: CardCallback) -> i32 {
    wii_backend::delete_entry_async(chn, dir_entry, callback)
}

/// # Safety
///
/// `dir_entry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn delete_entry(chn: i32, dir_entry: *mut CardDir) -> i32 {
    wii_backend::delete_entry(chn, dir_entry)
}

/// # Safety
///
/// `dir` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn find_first(chn: i32, dir: *mut CardDir, showall: Bool) -> i32 {
    wii_backend::find_first(chn, dir, showall)
}

/// # Safety
///
/// `dir` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn find_next(dir: *mut CardDir) -> i32 {
    wii_backend::find_next(dir)
}

/// # Safety
///
/// `dir_entries` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `count` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn get_directory(
    chn: i32,
    dir_entries: *mut CardDir,
    count: *mut i32,
    showall: Bool,
) -> i32 {
    wii_backend::get_directory(chn, dir_entries, count, showall)
}

/// # Safety
///
/// `stats` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn get_status(chn: i32, fileno: i32, stats: *mut CardStat) -> i32 {
    wii_backend::get_status(chn, fileno, stats)
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that remains valid for reads for the duration of the call.
pub unsafe fn get_banner_fmt(stat: *const CardStat) -> u8 {
    if stat.is_null() {
        CARD_BANNER_NONE
    } else {
        unsafe { (*stat).banner_fmt & CARD_BANNER_MASK }
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that is exclusively accessible for the duration of the call.
pub unsafe fn set_banner_fmt(stat: *mut CardStat, fmt: u8) {
    if let Some(stat) = unsafe { stat.as_mut() } {
        stat.banner_fmt = (stat.banner_fmt & !CARD_BANNER_MASK) | (fmt & CARD_BANNER_MASK);
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that remains valid for reads for the duration of the call.
pub unsafe fn get_icon_fmt(stat: *const CardStat, n: usize) -> u16 {
    if stat.is_null() || n >= CARD_MAXICONS {
        0
    } else {
        unsafe { ((*stat).icon_fmt >> (2 * n)) & CARD_ICON_MASK as u16 }
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that is exclusively accessible for the duration of the call.
pub unsafe fn set_icon_fmt(stat: *mut CardStat, n: usize, fmt: u16) {
    if n >= CARD_MAXICONS {
        return;
    }
    if let Some(stat) = unsafe { stat.as_mut() } {
        let shift = 2 * n;
        let mask = (CARD_ICON_MASK as u16) << shift;
        stat.icon_fmt = (stat.icon_fmt & !mask) | ((fmt & CARD_ICON_MASK as u16) << shift);
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that remains valid for reads for the duration of the call.
pub unsafe fn get_icon_speed(stat: *const CardStat, n: usize) -> u16 {
    if stat.is_null() || n >= CARD_MAXICONS {
        0
    } else {
        unsafe { ((*stat).icon_speed >> (2 * n)) & CARD_SPEED_MASK as u16 }
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that is exclusively accessible for the duration of the call.
pub unsafe fn set_icon_speed(stat: *mut CardStat, n: usize, speed: u16) {
    if n >= CARD_MAXICONS {
        return;
    }
    if let Some(stat) = unsafe { stat.as_mut() } {
        let shift = 2 * n;
        let mask = (CARD_SPEED_MASK as u16) << shift;
        stat.icon_speed = (stat.icon_speed & !mask) | ((speed & CARD_SPEED_MASK as u16) << shift);
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that is exclusively accessible for the duration of the call.
pub unsafe fn set_icon_addr(stat: *mut CardStat, addr: u32) {
    if let Some(stat) = unsafe { stat.as_mut() } {
        stat.icon_addr = addr;
    }
}

/// # Safety
///
/// If non-null, `stat` must be properly aligned and point to an initialized
/// `CardStat` that is exclusively accessible for the duration of the call.
pub unsafe fn set_comment_addr(stat: *mut CardStat, addr: u32) {
    if let Some(stat) = unsafe { stat.as_mut() } {
        stat.comment_addr = addr;
    }
}

/// # Safety
///
/// `entry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn get_status_ex(chn: i32, fileno: i32, entry: *mut CardDirEntry) -> i32 {
    wii_backend::get_status_ex(chn, fileno, entry)
}

/// # Safety
///
/// `stats` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// All referenced objects must remain valid and unmoved until the
/// asynchronous operation and its callback have completed.
pub unsafe fn set_status_async(
    chn: i32,
    fileno: i32,
    stats: *mut CardStat,
    callback: CardCallback,
) -> i32 {
    wii_backend::set_status_async(chn, fileno, stats, callback)
}

/// # Safety
///
/// `stats` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn set_status(chn: i32, fileno: i32, stats: *mut CardStat) -> i32 {
    wii_backend::set_status(chn, fileno, stats)
}

/// # Safety
///
/// `entry` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn set_status_ex(chn: i32, fileno: i32, entry: *mut CardDirEntry) -> i32 {
    wii_backend::set_status_ex(chn, fileno, entry)
}

/// # Safety
///
/// `attr` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn get_attributes(chn: i32, fileno: i32, attr: *mut u8) -> i32 {
    wii_backend::get_attributes(chn, fileno, attr)
}

pub fn set_attributes_async(chn: i32, fileno: i32, attr: u8, callback: CardCallback) -> i32 {
    wii_backend::set_attributes_async(chn, fileno, attr, callback)
}

pub fn set_attributes(chn: i32, fileno: i32, attr: u8) -> i32 {
    wii_backend::set_attributes(chn, fileno, attr)
}

pub fn format_async(chn: i32, callback: CardCallback) -> i32 {
    wii_backend::format_async(chn, callback)
}

pub fn format(chn: i32) -> i32 {
    wii_backend::format(chn)
}

/// # Safety
///
/// `serial1` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `serial2` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn get_serial_no(chn: i32, serial1: *mut u32, serial2: *mut u32) -> i32 {
    wii_backend::get_serial_no(chn, serial1, serial2)
}

pub fn shutdown() -> i32 {
    wii_backend::shutdown()
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::{CARD_ERROR_READY, CardCallback, init_once};
    use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

    #[test]
    fn callbacks_use_c_abi() {
        unsafe extern "C" fn callback(_channel: i32, _result: i32) {}

        let _callback: CardCallback = Some(callback);
    }

    #[test]
    fn repeated_init_keeps_first_identifiers_and_does_not_reinitialize_backend() {
        let state = AtomicU8::new(super::CARD_INIT_NOT_STARTED);
        let gamecode = AtomicU32::new(u32::MAX);
        let init_calls = AtomicU32::new(0);

        assert_eq!(
            init_once(&state, || {
                init_calls.fetch_add(1, Ordering::Relaxed);
                gamecode.store(u32::from_be_bytes(*b"ABCD"), Ordering::Release);
                CARD_ERROR_READY
            }),
            CARD_ERROR_READY
        );
        assert_eq!(
            gamecode.load(Ordering::Acquire),
            u32::from_be_bytes(*b"ABCD")
        );

        assert_eq!(
            init_once(&state, || {
                init_calls.fetch_add(1, Ordering::Relaxed);
                gamecode.store(u32::from_be_bytes(*b"WXYZ"), Ordering::Release);
                CARD_ERROR_READY
            }),
            CARD_ERROR_READY
        );
        assert_eq!(init_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            gamecode.load(Ordering::Acquire),
            u32::from_be_bytes(*b"ABCD")
        );
    }
}
