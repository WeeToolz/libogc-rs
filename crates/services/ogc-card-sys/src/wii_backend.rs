#![cfg(target_arch = "powerpc")]

use core::ffi::c_void;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use ogc_cache::{dc_invalidate_range, dc_store_range};
use ogc_card_metadata::{
    self as metadata, BLOCK_SIZE as METADATA_BLOCK_SIZE, BlockIo, CommitError, MetadataBlock,
    PairCommit,
};

use ogc_lwp_sys as lwp;
use ogc_system_sys::sys_get_hollywood_revision;
use ogc_system_sys::{_sys_lock_sram, _sys_lock_sram_ex, _sys_unlock_sram, _sys_unlock_sram_ex};

use ogc_exi::{
    EXI_DEVICE_0, EXI_MEMCARD59, EXI_MEMCARD123, EXI_MEMCARD251, EXI_MEMCARD507, EXI_MEMCARD1019,
    EXI_MEMCARD2043, EXI_READ, EXI_SPEED1MHZ, EXI_SPEED16MHZ, EXI_WRITE,
};

use super::*;

const EXI_REG_BASE: usize = 0xCD00_6800;
const EXI_SELECT_MASK: u32 = 0x0405;
const EXI_STATUS_UNLOCKED: u8 = 0x40;

const ASYNC_QUEUE_CAPACITY: usize = 16;
const ASYNC_WORKER_STACK_SIZE: usize = 16 * 1024;
const ASYNC_WORKER_PRIORITY: u8 = 105;
const MAX_CARD_SECTOR_SIZE: usize = 0x40000;
const VI_REG_BASE: usize = 0xCC00_2000;

fn format_serial(
    time: u64,
    flash_id: [u8; 12],
    counter_bias: u32,
    language: u8,
    vi_state: u16,
) -> [u32; 8] {
    let mut random = time;
    let mut random_bytes = [0; 12];
    for (byte, flash_byte) in random_bytes.iter_mut().zip(flash_id) {
        random = random.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039) >> 16;
        *byte = flash_byte.wrapping_add(random as u8);
        random = (random.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039) >> 16) & 0x7fff;
    }
    [
        u32::from_be_bytes(random_bytes[0..4].try_into().unwrap()),
        u32::from_be_bytes(random_bytes[4..8].try_into().unwrap()),
        u32::from_be_bytes(random_bytes[8..12].try_into().unwrap()),
        (time >> 32) as u32,
        time as u32,
        counter_bias,
        u32::from(language),
        u32::from(vi_state),
    ]
}

unsafe fn formatted_serial(chn: i32) -> Option<[u32; 8]> {
    let sram = loop {
        let sram = unsafe { _sys_lock_sram() };
        if !sram.is_null() {
            break sram;
        }
        lwp::yield_thread();
    };
    let counter_bias = unsafe { core::ptr::addr_of!((*sram).counter_bias).read_unaligned() };
    let language = unsafe { core::ptr::addr_of!((*sram).lang).read_unaligned() };
    unsafe { _sys_unlock_sram(0) };

    let sram_ex = loop {
        let sram_ex = unsafe { _sys_lock_sram_ex() };
        if !sram_ex.is_null() {
            break sram_ex;
        }
        lwp::yield_thread();
    };
    let flash_ids = unsafe { core::ptr::addr_of!((*sram_ex).flash_id).read_unaligned() };
    unsafe { _sys_unlock_sram_ex(0) };

    let slot = slot_index(chn)?;
    let flash_id = flash_ids[slot];
    let time = ogc_core::powerpc::read_timebase();
    let vi_state = unsafe { core::ptr::read_volatile((VI_REG_BASE as *const u16).add(55)) };
    Some(format_serial(
        time,
        flash_id,
        counter_bias,
        language,
        vi_state,
    ))
}

fn file_range_exceeds_len(file_len: u32, len: u32, offset: u32) -> bool {
    offset > file_len || len > file_len.saturating_sub(offset)
}

fn card_time_seconds(ticks: u64, is_wii: bool) -> u32 {
    let ticks_per_second = if is_wii { 60_750_000 } else { 40_500_000 };
    (ticks / ticks_per_second) as u32
}

fn current_card_time_seconds() -> u32 {
    let ticks = ogc_core::powerpc::read_timebase();
    let is_wii = unsafe { sys_get_hollywood_revision() != 0 };
    card_time_seconds(ticks, is_wii)
}

fn valid_file_block(block: u16, block_count: u32) -> bool {
    block >= 5 && u32::from(block) < block_count
}

fn valid_file_number(fileno: i32) -> bool {
    (0..CARD_MAXFILES as i32).contains(&fileno)
}

fn valid_create_size(size: u32, sector_size: u32) -> bool {
    size != 0 && sector_size != 0 && size.is_multiple_of(sector_size)
}

fn status_entry_has_filename(entry: &CardDirEntry) -> bool {
    !matches!(entry.filename[0], 0 | 0xff)
}

fn snapshot_dir_entry(dir_entry: *const CardDir) -> Option<CardDir> {
    unsafe { dir_entry.as_ref().copied() }
}

fn snapshot_card_stat(stats: *const CardStat) -> Option<CardStat> {
    unsafe { stats.as_ref().copied() }
}

fn snapshot_filename(
    filename: *const core::ffi::c_char,
) -> Option<([u8; CARD_FILENAMELEN + 1], usize)> {
    if filename.is_null() {
        return None;
    }
    let bytes = unsafe { core::ffi::CStr::from_ptr(filename) }.to_bytes();
    let mut snapshot = [0; CARD_FILENAMELEN + 1];
    let copy_len = bytes.len().min(CARD_FILENAMELEN);
    snapshot[..copy_len].copy_from_slice(&bytes[..copy_len]);
    Some((snapshot, bytes.len()))
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum AsyncKind {
    None,
    Mount,
    Read,
    Write,
    Create,
    CreateEntry,
    Delete,
    DeleteEntry,
    Format,
    SetStatus,
    SetAttributes,
}

#[derive(Copy, Clone)]
struct AsyncRequest {
    kind: AsyncKind,
    chn: i32,
    workarea: *mut c_void,
    detach_cb: CardCallback,
    callback: CardCallback,
    file: *mut CardFile,
    buffer: *mut c_void,
    len: u32,
    offset: u32,
    filename: [u8; CARD_FILENAMELEN + 1],
    size: u32,
    direntry: CardDir,
    fileno: i32,
    stats: core::mem::MaybeUninit<CardStat>,
    attr: u8,
    encoding: u32,
    generation: u32,
}

impl AsyncRequest {
    const fn empty() -> Self {
        Self {
            kind: AsyncKind::None,
            chn: -1,
            workarea: core::ptr::null_mut(),
            detach_cb: None,
            callback: None,
            file: core::ptr::null_mut(),
            buffer: core::ptr::null_mut(),
            len: 0,
            offset: 0,
            filename: [0; CARD_FILENAMELEN + 1],
            size: 0,
            direntry: CardDir {
                chn: 0,
                fileno: 0,
                filelen: 0,
                permissions: 0,
                filename: [0; CARD_FILENAMELEN],
                gamecode: [0; 4],
                company: [0; 2],
                showall: 0,
            },
            fileno: -1,
            stats: core::mem::MaybeUninit::zeroed(),
            attr: 0,
            encoding: 0,
            generation: 0,
        }
    }
}

#[repr(align(32))]
struct AsyncWorkerStack([u8; ASYNC_WORKER_STACK_SIZE]);

#[repr(align(32))]
struct SectorScratch([u8; MAX_CARD_SECTOR_SIZE]);

static ASYNC_READY: AtomicBool = AtomicBool::new(false);
static ASYNC_STARTING: AtomicBool = AtomicBool::new(false);
static ASYNC_STOP: AtomicBool = AtomicBool::new(false);
static SLOT_DETACH_CALLBACK: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];
static SLOT_GENERATION: [AtomicU32; 2] = [const { AtomicU32::new(1) }; 2];
static mut ASYNC_MUTEX: lwp::Mutex = lwp::LWP_MUTEX_NULL;
static mut ASYNC_COND: lwp::CondVar = lwp::LWP_COND_NULL;
static mut ASYNC_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;
static mut ASYNC_STACK: AsyncWorkerStack = AsyncWorkerStack([0; ASYNC_WORKER_STACK_SIZE]);
static mut WRITE_SCRATCH: [SectorScratch; 2] = [
    SectorScratch([0; MAX_CARD_SECTOR_SIZE]),
    SectorScratch([0; MAX_CARD_SECTOR_SIZE]),
];
static mut ASYNC_QUEUE: [AsyncRequest; ASYNC_QUEUE_CAPACITY] =
    [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
static mut ASYNC_HEAD: usize = 0;
static mut ASYNC_LEN: usize = 0;

fn encode_callback(callback: CardCallback) -> usize {
    callback.map_or(0, |callback| callback as *const () as usize)
}

unsafe fn decode_callback(value: usize) -> CardCallback {
    if value == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(i32, i32)>(value) })
    }
}

fn current_generation(slot: usize) -> i32 {
    SLOT_GENERATION[slot].load(Ordering::Acquire) as i32
}

fn cancel_queued_for_slot(chn: i32) {
    if !ASYNC_READY.load(Ordering::Acquire) {
        return;
    }
    let mut cancelled = [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
    let mut cancelled_len = 0usize;
    unsafe {
        let mutex = ASYNC_MUTEX;
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            return;
        }
        let mut retained = [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
        let mut retained_len = 0usize;
        for index in 0..ASYNC_LEN {
            let request = ASYNC_QUEUE[(ASYNC_HEAD + index) % ASYNC_QUEUE_CAPACITY];
            if request.chn == chn && request.kind != AsyncKind::Mount {
                cancelled[cancelled_len] = request;
                cancelled_len += 1;
            } else {
                retained[retained_len] = request;
                retained_len += 1;
            }
        }
        ASYNC_QUEUE = [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
        ASYNC_HEAD = 0;
        ASYNC_LEN = retained_len;
        ASYNC_QUEUE[..retained_len].copy_from_slice(&retained[..retained_len]);
        let _ = lwp::mutex_unlock(mutex);
    }
    for request in cancelled.into_iter().take(cancelled_len) {
        finish_callback(request.chn, CARD_ERROR_CANCELED, request.callback);
    }
}

fn check_detach_slot(slot: usize) {
    if !SLOT_MOUNTED[slot].load(Ordering::Acquire) {
        return;
    }
    let chn = if slot == 0 { CARD_SLOTA } else { CARD_SLOTB };
    if probe(chn) == CARD_ERROR_READY {
        return;
    }
    SLOT_MOUNTED[slot].store(false, Ordering::Release);
    SLOT_PRESENT[slot].store(false, Ordering::Release);
    SLOT_WORKAREA[slot].store(0, Ordering::Release);
    SLOT_GENERATION[slot].fetch_add(1, Ordering::AcqRel);
    set_error(slot, CARD_ERROR_NOCARD);
    cancel_queued_for_slot(chn);
    let callback = SLOT_DETACH_CALLBACK[slot].swap(0, Ordering::AcqRel);
    unsafe {
        if let Some(callback) = decode_callback(callback) {
            callback(chn, CARD_ERROR_NOCARD);
        }
    }
}

fn poll_detach() {
    check_detach_slot(0);
    check_detach_slot(1);
}

fn execute_async(mut request: AsyncRequest) {
    let result = if request.kind != AsyncKind::Mount
        && slot_index(request.chn)
            .is_some_and(|slot| request.generation != current_generation(slot) as u32)
    {
        CARD_ERROR_CANCELED
    } else {
        match request.kind {
            AsyncKind::Mount => mount_inner(request.chn, request.workarea, request.detach_cb),
            AsyncKind::Read => read(request.file, request.buffer, request.len, request.offset),
            AsyncKind::Write => write(request.file, request.buffer, request.len, request.offset),
            AsyncKind::Create => create(
                request.chn,
                request.filename.as_ptr().cast(),
                request.size,
                request.file,
            ),
            AsyncKind::CreateEntry => {
                create_entry(request.chn, &mut request.direntry, request.file)
            }
            AsyncKind::Delete => delete(request.chn, request.filename.as_ptr().cast()),
            AsyncKind::DeleteEntry => delete_entry(request.chn, &mut request.direntry),
            AsyncKind::Format => format_with_encoding(request.chn, request.encoding),
            AsyncKind::SetStatus => {
                let mut stats = unsafe { request.stats.assume_init() };
                set_status(request.chn, request.fileno, &mut stats)
            }
            AsyncKind::SetAttributes => set_attributes(request.chn, request.fileno, request.attr),
            AsyncKind::None => CARD_ERROR_FATAL_ERROR,
        }
    };
    record_operation_result(request.chn, result);
    finish_callback(request.chn, result, request.callback);
}

unsafe extern "C" fn async_worker(_arg: *mut c_void) -> *mut c_void {
    unsafe {
        let timeout = lwp::TimeSpec {
            tv_sec: 0,
            tv_nsec: 100_000_000,
        };
        loop {
            let mutex = ASYNC_MUTEX;
            let cond = ASYNC_COND;
            let _ = lwp::mutex_lock(mutex);
            while ASYNC_LEN == 0 && !ASYNC_STOP.load(Ordering::Acquire) {
                let wait = lwp::cond_timed_wait(cond, mutex, &timeout);
                if wait != lwp::LWP_SUCCESSFUL {
                    break;
                }
            }
            if ASYNC_LEN == 0 && ASYNC_STOP.load(Ordering::Acquire) {
                let _ = lwp::mutex_unlock(mutex);
                return core::ptr::null_mut();
            }
            let request = if ASYNC_LEN == 0 {
                AsyncRequest::empty()
            } else {
                let request = ASYNC_QUEUE[ASYNC_HEAD];
                ASYNC_QUEUE[ASYNC_HEAD] = AsyncRequest::empty();
                ASYNC_HEAD = (ASYNC_HEAD + 1) % ASYNC_QUEUE_CAPACITY;
                ASYNC_LEN -= 1;
                request
            };
            let _ = lwp::mutex_unlock(mutex);
            poll_detach();
            if request.kind != AsyncKind::None {
                execute_async(request);
            }
        }
    }
}
fn ensure_async_worker() -> bool {
    if ASYNC_READY.load(Ordering::Acquire) {
        return true;
    }
    if ASYNC_STARTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        while ASYNC_STARTING.load(Ordering::Acquire) {
            lwp::yield_thread();
        }
        return ASYNC_READY.load(Ordering::Acquire);
    }
    ASYNC_STOP.store(false, Ordering::Release);
    let mut ok = true;
    unsafe {
        ok &= lwp::mutex_init(core::ptr::addr_of_mut!(ASYNC_MUTEX), 0) == lwp::LWP_SUCCESSFUL;
        ok &= lwp::cond_init(core::ptr::addr_of_mut!(ASYNC_COND)) == lwp::LWP_SUCCESSFUL;
        if ok {
            ok &= lwp::create_thread(
                core::ptr::addr_of_mut!(ASYNC_THREAD),
                Some(async_worker),
                core::ptr::null_mut(),
                core::ptr::addr_of_mut!(ASYNC_STACK.0).cast(),
                ASYNC_WORKER_STACK_SIZE as u32,
                ASYNC_WORKER_PRIORITY,
            ) == lwp::LWP_SUCCESSFUL;
        }
    }
    ASYNC_READY.store(ok, Ordering::Release);
    ASYNC_STARTING.store(false, Ordering::Release);
    ok
}

fn submit_async(mut request: AsyncRequest) -> i32 {
    if !ensure_async_worker() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if let Some(slot) = slot_index(request.chn) {
        request.generation = current_generation(slot) as u32;
    }
    unsafe {
        let mutex = ASYNC_MUTEX;
        let cond = ASYNC_COND;
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            return CARD_ERROR_FATAL_ERROR;
        }
        if channel_busy(request.chn) {
            let _ = lwp::mutex_unlock(mutex);
            return CARD_ERROR_BUSY;
        }
        if ASYNC_LEN == ASYNC_QUEUE_CAPACITY {
            let _ = lwp::mutex_unlock(mutex);
            return CARD_ERROR_BUSY;
        }
        let tail = (ASYNC_HEAD + ASYNC_LEN) % ASYNC_QUEUE_CAPACITY;
        ASYNC_QUEUE[tail] = request;
        ASYNC_LEN += 1;
        mark_operation_busy(request.chn);
        let _ = lwp::cond_signal(cond);
        let _ = lwp::mutex_unlock(mutex);
    }
    CARD_ERROR_READY
}

static SLOT_LOCKED: [AtomicBool; 3] = [const { AtomicBool::new(false) }; 3];
static SLOT_PRESENT: [AtomicBool; 2] = [const { AtomicBool::new(false) }; 2];
static SLOT_MOUNTED: [AtomicBool; 2] = [const { AtomicBool::new(false) }; 2];
static SLOT_WORKAREA: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];
static SLOT_ERROR: [AtomicI32; 2] = [const { AtomicI32::new(CARD_ERROR_NOCARD) }; 2];
static SLOT_ID: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_SECTOR_SIZE: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_BLOCK_COUNT: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_LATENCY: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_FREE_BLOCKS: [AtomicU16; 2] = [const { AtomicU16::new(0) }; 2];
static SLOT_SERIAL1: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_SERIAL2: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
static SLOT_WRITE_TOKEN: [AtomicU32; 2] = [const { AtomicU32::new(1) }; 2];
static SLOT_DIR_TARGET: [AtomicUsize; 2] = [const { AtomicUsize::new(1) }; 2];
static SLOT_FAT_TARGET: [AtomicUsize; 2] = [const { AtomicUsize::new(1) }; 2];
static SLOT_METADATA_BUSY: [AtomicBool; 2] = [const { AtomicBool::new(false) }; 2];

#[repr(align(32))]
struct MetadataReadback([u8; METADATA_BLOCK_SIZE]);

static mut METADATA_READBACK: [MetadataReadback; 2] = [
    MetadataReadback([0; METADATA_BLOCK_SIZE]),
    MetadataReadback([0; METADATA_BLOCK_SIZE]),
];

struct MetadataGuard {
    slot: usize,
}

impl Drop for MetadataGuard {
    fn drop(&mut self) {
        SLOT_METADATA_BUSY[self.slot].store(false, Ordering::Release);
    }
}

fn lock_metadata(slot: usize) -> Result<MetadataGuard, i32> {
    SLOT_METADATA_BUSY[slot]
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| MetadataGuard { slot })
        .map_err(|_| CARD_ERROR_BUSY)
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CardHeader {
    serial: [u32; 8],
    device_id: u16,
    size: u16,
    encoding: u16,
    padding: [u8; 0x1d6],
    chksum1: u16,
    chksum2: u16,
}

impl Default for CardHeader {
    fn default() -> Self {
        Self {
            serial: [0; 8],
            device_id: 0,
            size: 0,
            encoding: 0,
            padding: [0; 0x1d6],
            chksum1: 0,
            chksum2: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CardDirTable {
    entries: [CardDirEntry; CARD_MAXFILES],
}

impl Default for CardDirTable {
    fn default() -> Self {
        Self {
            entries: [CardDirEntry::default(); CARD_MAXFILES],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CardBat {
    chksum1: u16,
    chksum2: u16,
    updated: u16,
    freeblocks: u16,
    lastalloc: u16,
    fat: [u16; 0xffb],
}

impl Default for CardBat {
    fn default() -> Self {
        Self {
            chksum1: 0,
            chksum2: 0,
            updated: 0,
            freeblocks: 0,
            lastalloc: 0,
            fat: [0; 0xffb],
        }
    }
}

#[inline(always)]
fn reg_ptr(index: usize) -> *mut u32 {
    (EXI_REG_BASE as *mut u32).wrapping_add(index)
}

#[inline(always)]
unsafe fn read_reg(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(reg_ptr(index)) }
}

#[inline(always)]
unsafe fn write_reg(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(reg_ptr(index), value);
    }
}

#[inline(always)]
fn slot_index(chn: i32) -> Option<usize> {
    match chn {
        CARD_SLOTA => Some(0),
        CARD_SLOTB => Some(1),
        _ => None,
    }
}

fn workarea(slot: usize) -> Option<*mut u8> {
    let ptr = SLOT_WORKAREA[slot].load(Ordering::Acquire);
    (ptr != 0).then_some(ptr as *mut u8)
}

#[inline(always)]
fn chan_valid(chn: i32) -> bool {
    (CARD_SLOTA..=CARD_SLOTB).contains(&chn)
}

#[inline(always)]
fn rotl32(value: u32, shift: u32) -> u32 {
    value.rotate_left(shift & 31)
}

fn card_geometry_from_id(id: u32) -> Option<(u32, u32)> {
    let card_type = id & 0xfc;
    let idx = (rotl32(id, 23) & 0x1c) as usize;
    let sector_size = match idx >> 2 {
        0 => 0x2000,
        1 => 0x4000,
        2 => 0x8000,
        3 => 0x10000,
        4 => 0x20000,
        5 => 0x40000,
        _ => 0,
    };
    if sector_size == 0 {
        return None;
    }
    match card_type {
        EXI_MEMCARD59 | EXI_MEMCARD123 | EXI_MEMCARD251 | EXI_MEMCARD507 | EXI_MEMCARD1019
        | EXI_MEMCARD2043 => {
            let blocks = ((card_type << 20) >> 3) / sector_size;
            if blocks > 8 {
                Some((sector_size, blocks))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn set_error(slot: usize, err: i32) {
    SLOT_ERROR[slot].store(err, Ordering::Release);
}

fn record_operation_result(chn: i32, result: i32) {
    if let Some(slot) = slot_index(chn) {
        // Match __card_putcntrlblock: an unattached card keeps its prior result unless busy.
        if SLOT_PRESENT[slot].load(Ordering::Acquire)
            || SLOT_ERROR[slot].load(Ordering::Acquire) == CARD_ERROR_BUSY
        {
            set_error(slot, result);
        }
    }
}

fn mark_operation_busy(chn: i32) {
    if let Some(slot) = slot_index(chn) {
        set_error(slot, CARD_ERROR_BUSY);
    }
}

fn channel_busy(chn: i32) -> bool {
    slot_index(chn).is_some_and(|slot| SLOT_ERROR[slot].load(Ordering::Acquire) == CARD_ERROR_BUSY)
}

fn lock_channel(chn: i32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    SLOT_LOCKED[slot]
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

fn unlock_channel(chn: i32) {
    if let Some(slot) = slot_index(chn) {
        SLOT_LOCKED[slot].store(false, Ordering::Release);
    }
}

fn select_raw(chn: i32, dev: i32, frq: i32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    let value = unsafe { read_reg(slot * 5) };
    let value = (value & EXI_SELECT_MASK) | (0x80u32 << (dev as u32)) | ((frq as u32) << 4);
    unsafe {
        write_reg(slot * 5, value);
    }
    true
}

fn dma_raw(chn: i32, data: *mut c_void, len: u32, mode: u32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    if data.is_null() || len == 0 {
        return false;
    }
    unsafe {
        write_reg(slot * 5 + 1, (data as usize as u32) & 0x03ff_ffe0);
        write_reg(slot * 5 + 2, len);
        write_reg(slot * 5 + 3, ((mode & 0x3) << 2) | 0x03);
        while (read_reg(slot * 5 + 3) & 1) != 0 {}
    }
    true
}

#[inline]
fn dma_with_cache_maintenance(
    buffer: *mut u8,
    len: u32,
    invalidate: impl FnOnce(*mut u8, u32),
    dma: impl FnOnce() -> bool,
) -> bool {
    invalidate(buffer, len);
    dma()
}

fn deselect_raw(chn: i32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    let value = unsafe { read_reg(slot * 5) };
    unsafe {
        write_reg(slot * 5, value & EXI_SELECT_MASK);
    }
    true
}

fn sync_raw(chn: i32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    let ctrl = unsafe { read_reg(slot * 5 + 3) };
    if (ctrl & 1) != 0 {
        unsafe {
            write_reg(slot * 5 + 3, ctrl & !1);
        }
    }
    true
}

fn latency_from_id(id: u32) -> u32 {
    let idx = (rotl32(id, 26) & 0x1c) as usize;
    match idx >> 2 {
        0 => 0x0000_0004,
        1 => 0x0000_0008,
        2 => 0x0000_0010,
        3 => 0x0000_0020,
        4 => 0x0000_0040,
        5 => 0x0000_0080,
        6 => 0x0000_0100,
        _ => 0x0000_0200,
    }
}

fn read_segment(chn: i32, address: u32, buffer: *mut u8, len: u32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    if buffer.is_null() || len == 0 {
        return false;
    }
    let mut cmd = [0u8; 5];
    cmd[0] = 0x52;
    cmd[1] = ((address & 0x00fe_0000) >> 17) as u8;
    cmd[2] = ((address & 0x0001_fe00) >> 9) as u8;
    cmd[3] = ((address & 0x0000_0180) >> 7) as u8;
    cmd[4] = (address & 0x0000_007f) as u8;
    let mut dummy = [0u8; 256];
    let latency = SLOT_LATENCY[slot]
        .load(Ordering::Acquire)
        .min(dummy.len() as u32) as usize;
    if !lock_channel(chn) {
        return false;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED16MHZ) {
        unlock_channel(chn);
        return false;
    }
    let ok = imm_raw(chn, cmd.as_mut_ptr().cast(), cmd.len() as u32, EXI_WRITE)
        && imm_raw(chn, dummy.as_mut_ptr().cast(), latency as u32, EXI_WRITE)
        && dma_with_cache_maintenance(
            buffer,
            len,
            |buffer, len| unsafe { dc_invalidate_range(buffer.cast(), len) },
            || dma_raw(chn, buffer.cast(), len, EXI_READ),
        );
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    ok
}

#[cfg(test)]
mod dma_cache_tests {
    use super::dma_with_cache_maintenance;
    use core::cell::Cell;

    #[test]
    fn maintains_the_dma_buffer_before_transfer() {
        let buffer = 0x1000usize as *mut u8;
        let invalidated = Cell::new(false);
        let dma_started = Cell::new(false);

        let result = dma_with_cache_maintenance(
            buffer,
            512,
            |actual_buffer, len| {
                assert_eq!(actual_buffer, buffer);
                assert_eq!(len, 512);
                invalidated.set(true);
            },
            || {
                assert!(invalidated.get());
                dma_started.set(true);
                true
            },
        );

        assert!(result);
        assert!(invalidated.get());
        assert!(dma_started.get());
    }
}

fn read_range(chn: i32, address: u32, buffer: *mut u8, len: u32) -> bool {
    if buffer.is_null() || len == 0 || (len & 0x1ff) != 0 {
        return false;
    }
    let mut offset = 0u32;
    while offset < len {
        let chunk = 0x200u32;
        if !read_segment(
            chn,
            address.wrapping_add(offset),
            unsafe { buffer.add(offset as usize) },
            chunk,
        ) {
            return false;
        }
        offset = offset.wrapping_add(chunk);
    }
    true
}

fn write_segment(chn: i32, address: u32, buffer: *mut u8, len: u32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    if buffer.is_null() || len == 0 {
        return false;
    }
    let mut cmd = [0u8; 5];
    cmd[0] = 0xf2;
    cmd[1] = ((address & 0x00fe_0000) >> 17) as u8;
    cmd[2] = ((address & 0x0001_fe00) >> 9) as u8;
    cmd[3] = ((address & 0x0000_0180) >> 7) as u8;
    cmd[4] = (address & 0x0000_007f) as u8;
    if !lock_channel(chn) {
        return false;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED16MHZ) {
        unlock_channel(chn);
        return false;
    }
    let ok = imm_raw(chn, cmd.as_mut_ptr().cast(), cmd.len() as u32, EXI_WRITE)
        && dma_with_cache_maintenance(
            buffer,
            len,
            |buffer, len| unsafe { dc_store_range(buffer.cast(), len) },
            || dma_raw(chn, buffer.cast(), len, EXI_WRITE),
        );
    let _ = clear_status(chn);
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    if ok {
        SLOT_WRITE_TOKEN[slot].fetch_add(1, Ordering::AcqRel);
    }
    ok
}

fn write_range(chn: i32, address: u32, buffer: *mut u8, len: u32) -> bool {
    if buffer.is_null() || len == 0 || (len & 0x7f) != 0 {
        return false;
    }
    let mut offset = 0u32;
    while offset < len {
        if !write_segment(
            chn,
            address.wrapping_add(offset),
            unsafe { buffer.add(offset as usize) },
            128,
        ) {
            return false;
        }
        offset = offset.wrapping_add(128);
    }
    true
}

fn sector_erase(chn: i32, sector: u32) -> bool {
    if sector & 0x7f != 0 {
        return false;
    }
    let Some(_) = slot_index(chn) else {
        return false;
    };
    if !lock_channel(chn) {
        return false;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED16MHZ) {
        unlock_channel(chn);
        return false;
    }
    let mut cmd = [
        0xf1u8,
        ((sector >> 17) & 0x7f) as u8,
        ((sector >> 9) & 0xff) as u8,
    ];
    let ok = imm_raw(chn, cmd.as_mut_ptr().cast(), cmd.len() as u32, EXI_WRITE);
    let _ = clear_status(chn);
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    ok
}

fn workarea_base(slot: usize) -> Option<*mut u8> {
    workarea(slot)
}

fn system_block_ptr(slot: usize, block: usize) -> Option<*mut u8> {
    (block < 5)
        .then(|| workarea_base(slot))
        .flatten()
        .map(|ptr| unsafe { ptr.add(block * METADATA_BLOCK_SIZE) })
}

fn dirblock_ptr(slot: usize, copy: usize) -> Option<*mut CardDirTable> {
    (copy < 2)
        .then(|| system_block_ptr(slot, copy + 1))
        .flatten()
        .map(|ptr| ptr.cast())
}

fn fatblock_ptr(slot: usize, copy: usize) -> Option<*mut CardBat> {
    (copy < 2)
        .then(|| system_block_ptr(slot, copy + 3))
        .flatten()
        .map(|ptr| ptr.cast())
}

fn header_ptr(slot: usize) -> Option<*mut CardHeader> {
    workarea_base(slot).map(|ptr| ptr.cast())
}

fn block_bytes<'a>(ptr: *const u8) -> Option<&'a [u8]> {
    (!ptr.is_null()).then(|| unsafe { core::slice::from_raw_parts(ptr, METADATA_BLOCK_SIZE) })
}

fn block_bytes_mut<'a>(ptr: *mut u8) -> Option<&'a mut [u8]> {
    (!ptr.is_null()).then(|| unsafe { core::slice::from_raw_parts_mut(ptr, METADATA_BLOCK_SIZE) })
}

fn card_fat_valid(slot: usize, table: *const CardBat) -> bool {
    let blocks = SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire);
    let Ok(blocks) = u16::try_from(blocks) else {
        return false;
    };
    block_bytes(table.cast()).is_some_and(|block| metadata::validate_fat(block, blocks).is_ok())
}

fn mounted_ok(slot: usize) -> bool {
    SLOT_MOUNTED[slot].load(Ordering::Acquire) && workarea(slot).is_some()
}

fn current_dir(slot: usize) -> Option<*mut CardDirTable> {
    dirblock_ptr(slot, SLOT_DIR_TARGET[slot].load(Ordering::Acquire))
}

fn current_fat(slot: usize) -> Option<*mut CardBat> {
    fatblock_ptr(slot, SLOT_FAT_TARGET[slot].load(Ordering::Acquire))
}

fn current_dir_back(slot: usize) -> Option<*mut CardDirTable> {
    dirblock_ptr(slot, SLOT_DIR_TARGET[slot].load(Ordering::Acquire) ^ 1)
}

fn current_fat_back(slot: usize) -> Option<*mut CardBat> {
    fatblock_ptr(slot, SLOT_FAT_TARGET[slot].load(Ordering::Acquire) ^ 1)
}

fn entry_used(entry: &CardDirEntry) -> bool {
    entry.gamecode[0] != 0xff && entry.filename[0] != 0xff
}

fn entry_matches_game(_slot: usize, entry: &CardDirEntry, showall: Bool) -> bool {
    if showall != 0 {
        return true;
    }
    let gamecode = current_gamecode();
    let company = current_company();
    (gamecode[0] == 0xff || entry.gamecode == gamecode)
        && (company[0] == 0xff || entry.company == company)
}

fn find_file_index(slot: usize, filename: *const core::ffi::c_char) -> Option<usize> {
    let dir = current_dir(slot)?;
    if filename.is_null() {
        return None;
    }
    let dir = unsafe { &*dir };
    dir.entries
        .iter()
        .enumerate()
        .find(|(_, entry)| {
            entry_used(entry)
                && entry_matches_game(slot, entry, 0)
                && entry_name_matches(entry, filename)
        })
        .map(|(idx, _)| idx)
}

fn find_file_index_for_entry(
    slot: usize,
    filename: &[u8],
    gamecode: &[u8; 4],
    company: &[u8; 2],
) -> Option<usize> {
    let dir = current_dir(slot)?;
    let dir = unsafe { &*dir };
    dir.entries
        .iter()
        .enumerate()
        .find(|(_, entry)| {
            entry_used(entry)
                && entry_matches_game(slot, entry, 1)
                && entry_name_matches_bytes(entry, filename)
                && (gamecode[0] == 0xff || entry.gamecode == *gamecode)
                && (company[0] == 0xff || entry.company == *company)
        })
        .map(|(idx, _)| idx)
}

fn raw_dir_entry_at(slot: usize, fileno: i32) -> Option<CardDirEntry> {
    let dir = current_dir(slot)?;
    if !valid_file_number(fileno) {
        return None;
    }
    Some(unsafe { (*dir).entries[fileno as usize] })
}

fn find_matching_index(slot: usize, start: usize, showall: Bool) -> Option<usize> {
    let dir = current_dir(slot)?;
    let dir = unsafe { &*dir };
    let gamecode = current_gamecode();
    let company = current_company();
    dir.entries
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, entry)| {
            entry_used(entry) && entry_matches_find(entry, &gamecode, &company, showall)
        })
        .map(|(idx, _)| idx)
}

fn find_directory_matching_index(slot: usize, start: usize, showall: Bool) -> Option<usize> {
    let dir = current_dir(slot)?;
    let dir = unsafe { &*dir };
    let gamecode = current_gamecode();
    let company = current_company();
    dir.entries
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, entry)| {
            entry_used(entry) && entry_matches_directory(entry, &gamecode, &company, showall)
        })
        .map(|(idx, _)| idx)
}

fn entry_matches_find(
    entry: &CardDirEntry,
    gamecode: &[u8; 4],
    company: &[u8; 2],
    showall: Bool,
) -> bool {
    showall != 0 || (entry.gamecode == *gamecode && entry.company == *company)
}

fn entry_matches_directory(
    entry: &CardDirEntry,
    gamecode: &[u8; 4],
    company: &[u8; 2],
    showall: Bool,
) -> bool {
    showall != 0
        || (gamecode[0] != 0xff
            && company[0] != 0xff
            && entry.gamecode == *gamecode
            && entry.company == *company)
}

fn dir_filename_len(filename: &[u8; CARD_FILENAMELEN]) -> usize {
    filename
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(CARD_FILENAMELEN)
}

fn fill_dir_fields(dir: *mut CardDir, idx: usize, entry: CardDirEntry, sector_size: u32) {
    // BUG FIX: C list APIs update only these shared fields; each caller owns its remaining fields.
    unsafe {
        (*dir).fileno = idx as u32;
        (*dir).filelen = entry.length as u32 * sector_size;
        (*dir).filename = entry.filename;
        (*dir).gamecode = entry.gamecode;
        (*dir).company = entry.company;
    }
}

fn fill_dir(slot: usize, idx: usize, dir: *mut CardDir) -> Option<u8> {
    let table = current_dir(slot)?;
    let entry = unsafe { (*table).entries[idx] };
    fill_dir_fields(
        dir,
        idx,
        entry,
        SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire),
    );
    Some(entry.permission)
}

fn update_icon_offsets(entry: &CardDirEntry, stats: &mut CardStat) {
    let mut iconaddr = entry.icon_addr;
    if iconaddr == u32::MAX {
        stats.banner_fmt = 0;
        stats.icon_fmt = 0;
        stats.icon_speed = 0;
        iconaddr = 0;
    }

    if (entry.banner_fmt & CARD_BANNER_MASK) != 0 {
        if (entry.banner_fmt & 0x10) == 0 {
            let banner_fmt = entry.banner_fmt & CARD_BANNER_MASK;
            if banner_fmt == CARD_BANNER_CI {
                stats.banner_fmt = banner_fmt;
                stats.offset_banner = iconaddr;
                stats.offset_banner_tlut = iconaddr.wrapping_add(3072);
                iconaddr = iconaddr.wrapping_add(3072 + 512);
            } else if banner_fmt == CARD_BANNER_RGB {
                stats.banner_fmt = banner_fmt;
                stats.offset_banner = iconaddr;
                stats.offset_banner_tlut = u32::MAX;
                iconaddr = iconaddr.wrapping_add(6144);
            }
        } else {
            stats.offset_banner = u32::MAX;
            stats.offset_banner_tlut = u32::MAX;
        }
    }

    let mut nicons = 0u32;
    let icon_base = iconaddr;
    for i in 0..CARD_MAXICONS {
        stats.iconfmt[i] = (entry.icon_fmt >> (i << 1)) & CARD_ICON_MASK as u16;
        stats.iconspeed[i] = (entry.icon_speed >> (i << 1)) & CARD_SPEED_MASK as u16;
        if stats.iconspeed[i] == 0 {
            stats.iconfmt[i] = 0;
        }
        if stats.iconfmt[i] != 0 {
            nicons += 1;
        }
    }

    for i in 0..CARD_MAXICONS {
        match stats.iconfmt[i] {
            1 => {
                stats.offset_icon[i] = iconaddr;
                stats.offset_icon_tlut[i] = icon_base.wrapping_add(nicons * 1024);
                iconaddr = iconaddr.wrapping_add(1024);
            }
            2 => {
                stats.offset_icon[i] = iconaddr;
                stats.offset_icon_tlut[i] = u32::MAX;
                iconaddr = iconaddr.wrapping_add(3072);
            }
            3 => {
                stats.offset_icon[i] = iconaddr;
                stats.offset_icon_tlut[i] = iconaddr.wrapping_add(1024);
                iconaddr = iconaddr.wrapping_add(1536);
            }
            _ => {
                stats.offset_icon[i] = u32::MAX;
                stats.offset_icon_tlut[i] = u32::MAX;
            }
        }
    }
}

fn apply_status_update(entry: &mut CardDirEntry, stats: &mut CardStat, last_modified: u32) {
    entry.banner_fmt = stats.banner_fmt;
    entry.icon_addr = stats.icon_addr;
    entry.icon_fmt = stats.icon_fmt;
    entry.icon_speed = stats.icon_speed;
    entry.comment_addr = stats.comment_addr;
    if entry.icon_addr == u32::MAX {
        entry.icon_speed = (entry.icon_speed & !(CARD_SPEED_MASK as u16)) | CARD_SPEED_FAST as u16;
    }
    entry.last_modified = last_modified;
    update_icon_offsets(entry, stats);
}

fn apply_status_ex_update(entry: &mut CardDirEntry, update: &CardDirEntry) {
    if entry.filename != update.filename
        || entry.gamecode != update.gamecode
        || entry.company != update.company
    {
        entry.filename = update.filename;
        entry.gamecode = update.gamecode;
        entry.company = update.company;
    }
    entry.last_modified = update.last_modified;
    entry.banner_fmt = update.banner_fmt;
    entry.icon_addr = update.icon_addr;
    entry.icon_fmt = update.icon_fmt;
    entry.icon_speed = update.icon_speed;
    entry.comment_addr = update.comment_addr;
    entry.permission = update.permission;
    entry.copy_times = update.copy_times;
}

fn fill_stat(slot: usize, entry: &CardDirEntry, stats: *mut CardStat) {
    unsafe {
        (*stats).filename = entry.filename;
        (*stats).len = (entry.length as u32) * SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
        (*stats).time = entry.last_modified;
        (*stats).gamecode = entry.gamecode;
        (*stats).company = entry.company;
        (*stats).banner_fmt = entry.banner_fmt;
        (*stats).icon_addr = entry.icon_addr;
        (*stats).icon_fmt = entry.icon_fmt;
        (*stats).icon_speed = entry.icon_speed;
        (*stats).comment_addr = entry.comment_addr;
        update_icon_offsets(entry, &mut *stats);
    }
}

fn seek_file(
    file: *mut CardFile,
    slot: usize,
    len: u32,
    offset: u32,
) -> core::result::Result<(u16, u32), i32> {
    let fileno = unsafe { (*file).filenum };
    let Some(entry) = raw_dir_entry_at(slot, fileno) else {
        return Err(CARD_ERROR_FATAL_ERROR);
    };
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0
        || !valid_file_block(entry.block, SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire))
    {
        return Err(CARD_ERROR_BROKEN);
    }
    let entry_len = u32::from(entry.length).saturating_mul(sector_size);
    if file_range_exceeds_len(entry_len, len, offset) {
        return Err(CARD_ERROR_LIMIT);
    }

    let mut block = unsafe { (*file).iblock };
    let mut cursor = unsafe { (*file).offset.max(0) as u32 };
    if !valid_file_block(block, SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire)) {
        return Err(CARD_ERROR_FATAL_ERROR);
    }
    if offset < cursor {
        block = entry.block;
        cursor = 0;
    }
    let target = offset & !(sector_size - 1);
    while cursor < target {
        block = file_block_next(slot, block).ok_or(CARD_ERROR_BROKEN)?;
        cursor = cursor.saturating_add(sector_size);
    }

    unsafe {
        (*file).offset = offset as i32;
        (*file).len = len as i32;
        (*file).iblock = block;
    }
    Ok((block, offset % sector_size))
}

fn refresh_serials_from_header(slot: usize) {
    let Some(header) = header_ptr(slot) else {
        return;
    };
    let header = unsafe { &*header };
    let serial1 = header.serial[0] ^ header.serial[2] ^ header.serial[4] ^ header.serial[6];
    let serial2 = header.serial[1] ^ header.serial[3] ^ header.serial[5] ^ header.serial[7];
    SLOT_SERIAL1[slot].store(serial1, Ordering::Release);
    SLOT_SERIAL2[slot].store(serial2, Ordering::Release);
}

fn configure_metadata_copies(slot: usize) -> Result<(), i32> {
    let Some(header) = system_block_ptr(slot, 0).and_then(|ptr| block_bytes(ptr)) else {
        return Err(CARD_ERROR_BROKEN);
    };
    metadata::validate_header(header).map_err(|_| CARD_ERROR_BROKEN)?;

    let dir_primary = dirblock_ptr(slot, 0).ok_or(CARD_ERROR_BROKEN)?;
    let dir_backup = dirblock_ptr(slot, 1).ok_or(CARD_ERROR_BROKEN)?;
    let dir_selection = metadata::select_copy(
        block_bytes(dir_primary.cast())
            .ok_or(metadata::MetadataError::InvalidLength)
            .and_then(metadata::validate_directory),
        block_bytes(dir_backup.cast())
            .ok_or(metadata::MetadataError::InvalidLength)
            .and_then(metadata::validate_directory),
    )
    .map_err(|_| CARD_ERROR_BROKEN)?;
    let dir_newest = dirblock_ptr(slot, dir_selection.newest.index()).ok_or(CARD_ERROR_BROKEN)?;
    let dir_target =
        dirblock_ptr(slot, dir_selection.next_target.index()).ok_or(CARD_ERROR_BROKEN)?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            dir_newest.cast::<u8>(),
            dir_target.cast::<u8>(),
            METADATA_BLOCK_SIZE,
        );
    }
    SLOT_DIR_TARGET[slot].store(dir_selection.next_target.index(), Ordering::Release);

    let blocks = u16::try_from(SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire))
        .map_err(|_| CARD_ERROR_BROKEN)?;
    let fat_primary = fatblock_ptr(slot, 0).ok_or(CARD_ERROR_BROKEN)?;
    let fat_backup = fatblock_ptr(slot, 1).ok_or(CARD_ERROR_BROKEN)?;
    let validate_fat = |ptr: *const CardBat| {
        block_bytes(ptr.cast())
            .ok_or(metadata::MetadataError::InvalidLength)
            .and_then(|block| metadata::validate_fat(block, blocks))
    };
    let fat_selection = metadata::select_copy(validate_fat(fat_primary), validate_fat(fat_backup))
        .map_err(|_| CARD_ERROR_BROKEN)?;
    let fat_newest = fatblock_ptr(slot, fat_selection.newest.index()).ok_or(CARD_ERROR_BROKEN)?;
    let fat_target =
        fatblock_ptr(slot, fat_selection.next_target.index()).ok_or(CARD_ERROR_BROKEN)?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            fat_newest.cast::<u8>(),
            fat_target.cast::<u8>(),
            METADATA_BLOCK_SIZE,
        );
    }
    SLOT_FAT_TARGET[slot].store(fat_selection.next_target.index(), Ordering::Release);
    Ok(())
}

fn reload_metadata(slot: usize) -> Result<(), i32> {
    let chn = if slot == 0 { CARD_SLOTA } else { CARD_SLOTB };
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0 {
        return Err(CARD_ERROR_BROKEN);
    }
    for index in 0..5 {
        let target = system_block_ptr(slot, index).ok_or(CARD_ERROR_BROKEN)?;
        if !read_range(
            chn,
            (index as u32).wrapping_mul(sector_size),
            target,
            METADATA_BLOCK_SIZE as u32,
        ) {
            return Err(if SLOT_PRESENT[slot].load(Ordering::Acquire) {
                CARD_ERROR_IOERROR
            } else {
                CARD_ERROR_NOCARD
            });
        }
    }
    configure_metadata_copies(slot)
}

fn allocate_blocks(slot: usize, blocksneed: u32) -> Option<u16> {
    let fat = current_fat(slot)?;
    if !card_fat_valid(slot, fat) {
        return None;
    }
    unsafe {
        if (*fat).freeblocks < blocksneed as u16 {
            return None;
        }
    }
    let block_count = SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire);
    let mut allocated = 0u32;
    let mut first = 0xffffu16;
    let mut prev = 0xffffu16;
    let mut curr = unsafe { (*fat).lastalloc };
    let mut scans = 0u32;
    while allocated < blocksneed {
        scans = scans.wrapping_add(1);
        if scans > block_count.saturating_sub(5) {
            return None;
        }
        curr = curr.wrapping_add(1);
        if curr < 5 || u32::from(curr) >= block_count {
            curr = 5;
        }
        let idx = usize::from(curr.saturating_sub(5));
        unsafe {
            if (*fat).fat[idx] == 0 {
                if first == 0xffff {
                    first = curr;
                } else {
                    (*fat).fat[usize::from(prev - 5)] = curr;
                }
                (*fat).fat[idx] = 0xffff;
                prev = curr;
                allocated += 1;
            }
        }
    }
    unsafe {
        (*fat).freeblocks = (*fat).freeblocks.saturating_sub(blocksneed as u16);
        (*fat).lastalloc = prev;
    }
    first.into()
}

fn free_chain(slot: usize, start: u16) -> bool {
    let Some(fat) = current_fat(slot) else {
        return false;
    };
    if !card_fat_valid(slot, fat) || start < 5 {
        return false;
    }
    let mut block = start;
    loop {
        let idx = usize::from(block - 5);
        let next = unsafe { (*fat).fat[idx] };
        unsafe {
            (*fat).fat[idx] = 0;
            (*fat).freeblocks = (*fat).freeblocks.saturating_add(1);
        }
        if next == 0 || next == 0xffff {
            break;
        }
        if next < 5 {
            return false;
        }
        block = next;
    }
    true
}

fn file_block_next(slot: usize, block: u16) -> Option<u16> {
    let fat = current_fat(slot)?;
    if !card_fat_valid(slot, fat) {
        return None;
    }
    let idx = usize::from(block.saturating_sub(5));
    unsafe {
        let next = (*fat).fat[idx];
        if next == 0 || next == 0xffff {
            None
        } else {
            Some(next)
        }
    }
}

fn read_file_chain(
    chn: i32,
    slot: usize,
    mut block: u16,
    mut offset: u32,
    mut len: u32,
    buffer: *mut u8,
) -> i32 {
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0 || buffer.is_null() {
        return CARD_ERROR_BROKEN;
    }
    let mut out_off = 0usize;
    while len > 0 {
        if block < 5 {
            return CARD_ERROR_BROKEN;
        }
        let block_addr = u32::from(block).wrapping_mul(sector_size);
        let block_off = offset.min(sector_size);
        let remain_in_block = sector_size.saturating_sub(block_off);
        let take = remain_in_block.min(len);
        if !read_range(
            chn,
            block_addr.wrapping_add(block_off),
            unsafe { buffer.add(out_off) },
            take,
        ) {
            return CARD_ERROR_NOCARD;
        }
        out_off += take as usize;
        len -= take;
        offset = 0;
        if len > 0 {
            let Some(next) = file_block_next(slot, block) else {
                return CARD_ERROR_BROKEN;
            };
            block = next;
        }
    }
    CARD_ERROR_READY
}

fn imm_raw(chn: i32, data: *mut c_void, len: u32, mode: u32) -> bool {
    let Some(slot) = slot_index(chn) else {
        return false;
    };
    if len == 0 || len > 4 {
        return false;
    }
    if mode != EXI_READ && data.is_null() {
        return false;
    }
    unsafe {
        if mode != EXI_READ {
            let mut value = 0u32;
            let bytes = core::slice::from_raw_parts(data.cast::<u8>(), len as usize);
            for (idx, byte) in bytes.iter().enumerate() {
                value |= (*byte as u32) << ((3 - idx) * 8);
            }
            write_reg(slot * 5 + 4, value);
        }
        write_reg(
            slot * 5 + 3,
            (((len - 1) & 0x3) << 4) | ((mode & 0x3) << 2) | 0x01,
        );
        if mode == EXI_READ {
            let value = read_reg(slot * 5 + 4);
            let bytes = core::slice::from_raw_parts_mut(data.cast::<u8>(), len as usize);
            for (idx, byte) in bytes.iter_mut().enumerate() {
                *byte = (value >> ((3 - idx) * 8)) as u8;
            }
        }
    }
    sync_raw(chn)
}

fn exi_get_id(chn: i32) -> Option<u32> {
    if !chan_valid(chn) || !lock_channel(chn) {
        return None;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED1MHZ) {
        unlock_channel(chn);
        return None;
    }
    let mut reg = 0u32;
    let mut id = 0u32;
    let ok = imm_raw(chn, (&mut reg as *mut u32).cast(), 2, EXI_WRITE)
        && imm_raw(chn, (&mut id as *mut u32).cast(), 4, EXI_READ);
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    if ok { Some(id) } else { None }
}

fn read_status(chn: i32) -> Option<u8> {
    if !chan_valid(chn) || !lock_channel(chn) {
        return None;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED16MHZ) {
        unlock_channel(chn);
        return None;
    }
    let mut cmd = [0x83u8, 0x00];
    let mut status = 0u8;
    let ok = imm_raw(chn, cmd.as_mut_ptr().cast(), cmd.len() as u32, EXI_WRITE)
        && imm_raw(chn, (&mut status as *mut u8).cast(), 1, EXI_READ);
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    if ok { Some(status) } else { None }
}

fn clear_status(chn: i32) -> bool {
    if !chan_valid(chn) || !lock_channel(chn) {
        return false;
    }
    if !select_raw(chn, EXI_DEVICE_0, EXI_SPEED16MHZ) {
        unlock_channel(chn);
        return false;
    }
    let mut cmd = 0x89u8;
    let ok = imm_raw(chn, (&mut cmd as *mut u8).cast(), 1, EXI_WRITE);
    let _ = deselect_raw(chn);
    unlock_channel(chn);
    ok
}

pub fn init() -> i32 {
    if !ensure_async_worker() {
        return CARD_ERROR_FATAL_ERROR;
    }
    for slot in 0..2 {
        SLOT_PRESENT[slot].store(false, Ordering::Release);
        SLOT_MOUNTED[slot].store(false, Ordering::Release);
        SLOT_WORKAREA[slot].store(0, Ordering::Release);
        SLOT_ERROR[slot].store(CARD_ERROR_NOCARD, Ordering::Release);
        SLOT_ID[slot].store(0, Ordering::Release);
        SLOT_SECTOR_SIZE[slot].store(0, Ordering::Release);
        SLOT_BLOCK_COUNT[slot].store(0, Ordering::Release);
        SLOT_LATENCY[slot].store(0, Ordering::Release);
        SLOT_FREE_BLOCKS[slot].store(0, Ordering::Release);
    }
    CARD_ERROR_READY
}

pub fn probe(chn: i32) -> i32 {
    unsafe { probe_ex(chn, core::ptr::null_mut(), core::ptr::null_mut()) }
}

/// # Safety
///
/// `mem_size` and `sect_size` must either be null or valid writable pointers.
pub unsafe fn probe_ex(chn: i32, mem_size: *mut i32, sect_size: *mut i32) -> i32 {
    unsafe {
        let Some(slot) = slot_index(chn) else {
            return CARD_ERROR_FATAL_ERROR;
        };
        if let Some(result) = card_probe_error(ogc_exi::probe_ex(chn)) {
            if result == CARD_ERROR_NOCARD {
                SLOT_PRESENT[slot].store(false, Ordering::Release);
            }
            set_error(slot, result);
            return result;
        }
        let Some(id) = exi_get_id(chn) else {
            set_error(slot, CARD_ERROR_BUSY);
            return CARD_ERROR_BUSY;
        };
        let Some((sector_size, blocks)) = card_geometry_from_id(id) else {
            SLOT_PRESENT[slot].store(false, Ordering::Release);
            SLOT_ID[slot].store(id, Ordering::Release);
            set_error(slot, CARD_ERROR_WRONGDEVICE);
            return CARD_ERROR_WRONGDEVICE;
        };
        SLOT_PRESENT[slot].store(true, Ordering::Release);
        SLOT_ID[slot].store(id, Ordering::Release);
        SLOT_SECTOR_SIZE[slot].store(sector_size, Ordering::Release);
        SLOT_BLOCK_COUNT[slot].store(blocks, Ordering::Release);
        SLOT_LATENCY[slot].store(latency_from_id(id), Ordering::Release);
        if !mem_size.is_null() {
            *mem_size = (sector_size * blocks) as i32;
        }
        if !sect_size.is_null() {
            *sect_size = sector_size as i32;
        }
        set_error(slot, CARD_ERROR_READY);
        CARD_ERROR_READY
    }
}

fn card_probe_error(exi_result: i32) -> Option<i32> {
    match exi_result {
        1 => None,
        0 => Some(CARD_ERROR_BUSY),
        _ => Some(CARD_ERROR_NOCARD),
    }
}

pub fn get_error_code(chn: i32) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    SLOT_ERROR[slot].load(Ordering::Acquire)
}

pub unsafe fn get_sector_size(chn: i32, sector_size: *mut u32) -> i32 {
    unsafe {
        let Some(slot) = slot_index(chn) else {
            return CARD_ERROR_NOCARD;
        };
        if !SLOT_PRESENT[slot].load(Ordering::Acquire) {
            return CARD_ERROR_NOCARD;
        }
        if sector_size.is_null() {
            return CARD_ERROR_FATAL_ERROR;
        }
        *sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
        CARD_ERROR_READY
    }
}

pub unsafe fn get_block_count(chn: i32, block_count: *mut u32) -> i32 {
    unsafe {
        let Some(slot) = slot_index(chn) else {
            return CARD_ERROR_NOCARD;
        };
        if !SLOT_PRESENT[slot].load(Ordering::Acquire) {
            return CARD_ERROR_NOCARD;
        }
        if block_count.is_null() {
            return CARD_ERROR_FATAL_ERROR;
        }
        *block_count = SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire);
        CARD_ERROR_READY
    }
}

pub unsafe fn get_free_blocks(chn: i32, freeblocks: *mut u16) -> i32 {
    unsafe {
        let Some(slot) = slot_index(chn) else {
            return CARD_ERROR_NOCARD;
        };
        if freeblocks.is_null() {
            return CARD_ERROR_FATAL_ERROR;
        }
        if !mounted_ok(slot) {
            return CARD_ERROR_NOCARD;
        }
        let Some(fat) = current_fat(slot) else {
            return CARD_ERROR_BROKEN;
        };
        if !card_fat_valid(slot, fat) {
            return CARD_ERROR_BROKEN;
        }
        *freeblocks = (*fat).freeblocks;
        CARD_ERROR_READY
    }
}

fn mount_inner(chn: i32, workarea_ptr: *mut c_void, detach_cb: CardCallback) -> i32 {
    if workarea_ptr.is_null() {
        return CARD_ERROR_NOCARD;
    }
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if (workarea_ptr as usize) & 0x1f != 0 {
        set_error(slot, CARD_ERROR_FATAL_ERROR);
        return CARD_ERROR_FATAL_ERROR;
    }
    SLOT_WORKAREA[slot].store(workarea_ptr as usize, Ordering::Release);
    if !SLOT_PRESENT[slot].load(Ordering::Acquire) {
        let _ = unsafe { probe_ex(chn, core::ptr::null_mut(), core::ptr::null_mut()) };
    }
    if !SLOT_PRESENT[slot].load(Ordering::Acquire) {
        set_error(slot, CARD_ERROR_NOCARD);
        return CARD_ERROR_NOCARD;
    }

    let status = read_status(chn).unwrap_or(0);
    if (status & EXI_STATUS_UNLOCKED) == 0 {
        set_error(slot, CARD_ERROR_UNLOCKED);
        return CARD_ERROR_UNLOCKED;
    }

    let Some(base) = workarea(slot) else {
        set_error(slot, CARD_ERROR_NOCARD);
        return CARD_ERROR_NOCARD;
    };
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0 {
        set_error(slot, CARD_ERROR_BROKEN);
        return CARD_ERROR_BROKEN;
    }
    for idx in 0..5 {
        let dst = unsafe { base.add(idx * 8192) };
        let addr = (idx as u32).wrapping_mul(sector_size);
        if !read_range(chn, addr, dst, 8192) {
            set_error(slot, CARD_ERROR_NOCARD);
            return CARD_ERROR_NOCARD;
        }
    }
    if configure_metadata_copies(slot).is_err() {
        set_error(slot, CARD_ERROR_BROKEN);
        return CARD_ERROR_BROKEN;
    }
    let Some(fat) = current_fat(slot) else {
        set_error(slot, CARD_ERROR_BROKEN);
        return CARD_ERROR_BROKEN;
    };
    refresh_serials_from_header(slot);
    let mut freeblocks = 0u16;
    for curblock in 5..SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire) {
        if unsafe { (*fat).fat[(curblock as usize) - 5] } == 0 {
            freeblocks = freeblocks.wrapping_add(1);
        }
    }
    SLOT_FREE_BLOCKS[slot].store(freeblocks, Ordering::Release);
    SLOT_MOUNTED[slot].store(true, Ordering::Release);
    set_error(slot, CARD_ERROR_READY);
    let _ = clear_status(chn);
    SLOT_DETACH_CALLBACK[slot].store(encode_callback(detach_cb), Ordering::Release);
    SLOT_GENERATION[slot].fetch_add(1, Ordering::AcqRel);
    CARD_ERROR_READY
}

pub fn mount_async(
    chn: i32,
    workarea: *mut c_void,
    detach_cb: CardCallback,
    attach_cb: CardCallback,
) -> i32 {
    if workarea.is_null() {
        return CARD_ERROR_NOCARD;
    }
    if slot_index(chn).is_none() {
        return CARD_ERROR_FATAL_ERROR;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::Mount,
        chn,
        workarea,
        detach_cb,
        callback: attach_cb,
        ..AsyncRequest::empty()
    })
}

pub fn mount(chn: i32, workarea: *mut c_void, detach_cb: CardCallback) -> i32 {
    mount_inner(chn, workarea, detach_cb)
}

pub fn unmount(chn: i32) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    SLOT_MOUNTED[slot].store(false, Ordering::Release);
    SLOT_WORKAREA[slot].store(0, Ordering::Release);
    SLOT_DETACH_CALLBACK[slot].store(0, Ordering::Release);
    SLOT_GENERATION[slot].fetch_add(1, Ordering::AcqRel);
    set_error(slot, CARD_ERROR_READY);
    CARD_ERROR_READY
}

pub fn open(chn: i32, filename: *const core::ffi::c_char, file: *mut CardFile) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if file.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    unsafe { (*file).filenum = -1 };
    if filename.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(idx) = find_file_index(slot, filename) else {
        return CARD_ERROR_NOFILE;
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let entry = unsafe { (*dir).entries[idx] };
    if !valid_file_block(entry.block, SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire)) {
        return CARD_ERROR_BROKEN;
    }
    unsafe {
        (*file).chn = chn;
        (*file).filenum = idx as i32;
        (*file).offset = 0;
        (*file).len = (entry.length as u32 * SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire)) as i32;
        (*file).iblock = entry.block;
    }
    CARD_ERROR_READY
}

pub fn open_entry(chn: i32, entry: *mut CardDir, file: *mut CardFile) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if file.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    unsafe { (*file).filenum = -1 };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(entry) = (unsafe { entry.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    let filename_len = dir_filename_len(&entry.filename);
    let Some(idx) = find_file_index_for_entry(
        slot,
        &entry.filename[..filename_len],
        &entry.gamecode,
        &entry.company,
    ) else {
        return CARD_ERROR_NOFILE;
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let saved = unsafe { (*dir).entries[idx] };
    if !valid_file_block(saved.block, SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire)) {
        return CARD_ERROR_BROKEN;
    }
    unsafe {
        (*file).chn = chn;
        (*file).filenum = idx as i32;
        (*file).offset = 0;
        (*file).len = (saved.length as u32 * SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire)) as i32;
        (*file).iblock = saved.block;
    }
    CARD_ERROR_READY
}

pub fn read_async(
    file: *mut CardFile,
    buffer: *mut c_void,
    len: u32,
    offset: u32,
    callback: CardCallback,
) -> i32 {
    if len == 0 || (len & 0x1ff) != 0 || (offset > 0 && (offset & 0x1ff) != 0) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let Some(file_state) = (unsafe { file.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !valid_file_number(file_state.filenum) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let chn = file_state.chn;
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    if let Err(code) = seek_file(file, slot, len, offset) {
        return code;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::Read,
        chn,
        callback,
        file,
        buffer,
        len,
        offset,
        ..AsyncRequest::empty()
    })
}

pub fn read(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    let Some(file_state) = (unsafe { file.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if buffer.is_null() || len == 0 {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !valid_file_number(file_state.filenum) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let chn = file_state.chn;
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0 || (len & 0x1ff) != 0 || (offset & 0x1ff) != 0 {
        return CARD_ERROR_FATAL_ERROR;
    }
    let (block, block_offset) = match seek_file(file, slot, len, offset) {
        Ok(position) => position,
        Err(code) => return code,
    };
    read_file_chain(chn, slot, block, block_offset, len, buffer.cast::<u8>())
}

pub fn find_first(chn: i32, dir: *mut CardDir, showall: Bool) -> i32 {
    if dir.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    // BUG FIX: CARD_FindFirst initializes these fields before searching or returning an error.
    init_dir_search(dir, chn, showall);
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(idx) = find_matching_index(slot, 0, showall) else {
        unsafe { (*dir).fileno = CARD_MAXFILES as u32 };
        return CARD_ERROR_NOFILE;
    };
    if fill_dir(slot, idx, dir).is_none() {
        return CARD_ERROR_BROKEN;
    }
    CARD_ERROR_READY
}

pub fn find_next(dir: *mut CardDir) -> i32 {
    if dir.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    // BUG FIX: CARD_FindNext advances the public cursor before validating/searching.
    unsafe { (*dir).fileno = (*dir).fileno.wrapping_add(1) };
    let chn = unsafe { (*dir).chn };
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let start = unsafe { (*dir).fileno as usize };
    if start >= CARD_MAXFILES {
        return CARD_ERROR_NOFILE;
    }
    let showall = unsafe { (*dir).showall };
    let Some(idx) = find_matching_index(slot, start, showall) else {
        unsafe { (*dir).fileno = CARD_MAXFILES as u32 };
        return CARD_ERROR_NOFILE;
    };
    if fill_dir(slot, idx, dir).is_none() {
        return CARD_ERROR_BROKEN;
    }
    CARD_ERROR_READY
}

fn init_dir_search(dir: *mut CardDir, chn: i32, showall: Bool) {
    unsafe {
        (*dir).chn = chn;
        (*dir).fileno = 0;
        (*dir).filelen = 0;
        (*dir).filename[0] = 0;
        (*dir).gamecode[0] = 0;
        (*dir).company[0] = 0;
        (*dir).showall = showall;
    }
}

pub fn get_directory(chn: i32, dir_entries: *mut CardDir, count: *mut i32, showall: Bool) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if dir_entries.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let mut written = 0usize;
    let mut idx = 0usize;
    while written < CARD_MAXFILES {
        let Some(found) = find_directory_matching_index(slot, idx, showall) else {
            break;
        };
        let output = unsafe { dir_entries.add(written) };
        let Some(permissions) = fill_dir(slot, found, output) else {
            break;
        };
        unsafe { (*output).permissions = permissions };
        written += 1;
        idx = found + 1;
    }
    if !count.is_null() {
        unsafe {
            *count = written as i32;
        }
    }
    if written == 0 {
        CARD_ERROR_NOFILE
    } else {
        CARD_ERROR_READY
    }
}

pub fn get_status(chn: i32, fileno: i32, stats: *mut CardStat) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !valid_file_number(fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    if stats.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(entry) = raw_dir_entry_at(slot, fileno) else {
        return CARD_ERROR_NOFILE;
    };
    fill_stat(slot, &entry, stats);
    CARD_ERROR_READY
}

pub fn get_status_ex(chn: i32, fileno: i32, entry: *mut CardDirEntry) -> i32 {
    if entry.is_null() {
        return CARD_ERROR_NOCARD;
    }
    if !(0..2).contains(&chn) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !valid_file_number(fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(src) = raw_dir_entry_at(slot, fileno) else {
        return CARD_ERROR_NOFILE;
    };
    unsafe {
        *entry = src;
    }
    CARD_ERROR_READY
}

pub fn get_attributes(chn: i32, fileno: i32, attr: *mut u8) -> i32 {
    let mut entry = CardDirEntry::default();
    let result = get_status_ex(chn, fileno, &mut entry);
    if result < 0 {
        return result;
    }
    if attr.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    unsafe {
        *attr = entry.permission;
    }
    result
}

fn find_free_entry(slot: usize) -> Option<usize> {
    let dir = current_dir(slot)?;
    let dir = unsafe { &*dir };
    dir.entries
        .iter()
        .enumerate()
        .find(|(_, entry)| !entry_used(entry))
        .map(|(idx, _)| idx)
}

fn entry_filename_len(entry: &CardDirEntry) -> usize {
    entry
        .filename
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(CARD_FILENAMELEN)
}

fn entry_name_matches_bytes(entry: &CardDirEntry, filename: &[u8]) -> bool {
    let len = entry_filename_len(entry);
    len == filename.len() && &entry.filename[..len] == filename
}

fn duplicate_entry_exists(slot: usize, filename: &[u8], skip: usize) -> bool {
    let Some(dir) = current_dir(slot) else {
        return false;
    };
    let gamecode = current_gamecode();
    let company = current_company();
    let dir = unsafe { &*dir };
    dir.entries.iter().enumerate().any(|(idx, entry)| {
        idx != skip
            && entry_used(entry)
            && entry_name_matches_bytes(entry, filename)
            && (gamecode[0] == 0xff
                || company[0] == 0xff
                || (entry.gamecode == gamecode && entry.company == company))
    })
}

fn duplicate_entry_exists_with_meta(
    slot: usize,
    filename: &[u8],
    gamecode: &[u8; 4],
    company: &[u8; 2],
    skip: usize,
) -> bool {
    let Some(dir) = current_dir(slot) else {
        return false;
    };
    let dir = unsafe { &*dir };
    dir.entries.iter().enumerate().any(|(idx, entry)| {
        idx != skip
            && entry_used(entry)
            && entry_name_matches_bytes(entry, filename)
            && entry.gamecode == *gamecode
            && entry.company == *company
    })
}

fn apply_current_game_ids(entry: &mut CardDirEntry, gamecode: [u8; 4], company: [u8; 2]) {
    if gamecode[0] != 0xff {
        entry.gamecode = gamecode;
    }
    // BUG FIX: C checks the game code twice here, so it can omit a valid company ID.
    if company[0] != 0xff {
        entry.company = company;
    }
}

fn init_entry_common(slot: usize, entry: &mut CardDirEntry, filename: &[u8], size: u32) {
    entry.gamecode = [0; 4];
    entry.company = [0; 2];
    apply_current_game_ids(entry, current_gamecode(), current_company());
    entry.pad_00 = 0xff;
    entry.banner_fmt = 0;
    entry.filename = [0; CARD_FILENAMELEN];
    let copy_len = filename.len().min(CARD_FILENAMELEN);
    entry.filename[..copy_len].copy_from_slice(&filename[..copy_len]);
    entry.last_modified = current_card_time_seconds();
    entry.icon_addr = u32::MAX;
    entry.icon_fmt = 0;
    entry.icon_speed = CARD_SPEED_FAST as u16;
    entry.permission = CARD_ATTRIB_PUBLIC;
    entry.copy_times = 0;
    entry.block = 5;
    entry.length = (size / SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire)) as u16;
    entry.pad_01 = 0xffff;
    entry.comment_addr = u32::MAX;
}

struct CardMetadataIo {
    slot: usize,
}

impl CardMetadataIo {
    fn channel(&self) -> i32 {
        if self.slot == 0 {
            CARD_SLOTA
        } else {
            CARD_SLOTB
        }
    }

    fn address(&self, sector: u32) -> Result<u32, i32> {
        let size = SLOT_SECTOR_SIZE[self.slot].load(Ordering::Acquire);
        (size != 0)
            .then_some(sector.wrapping_mul(size))
            .ok_or(CARD_ERROR_BROKEN)
    }

    fn transport_error(&self) -> i32 {
        if SLOT_PRESENT[self.slot].load(Ordering::Acquire) {
            CARD_ERROR_IOERROR
        } else {
            CARD_ERROR_NOCARD
        }
    }
}

impl BlockIo for CardMetadataIo {
    type Error = i32;

    fn erase(&mut self, sector: u32) -> Result<(), Self::Error> {
        let address = self.address(sector)?;
        sector_erase(self.channel(), address)
            .then_some(())
            .ok_or_else(|| self.transport_error())
    }

    fn write(&mut self, sector: u32, block: &[u8]) -> Result<(), Self::Error> {
        let address = self.address(sector)?;
        write_range(
            self.channel(),
            address,
            block.as_ptr().cast_mut(),
            block.len() as u32,
        )
        .then_some(())
        .ok_or_else(|| self.transport_error())
    }

    fn read(&mut self, sector: u32, block: &mut [u8]) -> Result<(), Self::Error> {
        let address = self.address(sector)?;
        read_range(
            self.channel(),
            address,
            block.as_mut_ptr(),
            block.len() as u32,
        )
        .then_some(())
        .ok_or_else(|| self.transport_error())
    }
}

fn commit_error_code(error: CommitError<i32>) -> i32 {
    match error {
        CommitError::Io(code) => code,
        CommitError::InvalidLength | CommitError::ReadbackMismatch => CARD_ERROR_BROKEN,
    }
}

fn recover_after_metadata_failure(slot: usize) {
    if let Err(code) = reload_metadata(slot) {
        set_error(slot, code);
        SLOT_MOUNTED[slot].store(false, Ordering::Release);
    }
}

fn commit_directory(slot: usize) -> i32 {
    let target_index = SLOT_DIR_TARGET[slot].load(Ordering::Acquire);
    let Some(target) = dirblock_ptr(slot, target_index).map(|ptr| ptr.cast::<u8>()) else {
        return CARD_ERROR_BROKEN;
    };
    let Some(readback) = current_dir_back(slot).map(|ptr| ptr.cast::<u8>()) else {
        return CARD_ERROR_BROKEN;
    };
    let Some(target_bytes) = block_bytes_mut(target) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_directory(target_bytes).is_err() {
        recover_after_metadata_failure(slot);
        return CARD_ERROR_BROKEN;
    }
    let Some(readback_bytes) = block_bytes_mut(readback) else {
        return CARD_ERROR_BROKEN;
    };
    let mut io = CardMetadataIo { slot };
    let result = metadata::commit_verified(
        &mut io,
        (target_index + 1) as u32,
        target_bytes,
        readback_bytes,
    );
    match result {
        Ok(()) => {
            SLOT_DIR_TARGET[slot].store(target_index ^ 1, Ordering::Release);
            CARD_ERROR_READY
        }
        Err(error) => {
            let code = commit_error_code(error);
            recover_after_metadata_failure(slot);
            code
        }
    }
}

fn commit_fat(slot: usize) -> i32 {
    let target_index = SLOT_FAT_TARGET[slot].load(Ordering::Acquire);
    let Some(target) = fatblock_ptr(slot, target_index).map(|ptr| ptr.cast::<u8>()) else {
        return CARD_ERROR_BROKEN;
    };
    let Some(readback) = current_fat_back(slot).map(|ptr| ptr.cast::<u8>()) else {
        return CARD_ERROR_BROKEN;
    };
    let blocks = match u16::try_from(SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire)) {
        Ok(value) => value,
        Err(_) => return CARD_ERROR_BROKEN,
    };
    let Some(target_bytes) = block_bytes_mut(target) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_fat(target_bytes, blocks).is_err() {
        recover_after_metadata_failure(slot);
        return CARD_ERROR_BROKEN;
    }
    let Some(readback_bytes) = block_bytes_mut(readback) else {
        return CARD_ERROR_BROKEN;
    };
    let mut io = CardMetadataIo { slot };
    let result = metadata::commit_verified(
        &mut io,
        (target_index + 3) as u32,
        target_bytes,
        readback_bytes,
    );
    match result {
        Ok(()) => {
            SLOT_FAT_TARGET[slot].store(target_index ^ 1, Ordering::Release);
            let Some(fat) = current_fat(slot) else {
                return CARD_ERROR_BROKEN;
            };
            SLOT_FREE_BLOCKS[slot].store(unsafe { (*fat).freeblocks }, Ordering::Release);
            CARD_ERROR_READY
        }
        Err(error) => {
            let code = commit_error_code(error);
            recover_after_metadata_failure(slot);
            code
        }
    }
}

fn write_formatted_metadata(slot: usize) -> i32 {
    let blocks = match u16::try_from(SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire)) {
        Ok(value) => value,
        Err(_) => return CARD_ERROR_BROKEN,
    };
    let valid = system_block_ptr(slot, 0)
        .and_then(|ptr| block_bytes(ptr))
        .is_some_and(|block| metadata::validate_header(block).is_ok())
        && (0..2).all(|copy| {
            dirblock_ptr(slot, copy)
                .and_then(|ptr| block_bytes(ptr.cast()))
                .is_some_and(|block| metadata::validate_directory(block).is_ok())
        })
        && (0..2).all(|copy| {
            fatblock_ptr(slot, copy)
                .and_then(|ptr| block_bytes(ptr.cast()))
                .is_some_and(|block| metadata::validate_fat(block, blocks).is_ok())
        });
    if !valid {
        return CARD_ERROR_BROKEN;
    }
    let mut io = CardMetadataIo { slot };
    let result = metadata::commit_system_blocks(|sector| {
        let Some(expected) =
            system_block_ptr(slot, sector as usize).and_then(|ptr| block_bytes(ptr))
        else {
            return Err(CARD_ERROR_BROKEN);
        };
        let readback = unsafe { &mut METADATA_READBACK[slot].0 };
        if let Err(error) = metadata::commit_verified(&mut io, sector, expected, readback) {
            let code = commit_error_code(error);
            recover_after_metadata_failure(slot);
            return Err(code);
        }
        Ok(())
    });
    if let Err(code) = result {
        return code;
    }
    reload_metadata(slot).map_or_else(|code| code, |()| CARD_ERROR_READY)
}

pub fn close(file: *mut CardFile) -> i32 {
    let Some(file) = (unsafe { file.as_mut() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    let Some(slot) = slot_index(file.chn) else {
        return CARD_ERROR_NOCARD;
    };
    // BUG FIX: CARD_Close validates the file number before checking card state.
    if !(0..CARD_MAXFILES as i32).contains(&file.filenum) {
        return CARD_ERROR_NOFILE;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    file.chn = -1;
    CARD_ERROR_READY
}

pub fn write(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    let Some(file_state) = (unsafe { file.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !valid_file_number(file_state.filenum) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let chn = file_state.chn;
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    if buffer.is_null() || len == 0 || (len & 0x1ff) != 0 || (offset & 0x1ff) != 0 {
        return CARD_ERROR_FATAL_ERROR;
    }
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0 || sector_size as usize > MAX_CARD_SECTOR_SIZE {
        return CARD_ERROR_FATAL_ERROR;
    }
    let (mut block, mut block_offset) = match seek_file(file, slot, len, offset) {
        Ok(position) => position,
        Err(code) => return code,
    };
    let mut remaining = len;
    let mut src_off = 0usize;

    while remaining > 0 {
        let chunk = remaining.min(sector_size - block_offset);
        let address = u32::from(block).wrapping_mul(sector_size);
        let write_ptr = if block_offset == 0 && chunk == sector_size {
            unsafe { buffer.cast::<u8>().add(src_off) }
        } else {
            let scratch = unsafe { WRITE_SCRATCH[slot].0.as_mut_ptr() };
            if !read_range(chn, address, scratch, sector_size) {
                return CARD_ERROR_NOCARD;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buffer.cast::<u8>().add(src_off),
                    scratch.add(block_offset as usize),
                    chunk as usize,
                );
            }
            scratch
        };

        if !sector_erase(chn, address) {
            return CARD_ERROR_NOCARD;
        }
        if !write_range(chn, address, write_ptr, sector_size) {
            return CARD_ERROR_NOCARD;
        }

        remaining -= chunk;
        src_off += chunk as usize;
        block_offset = 0;
        if remaining > 0 {
            let Some(next) = file_block_next(slot, block) else {
                return CARD_ERROR_BROKEN;
            };
            block = next;
        }
    }
    CARD_ERROR_READY
}

#[cfg(test)]
mod tests {
    use super::{
        CARD_ERROR_BUSY, CARD_ERROR_FATAL_ERROR, CARD_ERROR_NOCARD, CARD_ERROR_NOFILE,
        apply_status_ex_update, apply_status_update, card_probe_error, card_time_seconds,
        entry_matches_directory, entry_matches_find, file_range_exceeds_len, fill_dir_fields,
        find_first, find_next, format_serial, get_block_count, get_free_blocks, get_sector_size,
        init_created_file, probe_ex, status_entry_has_filename, valid_file_block,
        valid_file_number,
    };

    #[test]
    fn status_update_preserves_file_identity_and_other_icon_speeds() {
        let mut entry = super::CardDirEntry::default();
        entry.filename[0] = b'f';
        entry.gamecode = *b"GAME";
        entry.company = *b"01";
        entry.length = 4;
        entry.icon_speed = 0x000c;
        let mut stats = super::CardStat::default();
        stats.filename[0] = b'x';
        stats.gamecode = *b"XXXX";
        stats.company = *b"YY";
        stats.len = 99;
        stats.icon_addr = u32::MAX;

        apply_status_update(&mut entry, &mut stats, 123);

        assert_eq!(entry.filename[0], b'f');
        assert_eq!(entry.gamecode, *b"GAME");
        assert_eq!(entry.company, *b"01");
        assert_eq!(entry.length, 4);
        assert_eq!(entry.last_modified, 123);
        assert_eq!(entry.icon_speed, 0x000d);
    }

    #[test]
    fn status_ex_update_preserves_card_chain_metadata() {
        let mut entry = super::CardDirEntry::default();
        entry.block = 12;
        entry.length = 7;
        entry.pad_00 = 0x34;
        entry.pad_01 = 0x5678;
        let update = super::CardDirEntry {
            last_modified: 99,
            permission: 3,
            copy_times: 2,
            ..super::CardDirEntry::default()
        };

        apply_status_ex_update(&mut entry, &update);

        assert_eq!(entry.block, 12);
        assert_eq!(entry.length, 7);
        assert_eq!(entry.pad_00, 0x34);
        assert_eq!(entry.pad_01, 0x5678);
        assert_eq!(entry.last_modified, 99);
        assert_eq!(entry.permission, 3);
        assert_eq!(entry.copy_times, 2);
    }

    #[test]
    fn get_status_ex_matches_c_argument_error_precedence() {
        assert_eq!(
            super::get_status_ex(-1, -1, core::ptr::null_mut()),
            CARD_ERROR_NOCARD
        );
        assert_eq!(
            super::get_status_ex(
                -1,
                -1,
                core::ptr::NonNull::<super::CardDirEntry>::dangling().as_ptr()
            ),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn get_attributes_matches_c_invalid_channel_error() {
        assert_eq!(
            super::get_attributes(-1, -1, core::ptr::null_mut()),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::get_attributes(0, 0, core::ptr::null_mut()),
            CARD_ERROR_NOCARD
        );
    }

    #[test]
    fn set_attributes_async_validates_arguments_before_queueing() {
        assert_eq!(
            super::set_attributes_async(-1, -1, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::set_attributes_async(0, -1, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn async_directory_entry_snapshot_does_not_borrow_caller_storage() {
        let mut source = super::CardDir::default();
        source.filelen = 4096;
        source.filename[0] = b's';
        let snapshot = super::snapshot_dir_entry(&source).unwrap();
        source.filelen = 0;
        source.filename[0] = b'x';

        assert_eq!(snapshot.filelen, 4096);
        assert_eq!(snapshot.filename[0], b's');
        assert_eq!(source.filename[0], b'x');
    }

    #[test]
    fn async_card_stat_snapshot_does_not_borrow_caller_storage() {
        let mut source = super::CardStat::default();
        source.banner_fmt = 3;
        let snapshot = super::snapshot_card_stat(&source).unwrap();
        source.banner_fmt = 7;

        assert_eq!(snapshot.banner_fmt, 3);
        assert_eq!(source.banner_fmt, 7);
    }

    #[test]
    fn async_filename_snapshot_copies_and_limits_card_filename_bytes() {
        let mut source = *b"save\0";
        let (snapshot, len) = super::snapshot_filename(source.as_ptr().cast()).unwrap();
        source[0] = b'x';

        assert_eq!(len, 4);
        assert_eq!(&snapshot[..5], b"save\0");
        assert_eq!(source[0], b'x');

        let long_name = [b'n'; super::CARD_FILENAMELEN + 1];
        let mut nul_terminated = [0; super::CARD_FILENAMELEN + 2];
        nul_terminated[..long_name.len()].copy_from_slice(&long_name);
        let (truncated, len) = super::snapshot_filename(nul_terminated.as_ptr().cast()).unwrap();
        assert_eq!(len, super::CARD_FILENAMELEN + 1);
        assert_eq!(
            &truncated[..super::CARD_FILENAMELEN],
            &long_name[..super::CARD_FILENAMELEN]
        );
        assert_eq!(truncated[super::CARD_FILENAMELEN], 0);
    }

    #[test]
    fn create_async_rejects_overlong_name_before_queueing() {
        let name = [b'n'; super::CARD_FILENAMELEN + 1];
        let mut nul_terminated = [0; super::CARD_FILENAMELEN + 2];
        nul_terminated[..name.len()].copy_from_slice(&name);
        assert_eq!(
            super::create_async(
                0,
                nul_terminated.as_ptr().cast(),
                0,
                core::ptr::null_mut(),
                None,
            ),
            super::CARD_ERROR_NAMETOOLONG
        );
    }

    #[test]
    fn card_create_checks_name_length_before_channel() {
        let name = [b'n'; super::CARD_FILENAMELEN + 1];
        let mut nul_terminated = [0; super::CARD_FILENAMELEN + 2];
        nul_terminated[..name.len()].copy_from_slice(&name);
        assert_eq!(
            super::create(-1, nul_terminated.as_ptr().cast(), 0, core::ptr::null_mut(),),
            super::CARD_ERROR_NAMETOOLONG
        );
    }

    #[test]
    fn card_delete_invalid_channel_precedes_pointer_access() {
        assert_eq!(
            super::delete_async(-1, core::ptr::null(), None),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::delete_entry_async(-1, core::ptr::null_mut(), None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn card_async_channel_errors_match_c_entry_points() {
        assert_eq!(
            super::mount_async(-1, core::ptr::null_mut(), None, None),
            super::CARD_ERROR_NOCARD
        );
        assert_eq!(
            super::mount_async(
                -1,
                core::ptr::NonNull::<u8>::dangling().as_ptr().cast(),
                None,
                None,
            ),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(super::format_async(-1, None), super::CARD_ERROR_NOCARD);

        let mut entry = super::CardDir::default();
        assert_eq!(
            super::create_entry_async(-1, &mut entry, core::ptr::null_mut(), None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn read_async_rejects_unaligned_ranges_before_file_lookup() {
        assert_eq!(
            super::read_async(core::ptr::null_mut(), core::ptr::null_mut(), 0, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::read_async(core::ptr::null_mut(), core::ptr::null_mut(), 512, 1, None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn open_entry_checks_channel_before_caller_pointers() {
        let mut file = super::CardFile {
            filenum: 17,
            ..super::CardFile::default()
        };
        assert_eq!(
            super::open_entry(-1, core::ptr::null_mut(), &mut file),
            CARD_ERROR_NOCARD
        );
        assert_eq!(file.filenum, 17);
    }

    #[test]
    fn card_transfers_check_file_number_before_channel() {
        let file = super::CardFile {
            chn: -1,
            filenum: -1,
            ..super::CardFile::default()
        };
        let file_ptr = (&file as *const super::CardFile).cast_mut();
        let buffer = core::ptr::NonNull::<u8>::dangling().as_ptr().cast();

        assert_eq!(
            super::read(file_ptr, buffer, 512, 0),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::write(file_ptr, buffer, 512, 0),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::read_async(file_ptr, buffer, 512, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::write_async(file_ptr, buffer, 512, 0, None),
            CARD_ERROR_FATAL_ERROR
        );

        let file = super::CardFile {
            chn: -1,
            filenum: 0,
            ..super::CardFile::default()
        };
        let file_ptr = (&file as *const super::CardFile).cast_mut();
        assert_eq!(
            super::read(file_ptr, buffer, 512, 0),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::write(file_ptr, buffer, 512, 0),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::read_async(file_ptr, buffer, 512, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            super::write_async(file_ptr, buffer, 512, 0, None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn create_size_requires_a_nonzero_sector_multiple() {
        assert!(!super::valid_create_size(0, 8192));
        assert!(!super::valid_create_size(512, 8192));
        assert!(super::valid_create_size(8192, 8192));
        assert!(!super::valid_create_size(8192, 0));
    }

    #[test]
    fn create_entry_applies_gamecode_and_company_wildcards_independently() {
        let mut entry = super::CardDirEntry::default();
        super::apply_current_game_ids(&mut entry, [0xff; 4], *b"01");
        assert_eq!(entry.gamecode, [0; 4]);
        assert_eq!(entry.company, *b"01");

        entry = super::CardDirEntry::default();
        super::apply_current_game_ids(&mut entry, *b"GAME", [0xff; 2]);
        assert_eq!(entry.gamecode, *b"GAME");
        assert_eq!(entry.company, [0; 2]);
    }

    #[test]
    fn operation_result_updates_channel_error_code() {
        let previous_error = super::SLOT_ERROR[0].swap(
            super::CARD_ERROR_READY,
            core::sync::atomic::Ordering::AcqRel,
        );
        let previous_present =
            super::SLOT_PRESENT[0].swap(false, core::sync::atomic::Ordering::AcqRel);
        super::mark_operation_busy(super::CARD_SLOTA);
        assert_eq!(super::get_error_code(super::CARD_SLOTA), CARD_ERROR_BUSY);
        assert!(super::channel_busy(super::CARD_SLOTA));
        super::record_operation_result(super::CARD_SLOTA, CARD_ERROR_FATAL_ERROR);
        assert_eq!(
            super::get_error_code(super::CARD_SLOTA),
            CARD_ERROR_FATAL_ERROR
        );
        super::SLOT_PRESENT[0].store(previous_present, core::sync::atomic::Ordering::Release);
        super::SLOT_ERROR[0].store(previous_error, core::sync::atomic::Ordering::Release);
    }

    #[test]
    fn filename_lookup_matches_c_bounded_strncmp() {
        let entry = super::CardDirEntry {
            filename: [b'a'; super::CARD_FILENAMELEN],
            ..super::CardDirEntry::default()
        };
        let mut longer_name = [0; super::CARD_FILENAMELEN + 2];
        longer_name[..super::CARD_FILENAMELEN].fill(b'a');
        longer_name[super::CARD_FILENAMELEN] = b'x';
        assert!(super::super::entry_name_matches(
            &entry,
            longer_name.as_ptr().cast()
        ));

        longer_name[super::CARD_FILENAMELEN - 1] = b'b';
        assert!(!super::super::entry_name_matches(
            &entry,
            longer_name.as_ptr().cast()
        ));
    }

    #[test]
    fn set_status_async_checks_c_argument_errors_before_queueing() {
        assert_eq!(
            super::set_status_async(-1, -1, core::ptr::null_mut(), None),
            super::CARD_ERROR_NOCARD
        );
        assert_eq!(
            super::set_status_async(0, -1, core::ptr::null_mut(), None),
            CARD_ERROR_FATAL_ERROR
        );
    }

    #[test]
    fn card_timestamp_uses_console_timebase_rate() {
        assert_eq!(card_time_seconds(121_500_000, true), 2);
        assert_eq!(card_time_seconds(81_000_000, false), 2);
    }

    #[test]
    fn format_serial_matches_libogc_header_fields() {
        assert_eq!(
            format_serial(
                0x1234_5678_9abc_def0,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
                0x1020_3040,
                2,
                0x5678,
            ),
            [
                0xf884_d3ba,
                0x141f_bba3,
                0xca6b_a497,
                0x1234_5678,
                0x9abc_def0,
                0x1020_3040,
                2,
                0x5678,
            ]
        );
    }

    #[test]
    fn file_range_check_accepts_end_and_rejects_overflow() {
        assert!(!file_range_exceeds_len(1024, 512, 512));
        assert!(file_range_exceeds_len(1024, 512, 1024));
        assert!(file_range_exceeds_len(1024, 1, u32::MAX));
    }

    #[test]
    fn file_block_check_rejects_system_and_out_of_range_blocks() {
        assert!(!valid_file_block(4, 32));
        assert!(valid_file_block(5, 32));
        assert!(!valid_file_block(32, 32));
    }

    #[test]
    fn file_number_check_matches_card_limits() {
        assert!(!valid_file_number(-1));
        assert!(valid_file_number(0));
        assert!(!valid_file_number(super::CARD_MAXFILES as i32));
    }

    #[test]
    fn status_entry_filename_check_matches_libogc() {
        let mut entry = super::CardDirEntry::default();
        assert!(!status_entry_has_filename(&entry));
        entry.filename[0] = 0xff;
        assert!(!status_entry_has_filename(&entry));
        entry.filename[0] = b'a';
        assert!(status_entry_has_filename(&entry));
    }

    #[test]
    fn invalid_card_channels_return_the_c_error_code() {
        let mut output_u32 = 0;
        let mut output_u16 = 0;
        assert_eq!(
            unsafe { probe_ex(-1, core::ptr::null_mut(), core::ptr::null_mut()) },
            CARD_ERROR_FATAL_ERROR
        );
        assert_eq!(
            unsafe { get_sector_size(-1, &mut output_u32) },
            CARD_ERROR_NOCARD
        );
        assert_eq!(
            unsafe { get_block_count(-1, &mut output_u32) },
            CARD_ERROR_NOCARD
        );
        assert_eq!(
            unsafe { get_free_blocks(-1, &mut output_u16) },
            CARD_ERROR_NOCARD
        );
        assert_eq!(
            unsafe { get_sector_size(0, core::ptr::null_mut()) },
            CARD_ERROR_NOCARD
        );
        assert_eq!(
            unsafe { get_block_count(0, core::ptr::null_mut()) },
            CARD_ERROR_NOCARD
        );
    }

    #[test]
    fn card_probe_maps_exi_status_like_libogc() {
        assert_eq!(card_probe_error(1), None);
        assert_eq!(card_probe_error(0), Some(CARD_ERROR_BUSY));
        assert_eq!(card_probe_error(-1), Some(CARD_ERROR_NOCARD));
    }

    #[test]
    fn directory_search_updates_fields_before_returning_errors() {
        let mut dir = super::CardDir {
            chn: 0,
            fileno: 5,
            filelen: 99,
            filename: [b'x'; super::CARD_FILENAMELEN],
            gamecode: [b'y'; 4],
            company: [b'z'; 2],
            showall: 0,
            ..super::CardDir::default()
        };
        assert_eq!(find_first(-1, &mut dir, 1), CARD_ERROR_NOCARD);
        assert_eq!(dir.chn, -1);
        assert_eq!(dir.fileno, 0);
        assert_eq!(dir.filelen, 0);
        assert_eq!(dir.filename[0], 0);
        assert_eq!(dir.filename[1], b'x');
        assert_eq!(dir.gamecode[0], 0);
        assert_eq!(dir.gamecode[1], b'y');
        assert_eq!(dir.company[0], 0);
        assert_eq!(dir.company[1], b'z');
        assert_eq!(dir.showall, 1);

        dir.fileno = 8;
        assert_eq!(find_next(&mut dir), CARD_ERROR_NOCARD);
        assert_eq!(dir.fileno, 9);
    }

    #[test]
    fn directory_filters_match_libogc_wildcard_rules() {
        let entry = super::CardDirEntry {
            gamecode: *b"GAME",
            company: *b"01",
            ..super::CardDirEntry::default()
        };

        assert!(!entry_matches_find(&entry, &[0xff; 4], &[0xff; 2], 0));
        assert!(!entry_matches_directory(&entry, &[0xff; 4], &[0xff; 2], 0));
        assert!(!entry_matches_find(&entry, b"GAME", &[0xff; 2], 0));
        assert!(!entry_matches_directory(&entry, b"GAME", &[0xff; 2], 0));
        assert!(entry_matches_find(&entry, b"GAME", b"01", 0));
        assert!(entry_matches_directory(&entry, b"GAME", b"01", 0));
        assert!(entry_matches_find(&entry, &[0xff; 4], &[0xff; 2], 1));
        assert!(entry_matches_directory(&entry, &[0xff; 4], &[0xff; 2], 1));
    }

    #[test]
    fn directory_fill_preserves_fields_libogc_leaves_untouched() {
        let mut dir = super::CardDir {
            chn: -7,
            fileno: 99,
            filelen: 88,
            permissions: 0x5a,
            filename: [b'x'; super::CARD_FILENAMELEN],
            gamecode: [b'y'; 4],
            company: [b'z'; 2],
            showall: 1,
        };
        let entry = super::CardDirEntry {
            filename: [b'n'; super::CARD_FILENAMELEN],
            gamecode: *b"GAME",
            company: *b"01",
            length: 2,
            permission: 3,
            ..super::CardDirEntry::default()
        };

        fill_dir_fields(&mut dir, 4, entry, 512);

        assert_eq!(dir.chn, -7);
        assert_eq!(dir.fileno, 4);
        assert_eq!(dir.filelen, 1024);
        assert_eq!(dir.permissions, 0x5a);
        assert_eq!(dir.filename, [b'n'; super::CARD_FILENAMELEN]);
        assert_eq!(dir.gamecode, *b"GAME");
        assert_eq!(dir.company, *b"01");
        assert_eq!(dir.showall, 1);
    }

    #[test]
    fn created_file_state_preserves_request_length() {
        let mut file = super::CardFile {
            chn: -1,
            filenum: -1,
            offset: 99,
            len: 1234,
            iblock: 0,
        };

        init_created_file(&mut file, 1, 7, 12);

        assert_eq!(file.chn, 1);
        assert_eq!(file.filenum, 7);
        assert_eq!(file.offset, 0);
        assert_eq!(file.len, 1234);
        assert_eq!(file.iblock, 12);
    }

    #[test]
    fn close_checks_file_number_before_card_state() {
        let mut file = super::CardFile {
            chn: super::CARD_SLOTA,
            filenum: -1,
            ..super::CardFile::default()
        };

        assert_eq!(super::close(&mut file), CARD_ERROR_NOFILE);
    }
}

pub fn write_async(
    file: *mut CardFile,
    buffer: *mut c_void,
    len: u32,
    offset: u32,
    callback: CardCallback,
) -> i32 {
    let Some(file_state) = (unsafe { file.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !valid_file_number(file_state.filenum) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let chn = file_state.chn;
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    if let Err(code) = seek_file(file, slot, len, offset) {
        return code;
    }
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if sector_size == 0
        || (len & (sector_size - 1)) != 0
        || (offset > 0 && (offset & (sector_size - 1)) != 0)
    {
        return CARD_ERROR_FATAL_ERROR;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::Write,
        chn,
        callback,
        file,
        buffer,
        len,
        offset,
        ..AsyncRequest::empty()
    })
}

pub fn create_async(
    chn: i32,
    filename: *const core::ffi::c_char,
    size: u32,
    file: *mut CardFile,
    callback: CardCallback,
) -> i32 {
    let Some((filename, len)) = snapshot_filename(filename) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if len > CARD_FILENAMELEN {
        return CARD_ERROR_NAMETOOLONG;
    }
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if !valid_create_size(size, sector_size) || file.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::Create,
        chn,
        callback,
        filename,
        size,
        file,
        ..AsyncRequest::empty()
    })
}

pub fn create(chn: i32, filename: *const core::ffi::c_char, size: u32, file: *mut CardFile) -> i32 {
    if filename.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    let name = unsafe { core::ffi::CStr::from_ptr(filename).to_bytes() };
    if name.len() > CARD_FILENAMELEN {
        return CARD_ERROR_NAMETOOLONG;
    }
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if file.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if !valid_create_size(size, sector_size) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let Some(free_idx) = find_free_entry(slot) else {
        return CARD_ERROR_NOENT;
    };
    let Some(_fat) = current_fat(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let mut entry = unsafe { (*current_dir(slot).unwrap()).entries[free_idx] };
    if duplicate_entry_exists(slot, name, free_idx) {
        return CARD_ERROR_EXIST;
    }
    let blocksneed = size / sector_size;
    let Some(first_block) = allocate_blocks(slot, blocksneed) else {
        return CARD_ERROR_INSSPACE;
    };
    init_entry_common(slot, &mut entry, name, size);
    entry.block = first_block;
    unsafe {
        (*current_dir(slot).unwrap()).entries[free_idx] = entry;
    }
    refresh_serials_from_header(slot);
    let commit_result = metadata::commit_pair(PairCommit::Create, |block| {
        let result = match block {
            MetadataBlock::Directory => commit_directory(slot),
            MetadataBlock::Fat => commit_fat(slot),
        };
        (result == CARD_ERROR_READY).then_some(()).ok_or(result)
    });
    if let Err(code) = commit_result {
        return code;
    }
    init_created_file(file, chn, free_idx, first_block);
    CARD_ERROR_READY
}

fn init_created_file(file: *mut CardFile, chn: i32, fileno: usize, first_block: u16) {
    // BUG FIX: C initializes these fields but leaves the caller's request length unchanged.
    unsafe {
        (*file).chn = chn;
        (*file).filenum = fileno as i32;
        (*file).offset = 0;
        (*file).iblock = first_block;
    }
}

pub fn create_entry_async(
    chn: i32,
    direntry: *mut CardDir,
    file: *mut CardFile,
    callback: CardCallback,
) -> i32 {
    let Some(direntry) = snapshot_dir_entry(direntry) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let sector_size = SLOT_SECTOR_SIZE[slot].load(Ordering::Acquire);
    if !valid_create_size(direntry.filelen, sector_size) || file.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::CreateEntry,
        chn,
        callback,
        direntry,
        file,
        ..AsyncRequest::empty()
    })
}

pub fn create_entry(chn: i32, direntry: *mut CardDir, file: *mut CardFile) -> i32 {
    let Some(direntry) = (unsafe { direntry.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    create(
        chn,
        direntry.filename.as_ptr().cast(),
        direntry.filelen,
        file,
    )
}

pub fn delete_async(chn: i32, filename: *const core::ffi::c_char, callback: CardCallback) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some((filename, _)) = snapshot_filename(filename) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    submit_async(AsyncRequest {
        kind: AsyncKind::Delete,
        chn,
        callback,
        filename,
        ..AsyncRequest::empty()
    })
}

pub fn delete(chn: i32, filename: *const core::ffi::c_char) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    if filename.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let Some(idx) = find_file_index(slot, filename) else {
        return CARD_ERROR_NOFILE;
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let entry = unsafe { &mut (*dir).entries[idx] };
    if entry.block < 5 {
        return CARD_ERROR_BROKEN;
    }
    if !free_chain(slot, entry.block) {
        return CARD_ERROR_BROKEN;
    }
    unsafe {
        core::ptr::write_bytes(
            entry as *mut CardDirEntry as *mut u8,
            0xff,
            size_of::<CardDirEntry>(),
        );
    }
    let commit_result = metadata::commit_pair(PairCommit::Delete, |block| {
        let result = match block {
            MetadataBlock::Directory => commit_directory(slot),
            MetadataBlock::Fat => commit_fat(slot),
        };
        (result == CARD_ERROR_READY).then_some(()).ok_or(result)
    });
    if let Err(code) = commit_result {
        return code;
    }
    CARD_ERROR_READY
}

pub fn delete_entry_async(chn: i32, dir_entry: *mut CardDir, callback: CardCallback) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(dir_entry) = snapshot_dir_entry(dir_entry) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    submit_async(AsyncRequest {
        kind: AsyncKind::DeleteEntry,
        chn,
        callback,
        direntry: dir_entry,
        ..AsyncRequest::empty()
    })
}

pub fn delete_entry(chn: i32, dir_entry: *mut CardDir) -> i32 {
    let Some(dir_entry) = (unsafe { dir_entry.as_ref() }) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let filename_len = dir_filename_len(&dir_entry.filename);
    let Some(_idx) = find_file_index_for_entry(
        slot,
        &dir_entry.filename[..filename_len],
        &dir_entry.gamecode,
        &dir_entry.company,
    ) else {
        return CARD_ERROR_NOFILE;
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let entry = unsafe { &mut (*dir).entries[_idx] };
    if entry.block < 5 {
        return CARD_ERROR_BROKEN;
    }
    if !free_chain(slot, entry.block) {
        return CARD_ERROR_BROKEN;
    }
    unsafe {
        core::ptr::write_bytes(
            entry as *mut CardDirEntry as *mut u8,
            0xff,
            size_of::<CardDirEntry>(),
        );
    }
    let commit_result = metadata::commit_pair(PairCommit::Delete, |block| {
        let result = match block {
            MetadataBlock::Directory => commit_directory(slot),
            MetadataBlock::Fat => commit_fat(slot),
        };
        (result == CARD_ERROR_READY).then_some(()).ok_or(result)
    });
    if let Err(code) = commit_result {
        return code;
    }
    CARD_ERROR_READY
}

pub fn format_async(chn: i32, callback: CardCallback) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !SLOT_PRESENT[slot].load(Ordering::Acquire) {
        return CARD_ERROR_NOCARD;
    }
    if channel_busy(chn) {
        return CARD_ERROR_BUSY;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::Format,
        chn,
        callback,
        encoding: unsafe { ogc_system_sys::sys_get_font_encoding() },
        ..AsyncRequest::empty()
    })
}

pub fn format(chn: i32) -> i32 {
    format_with_encoding(chn, unsafe { ogc_system_sys::sys_get_font_encoding() })
}

fn format_with_encoding(chn: i32, encoding: u32) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let Some(base) = workarea_base(slot) else {
        return CARD_ERROR_BROKEN;
    };
    unsafe {
        core::ptr::write_bytes(base, 0xff, METADATA_BLOCK_SIZE * 3);
        core::ptr::write_bytes(
            base.add(METADATA_BLOCK_SIZE * 3),
            0,
            METADATA_BLOCK_SIZE * 2,
        );
    }
    let header = unsafe { &mut *(base.cast::<CardHeader>()) };
    let dir = unsafe { &mut *(base.add(0x2000).cast::<CardDirTable>()) };
    let dir_back = unsafe { &mut *(base.add(0x4000).cast::<CardDirTable>()) };
    let fat = unsafe { &mut *(base.add(0x6000).cast::<CardBat>()) };
    let fat_back = unsafe { &mut *(base.add(0x8000).cast::<CardBat>()) };
    header.device_id = 0;
    header.size = (SLOT_ID[slot].load(Ordering::Acquire) & 0xfc) as u16;
    header.encoding = encoding as u16;
    let Some(serial) = (unsafe { formatted_serial(chn) }) else {
        return CARD_ERROR_NOCARD;
    };
    header.serial = serial;
    refresh_serials_from_header(slot);
    dir_back.entries = dir.entries;
    fat.updated = u16::MAX;
    fat.freeblocks = (SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire) - 5) as u16;
    fat.lastalloc = 4;
    fat_back.updated = 0;
    fat_back.freeblocks = fat.freeblocks;
    fat_back.lastalloc = fat.lastalloc;
    let Some(header_bytes) = block_bytes_mut(base) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_header(header_bytes).is_err() {
        return CARD_ERROR_BROKEN;
    }
    let Some(dir_bytes) = block_bytes_mut(dir as *mut CardDirTable as *mut u8) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_directory(dir_bytes).is_err() {
        return CARD_ERROR_BROKEN;
    }
    let Some(dir_back_bytes) = block_bytes_mut(dir_back as *mut CardDirTable as *mut u8) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_directory(dir_back_bytes).is_err() {
        return CARD_ERROR_BROKEN;
    }
    let block_count = SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire) as u16;
    let Some(fat_bytes) = block_bytes_mut(fat as *mut CardBat as *mut u8) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_fat(fat_bytes, block_count).is_err() {
        return CARD_ERROR_BROKEN;
    }
    let Some(fat_back_bytes) = block_bytes_mut(fat_back as *mut CardBat as *mut u8) else {
        return CARD_ERROR_BROKEN;
    };
    if metadata::prepare_fat(fat_back_bytes, block_count).is_err() {
        return CARD_ERROR_BROKEN;
    }
    let result = write_formatted_metadata(slot);
    if result != CARD_ERROR_READY {
        return result;
    }
    SLOT_FREE_BLOCKS[slot].store(
        (SLOT_BLOCK_COUNT[slot].load(Ordering::Acquire) - 5) as u16,
        Ordering::Release,
    );
    CARD_ERROR_READY
}

pub fn set_status_async(
    chn: i32,
    fileno: i32,
    stats: *mut CardStat,
    callback: CardCallback,
) -> i32 {
    if !matches!(chn, CARD_SLOTA | CARD_SLOTB) {
        return CARD_ERROR_NOCARD;
    }
    if !valid_file_number(fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let stats_ptr = stats;
    let Some(mut stats) = snapshot_card_stat(stats_ptr) else {
        return CARD_ERROR_FATAL_ERROR;
    };
    if (stats.icon_addr != u32::MAX && stats.icon_addr >= CARD_READSIZE)
        || (stats.comment_addr != u32::MAX && stats.comment_addr % 8192 > 8128)
    {
        return CARD_ERROR_FATAL_ERROR;
    }
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let Some(mut entry) = raw_dir_entry_at(slot, fileno) else {
        return CARD_ERROR_BROKEN;
    };
    apply_status_update(&mut entry, &mut stats, current_card_time_seconds());
    unsafe {
        *stats_ptr = stats;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::SetStatus,
        chn,
        callback,
        fileno,
        stats: core::mem::MaybeUninit::new(stats),
        ..AsyncRequest::empty()
    })
}

pub fn set_status(chn: i32, fileno: i32, stats: *mut CardStat) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if stats.is_null() || !(0..CARD_MAXFILES as i32).contains(&fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    unsafe {
        if (*stats).icon_addr != u32::MAX && (*stats).icon_addr >= CARD_READSIZE {
            return CARD_ERROR_FATAL_ERROR;
        }
        if (*stats).comment_addr != u32::MAX && ((*stats).comment_addr % 8192) > 8128 {
            return CARD_ERROR_FATAL_ERROR;
        }
    }
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    let entry = unsafe { &mut (*dir).entries[fileno as usize] };
    if entry.block < 5 {
        return CARD_ERROR_NOFILE;
    }
    let timestamp = current_card_time_seconds();
    unsafe { apply_status_update(entry, &mut *stats, timestamp) };
    commit_directory(slot)
}

pub fn set_status_ex(chn: i32, fileno: i32, entry: *mut CardDirEntry) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if entry.is_null() || !(0..CARD_MAXFILES as i32).contains(&fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    if !status_entry_has_filename(unsafe { &*entry }) {
        return CARD_ERROR_FATAL_ERROR;
    }
    let mut bend = false;
    for byte in unsafe { &mut (*entry).filename } {
        if bend || *byte == 0 {
            *byte = 0;
            bend = true;
        }
    }
    let update = unsafe { *entry };
    let dst = unsafe { &mut (*dir).entries[fileno as usize] };
    let identity_changed = dst.filename != update.filename
        || dst.gamecode != update.gamecode
        || dst.company != update.company;
    if identity_changed {
        let filename_len = entry_filename_len(&update);
        if duplicate_entry_exists_with_meta(
            slot,
            &update.filename[..filename_len],
            &update.gamecode,
            &update.company,
            fileno as usize,
        ) {
            return CARD_ERROR_EXIST;
        }
    }
    apply_status_ex_update(dst, &update);
    commit_directory(slot)
}

pub fn set_attributes_async(chn: i32, fileno: i32, attr: u8, callback: CardCallback) -> i32 {
    let mut entry = CardDirEntry::default();
    let status = get_status_ex(chn, fileno, &mut entry);
    if status < 0 {
        return status;
    }
    if !status_entry_has_filename(&entry) {
        return CARD_ERROR_FATAL_ERROR;
    }
    submit_async(AsyncRequest {
        kind: AsyncKind::SetAttributes,
        chn,
        callback,
        fileno,
        attr,
        ..AsyncRequest::empty()
    })
}

pub fn set_attributes(chn: i32, fileno: i32, attr: u8) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if !valid_file_number(fileno) {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    let _metadata_guard = match lock_metadata(slot) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let Some(dir) = current_dir(slot) else {
        return CARD_ERROR_BROKEN;
    };
    if !status_entry_has_filename(unsafe { &(*dir).entries[fileno as usize] }) {
        return CARD_ERROR_FATAL_ERROR;
    }
    unsafe {
        (*dir).entries[fileno as usize].permission = attr;
    }
    commit_directory(slot)
}

pub fn get_serial_no(chn: i32, serial1: *mut u32, serial2: *mut u32) -> i32 {
    let Some(slot) = slot_index(chn) else {
        return CARD_ERROR_NOCARD;
    };
    if serial1.is_null() || serial2.is_null() {
        return CARD_ERROR_FATAL_ERROR;
    }
    if !mounted_ok(slot) {
        return CARD_ERROR_NOCARD;
    }
    refresh_serials_from_header(slot);
    unsafe {
        *serial1 = SLOT_SERIAL1[slot].load(Ordering::Acquire);
        *serial2 = SLOT_SERIAL2[slot].load(Ordering::Acquire);
    }
    CARD_ERROR_READY
}

pub fn shutdown() -> i32 {
    let mut cancelled = [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
    let mut cancelled_len = 0usize;
    let mut result = CARD_ERROR_READY;

    unsafe {
        if ASYNC_READY.load(Ordering::Acquire) {
            let mutex = ASYNC_MUTEX;
            let cond = ASYNC_COND;
            let thread = ASYNC_THREAD;
            if lwp::mutex_lock(mutex) == lwp::LWP_SUCCESSFUL {
                while ASYNC_LEN != 0 {
                    cancelled[cancelled_len] = ASYNC_QUEUE[ASYNC_HEAD];
                    ASYNC_QUEUE[ASYNC_HEAD] = AsyncRequest::empty();
                    ASYNC_HEAD = (ASYNC_HEAD + 1) % ASYNC_QUEUE_CAPACITY;
                    ASYNC_LEN -= 1;
                    cancelled_len += 1;
                }
                ASYNC_STOP.store(true, Ordering::Release);
                if lwp::cond_broadcast(cond) != lwp::LWP_SUCCESSFUL {
                    result = CARD_ERROR_FATAL_ERROR;
                }
                if lwp::mutex_unlock(mutex) != lwp::LWP_SUCCESSFUL {
                    result = CARD_ERROR_FATAL_ERROR;
                }
            } else {
                result = CARD_ERROR_FATAL_ERROR;
            }

            if thread != lwp::LWP_THREAD_NULL
                && lwp::join_thread(thread, core::ptr::null_mut()) != lwp::LWP_SUCCESSFUL
            {
                result = CARD_ERROR_FATAL_ERROR;
            }
            if cond != lwp::LWP_COND_NULL && lwp::cond_destroy(cond) != lwp::LWP_SUCCESSFUL {
                result = CARD_ERROR_FATAL_ERROR;
            }
            if mutex != lwp::LWP_MUTEX_NULL && lwp::mutex_destroy(mutex) != lwp::LWP_SUCCESSFUL {
                result = CARD_ERROR_FATAL_ERROR;
            }

            ASYNC_MUTEX = lwp::LWP_MUTEX_NULL;
            ASYNC_COND = lwp::LWP_COND_NULL;
            ASYNC_THREAD = lwp::LWP_THREAD_NULL;
            ASYNC_QUEUE = [AsyncRequest::empty(); ASYNC_QUEUE_CAPACITY];
            ASYNC_HEAD = 0;
            ASYNC_LEN = 0;
            ASYNC_READY.store(false, Ordering::Release);
        }
    }

    for request in cancelled.into_iter().take(cancelled_len) {
        // BUG FIX: A queued request marked its slot BUSY when it was accepted.
        // Shutdown cancels the request, so record the terminal result before
        // its callback to avoid leaving the slot permanently BUSY.
        record_operation_result(request.chn, CARD_ERROR_CANCELED);
        finish_callback(request.chn, CARD_ERROR_CANCELED, request.callback);
    }
    for slot in 0..2 {
        let chn = if slot == 0 { CARD_SLOTA } else { CARD_SLOTB };
        let unmount_result = unmount(chn);
        if unmount_result != CARD_ERROR_READY && unmount_result != CARD_ERROR_NOCARD {
            result = unmount_result;
        }
        SLOT_DETACH_CALLBACK[slot].store(0, Ordering::Release);
        SLOT_GENERATION[slot].fetch_add(1, Ordering::AcqRel);
    }
    result
}
