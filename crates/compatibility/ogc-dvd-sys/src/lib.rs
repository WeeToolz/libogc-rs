#![no_std]

//! Legacy libogc DVD compatibility facade backed by Wii `/dev/di` and a
//! GameCube direct-register transport.
//!
//! All asynchronous requests are serialized through one LWP worker. Completion
//! callbacks are therefore delivered after the submitting function returns.
//! Cancellation marks queued requests and suppresses active-request success.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_void};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

mod gc_firmware;
use gc_firmware::{DVD_PATCH_CODE_04, DVD_PATCH_CODE_06, DVD_PATCH_CODE_08, DVD_PATCH_CODE_Q08};

use ogc_di as di;
use ogc_ipc::{IPC_OPEN_RW, IoctlBuffers, open_file};
use ogc_lwp_sys as lwp;
pub use ogc_sys::Bool;

pub const DVD_STATE_FATAL_ERROR: i32 = -1;
pub const DVD_STATE_END: i32 = 0;
pub const DVD_STATE_BUSY: i32 = 1;
pub const DVD_STATE_WAITING: i32 = 2;
pub const DVD_STATE_COVER_CLOSED: i32 = 3;
pub const DVD_STATE_NO_DISK: i32 = 4;
pub const DVD_STATE_COVER_OPEN: i32 = 5;
pub const DVD_STATE_WRONG_DISK: i32 = 6;
pub const DVD_STATE_MOTOR_STOPPED: i32 = 7;
pub const DVD_STATE_IGNORED: i32 = 8;
pub const DVD_STATE_CANCELED: i32 = 10;
pub const DVD_STATE_RETRY: i32 = 11;

pub const DVD_ERROR_OK: i32 = 0;
pub const DVD_ERROR_FATAL: i32 = -1;
pub const DVD_ERROR_IGNORED: i32 = -2;
pub const DVD_ERROR_CANCELED: i32 = -3;
pub const DVD_ERROR_COVER_CLOSED: i32 = -4;

pub const DVD_RESETHARD: u32 = 0;
pub const DVD_RESETSOFT: u32 = 1;
pub const DVD_RESETNONE: u32 = 2;

pub const DVD_SPINMOTOR_DOWN: u32 = 0x0000_0000;
pub const DVD_SPINMOTOR_UP: u32 = 0x0000_0100;
pub const DVD_SPINMOTOR_ACCEPT: u32 = 0x0000_4000;
pub const DVD_SPINMOTOR_CHECKDISK: u32 = 0x0000_8000;

#[derive(Copy, Clone)]
enum ReadCommand {
    Absolute,
    ForBs,
}

impl ReadCommand {
    const fn value(self) -> u32 {
        match self {
            Self::Absolute => 0x0001,
            Self::ForBs => 0x0004,
        }
    }
}

const DVD_DISKIDSIZE: usize = 0x20;
const DVD_DRVINFSIZE: usize = 0x20;
const MAX_REQUESTS: usize = 17;
const MAX_USER_REQUESTS: usize = MAX_REQUESTS - 1;
const WORKER_STACK_SIZE: usize = 16 * 1024;
const WORKER_PRIORITY: u8 = 110;
const MAX_OFFSET: i64 = 8_511_160_320;
const COVER_INSERTED: u32 = di::DVD_COVER_DISC_INSERTED;
const DVD_WAIT_FOR_COVER_CLOSE: i32 = 0x79;
const DVD_DEVICE_PATH: &CStr = c"/dev/di";
const GC_DI_REG_BASE: usize = 0xCC00_6000;
const GC_PI_RESET_REG: usize = 0xCC00_3024;
const GC_DI_ERROR_INT: u32 = 1 << 2;
const GC_DI_TC_INT: u32 = 1 << 4;
const GC_DI_BREAK_INT: u32 = 1 << 6;
const GC_DI_INT_STATUS: u32 = GC_DI_ERROR_INT | GC_DI_TC_INT | GC_DI_BREAK_INT;
const GC_DI_COMMAND_TIMEOUT_TICKS: u64 = 10 * lwp::TICKS_PER_SECOND;
const GC_DI_MAX_READ_SIZE: u32 = 0x80000;
const GC_DI_DMA_START: u32 = 3;
const GC_DI_START: u32 = 1;
const GC_DVD_FW_WRITE_MEM: u32 = 0xFE01_0100;
const GC_DVD_FW_FUNC_CALL: u32 = 0xFE12_0000;
const GC_DVD_FW_ENABLE_EXT: u32 = 0x5500_0000;
const GC_DVD_FW_SET_STATUS: u32 = 0xEE00_0000;
const GC_DVD_PATCH_ADDRESS: u32 = 0x0040_D000;
const GC_DVD_MODEL_04: u32 = 0x2002_0402;
const GC_DVD_MODEL_06: u32 = 0x2001_0608;
const GC_DVD_MODEL_08: u32 = 0x2002_0823;
const GC_DVD_MODEL_Q08: u32 = 0x2001_0831;
const GC_MAX_INTERNAL_RETRIES: u8 = 2;
const GC_RC_COVER_CLOSED: i32 = i32::MIN + 1;
const GC_RC_WRONG_DISK: i32 = i32::MIN + 2;
const COVER_POLL_TICKS: u64 = if lwp::TICKS_PER_SECOND / 1_000 == 0 {
    1
} else {
    lwp::TICKS_PER_SECOND / 1_000
};

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct LwpNode {
    pub next: *mut LwpNode,
    pub prev: *mut LwpNode,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct DvdDiskId {
    pub gamename: [i8; 4],
    pub company: [i8; 2],
    pub disknum: u8,
    pub gamever: u8,
    pub streaming: u8,
    pub streambufsize: u8,
    pub pad: [u8; 22],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct DvdDrvInfo {
    pub rev_level: u16,
    pub dev_code: u16,
    pub rel_date: u32,
    pub pad: [u8; DVD_DRVINFSIZE - 8],
}

pub type DvdCbCallback = Option<unsafe extern "C" fn(result: i32, block: *mut DvdCmdBlk)>;
pub type DvdCallback = Option<unsafe extern "C" fn(result: i32, info: *mut DvdFileInfo)>;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct DvdCmdBlk {
    pub node: LwpNode,
    pub cmd: u32,
    pub state: i32,
    pub offset: i64,
    pub len: u32,
    pub buf: *mut c_void,
    pub currtxsize: u32,
    pub txdsize: u32,
    pub id: *mut DvdDiskId,
    pub cb: DvdCbCallback,
    pub usrdata: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct DvdFileInfo {
    pub block: DvdCmdBlk,
    pub addr: u32,
    pub len: u32,
    pub cb: DvdCallback,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum RequestKind {
    None,
    Mount,
    Control,
    SetOffset,
    SpinUp,
    Inquiry,
    DiskId,
    Read,
    Seek,
    StopStream,
}

#[derive(Copy, Clone)]
struct Request {
    kind: RequestKind,
    priority: u8,
    block: *mut DvdCmdBlk,
    buffer: *mut c_void,
    info: *mut DvdDrvInfo,
    id: *mut DvdDiskId,
    len: u32,
    offset: i64,
    command: u32,
    callback: DvdCbCallback,
}

impl Request {
    const fn empty() -> Self {
        Self {
            kind: RequestKind::None,
            priority: 2,
            block: ptr::null_mut(),
            buffer: ptr::null_mut(),
            info: ptr::null_mut(),
            id: ptr::null_mut(),
            len: 0,
            offset: 0,
            command: 0,
            callback: None,
        }
    }
}

fn queued_read_request(
    block: *mut DvdCmdBlk,
    buffer: *mut c_void,
    len: u32,
    offset: i64,
    priority: u8,
    command: ReadCommand,
) -> Request {
    Request {
        kind: RequestKind::Read,
        priority,
        block,
        buffer,
        len,
        offset,
        command: command.value(),
        ..Request::empty()
    }
}

#[repr(align(32))]
struct WorkerStack([u8; WORKER_STACK_SIZE]);
#[repr(align(32))]
struct Scratch([u8; 0x800]);
#[repr(C, align(32))]
struct CoverWaitIo {
    command: [u32; 8],
    output: [u8; 0x20],
}

impl CoverWaitIo {
    const fn zeroed() -> Self {
        Self {
            command: [0; 8],
            output: [0; 0x20],
        }
    }
}

fn cover_wait_command() -> [u32; 8] {
    let mut command = [0; 8];
    command[0] = (DVD_WAIT_FOR_COVER_CLOSE as u32) << 24;
    command
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static STARTING: AtomicBool = AtomicBool::new(false);
static WORKER_STOP: AtomicBool = AtomicBool::new(false);
static PAUSED: AtomicBool = AtomicBool::new(false);
static PAUSING: AtomicBool = AtomicBool::new(false);
static FATAL: AtomicBool = AtomicBool::new(false);
static DRIVE_STATUS: AtomicI32 = AtomicI32::new(DVD_STATE_NO_DISK);
static AUTO_INVALIDATION: AtomicU32 = AtomicU32::new(1);
static GCM_OFFSET_WORDS: AtomicU32 = AtomicU32::new(0);
static LAST_ERROR: AtomicU32 = AtomicU32::new(0);

static mut QUEUE_MUTEX: lwp::Mutex = lwp::LWP_MUTEX_NULL;
static mut QUEUE_COND: lwp::CondVar = lwp::LWP_COND_NULL;
static mut WORKER_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;
static mut WORKER_STACK: WorkerStack = WorkerStack([0; WORKER_STACK_SIZE]);
static mut REQUESTS: [Request; MAX_REQUESTS] = [Request::empty(); MAX_REQUESTS];
static mut QUEUE_HEAD: usize = 0;
static mut QUEUE_LEN: usize = 0;
static mut ACTIVE: *mut DvdCmdBlk = ptr::null_mut();
static MOUNT_PENDING: AtomicBool = AtomicBool::new(false);
static mut MOUNT_REQUEST: Request = Request::empty();
static mut CURRENT_DISK_ID: DvdDiskId = DvdDiskId {
    gamename: [0; 4],
    company: [0; 2],
    disknum: 0,
    gamever: 0,
    streaming: 0,
    streambufsize: 0,
    pad: [0; 22],
};
static mut DRIVE_INFO: DvdDrvInfo = DvdDrvInfo {
    rev_level: 0,
    dev_code: 0,
    rel_date: 0,
    pad: [0; DVD_DRVINFSIZE - 8],
};
static mut SYNC_BLOCK: DvdCmdBlk = DvdCmdBlk {
    node: LwpNode {
        next: ptr::null_mut(),
        prev: ptr::null_mut(),
    },
    cmd: 0,
    state: DVD_STATE_END,
    offset: 0,
    len: 0,
    buf: ptr::null_mut(),
    currtxsize: 0,
    txdsize: 0,
    id: ptr::null_mut(),
    cb: None,
    usrdata: ptr::null_mut(),
};
static mut SEEK_SCRATCH: Scratch = Scratch([0; 0x800]);

fn is_gamecube() -> bool {
    // libogc writes a zero Hollywood revision on GameCube and a nonzero
    // revision on Wii. This is the same runtime discriminator used elsewhere
    // in the Rust ports.
    unsafe { ogc_sys::sys_get_hollywood_revision() == 0 }
}

#[repr(align(32))]
struct GcDmaBuffer([u8; DVD_DISKIDSIZE]);

fn gc_di_register(index: usize) -> *mut u32 {
    (GC_DI_REG_BASE + index * size_of::<u32>()) as *mut u32
}

const fn gc_read_command_words(offset_words: u32, length: u32) -> [u32; 4] {
    [0xA800_0000, offset_words, length, length]
}

const fn gc_di_bytes_transferred(length: u32, remaining: u32) -> u32 {
    let bounded_remaining = if remaining < length {
        remaining
    } else {
        length
    };
    length.saturating_sub(bounded_remaining)
}

const fn gc_drive_state_from_error(status: u32) -> i32 {
    match status >> 24 {
        0 => DVD_STATE_END,
        1 => DVD_STATE_COVER_OPEN,
        2 => DVD_STATE_COVER_CLOSED,
        3 => DVD_STATE_NO_DISK,
        4 => DVD_STATE_MOTOR_STOPPED,
        5 => DVD_STATE_RETRY,
        _ => DVD_STATE_RETRY,
    }
}

fn gc_di_command(
    command: u32,
    arg1: u32,
    arg2: u32,
    buffer: *mut c_void,
    length: u32,
    dma: bool,
) -> i32 {
    gc_di_command_with_progress(command, arg1, arg2, buffer, length, dma).0
}

fn gc_di_command_with_progress(
    command: u32,
    arg1: u32,
    arg2: u32,
    buffer: *mut c_void,
    length: u32,
    dma: bool,
) -> (i32, u32) {
    if dma && (buffer.is_null() || (buffer as usize & 31) != 0) {
        return (-22, 0);
    }
    if dma {
        unsafe {
            ogc_cache::dc_invalidate_range(buffer.cast_const(), length);
        }
    }

    unsafe {
        let status_reg = gc_di_register(0);
        let status = ptr::read_volatile(status_reg);
        ptr::write_volatile(status_reg, status & GC_DI_INT_STATUS);
        ptr::write_volatile(gc_di_register(2), command);
        ptr::write_volatile(gc_di_register(3), arg1);
        ptr::write_volatile(gc_di_register(4), arg2);
        if dma {
            ptr::write_volatile(gc_di_register(5), buffer as u32);
            ptr::write_volatile(gc_di_register(6), length);
        }
        ptr::write_volatile(gc_di_register(7), if dma { 3 } else { 1 });

        let start = lwp::now_ticks();
        while (ptr::read_volatile(gc_di_register(7)) & 1) != 0 {
            if lwp::now_ticks().wrapping_sub(start) >= GC_DI_COMMAND_TIMEOUT_TICKS {
                return (-5, 0);
            }
            core::hint::spin_loop();
        }

        let status = ptr::read_volatile(status_reg);
        let raised = status & GC_DI_INT_STATUS;
        ptr::write_volatile(status_reg, raised);
        let transferred = if dma {
            let remaining = ptr::read_volatile(gc_di_register(6));
            let transferred = gc_di_bytes_transferred(length, remaining);
            if transferred != 0 {
                ogc_cache::dc_invalidate_range(buffer.cast_const(), transferred);
            }
            transferred
        } else {
            0
        };
        if (raised & (GC_DI_ERROR_INT | GC_DI_BREAK_INT)) != 0 || (raised & GC_DI_TC_INT) == 0 {
            (-5, transferred)
        } else {
            (0, transferred)
        }
    }
}

fn gc_di_command_control(command: u32, arg1: u32, arg2: u32, control: u32) -> i32 {
    unsafe {
        let status_reg = gc_di_register(0);
        let status = ptr::read_volatile(status_reg);
        ptr::write_volatile(status_reg, status & GC_DI_INT_STATUS);
        ptr::write_volatile(gc_di_register(2), command);
        ptr::write_volatile(gc_di_register(3), arg1);
        ptr::write_volatile(gc_di_register(4), arg2);
        ptr::write_volatile(gc_di_register(7), control);

        let start = lwp::now_ticks();
        while (ptr::read_volatile(gc_di_register(7)) & 1) != 0 {
            if lwp::now_ticks().wrapping_sub(start) >= GC_DI_COMMAND_TIMEOUT_TICKS {
                return -5;
            }
            core::hint::spin_loop();
        }

        let status = ptr::read_volatile(status_reg);
        let raised = status & GC_DI_INT_STATUS;
        ptr::write_volatile(status_reg, raised);
        if (raised & (GC_DI_ERROR_INT | GC_DI_BREAK_INT)) != 0 || (raised & GC_DI_TC_INT) == 0 {
            -5
        } else {
            0
        }
    }
}

fn gc_patch_for_model(rel_date: u32) -> Option<&'static [u8; 448]> {
    match rel_date {
        GC_DVD_MODEL_04 => Some(&DVD_PATCH_CODE_04),
        GC_DVD_MODEL_06 => Some(&DVD_PATCH_CODE_06),
        GC_DVD_MODEL_08 => Some(&DVD_PATCH_CODE_08),
        GC_DVD_MODEL_Q08 => Some(&DVD_PATCH_CODE_Q08),
        _ => None,
    }
}

fn gc_di_unlock_drive() -> i32 {
    for words in [
        [0xFF01_4D41, 0x5453_4849, 0x5441_0200],
        [0xFF00_4456, 0x442D_4741, 0x4D45_0300],
    ] {
        let rc = gc_di_command_control(words[0], words[1], words[2], GC_DI_START);
        if rc != 0 {
            return rc;
        }
    }
    0
}

fn gc_di_patch_drive(info: &DvdDrvInfo) -> i32 {
    let Some(patch) = gc_patch_for_model(info.rel_date) else {
        // C has no patch data for unknown models. Continue without injecting
        // firmware rather than issuing an undefined drive-memory sequence.
        return 0;
    };

    let rc = gc_di_unlock_drive();
    if rc != 0 {
        return rc;
    }

    let mut address = GC_DVD_PATCH_ADDRESS;
    for chunk in patch.chunks(12) {
        let rc = gc_di_command_control(
            GC_DVD_FW_WRITE_MEM,
            address,
            (chunk.len() as u32) << 16,
            GC_DI_DMA_START,
        );
        if rc != 0 {
            return rc;
        }

        let mut payload = [0u8; 12];
        payload[..chunk.len()].copy_from_slice(chunk);
        let words = [
            u32::from_be_bytes(payload[0..4].try_into().unwrap()),
            u32::from_be_bytes(payload[4..8].try_into().unwrap()),
            u32::from_be_bytes(payload[8..12].try_into().unwrap()),
        ];
        let rc = gc_di_command_control(words[0], words[1], words[2], GC_DI_START);
        if rc != 0 {
            return rc;
        }
        address += chunk.len() as u32;
    }

    gc_di_command_control(
        GC_DVD_FW_FUNC_CALL,
        GC_DVD_PATCH_ADDRESS,
        0x6675_6E63,
        GC_DI_START,
    )
}

fn gc_di_prepare_extensions(info: &DvdDrvInfo) -> i32 {
    let rc = gc_di_patch_drive(info);
    if rc != 0 {
        return rc;
    }
    let rc = gc_di_unlock_drive();
    if rc != 0 {
        return rc;
    }
    let rc = gc_di_command_control(GC_DVD_FW_ENABLE_EXT | (1 << 16), 0, 0, GC_DI_START);
    if rc != 0 {
        return rc;
    }
    let rc = gc_di_spin_motor(DVD_SPINMOTOR_ACCEPT | DVD_SPINMOTOR_UP);
    if rc != 0 {
        return rc;
    }
    if LAST_ERROR.load(Ordering::Acquire) == 0 {
        return gc_di_command_control(GC_DVD_FW_SET_STATUS | 0x0006_0300, 0, 0, GC_DI_START);
    }
    0
}

fn gc_di_initialize() {
    unsafe {
        ptr::write_volatile(gc_di_register(0), 0);
        ptr::write_volatile(gc_di_register(1), 0);
    }
}

fn gc_di_reset(reset_mode: u32) {
    unsafe {
        ptr::write_volatile(gc_di_register(1), 0);
        let mut value = ptr::read_volatile(GC_PI_RESET_REG as *const u32);
        ptr::write_volatile(GC_PI_RESET_REG as *mut u32, (value & !4) | 1);
        let start = lwp::now_ticks();
        let delay = ((lwp::TICKS_PER_SECOND * 12) / 1_000_000).max(1);
        while lwp::now_ticks().wrapping_sub(start) < delay {
            core::hint::spin_loop();
        }
        if reset_mode == DVD_RESETHARD {
            value |= 4;
        }
        value |= 1;
        ptr::write_volatile(GC_PI_RESET_REG as *mut u32, value);
        ptr::write_volatile(gc_di_register(0), 0);
    }
}

fn gc_di_read(buffer: *mut c_void, length: u32, offset_words: u32) -> (i32, u32) {
    if buffer.is_null() || length == 0 || (buffer as usize & 31) != 0 {
        return (-22, 0);
    }
    let [command, arg1, arg2, transfer_len] = gc_read_command_words(offset_words, length);
    gc_di_command_with_progress(command, arg1, arg2, buffer, transfer_len, true)
}

fn run_gc_read_chunks(length: u32, mut submit: impl FnMut(u32, u32) -> (i32, u32)) -> (i32, u32) {
    let mut transferred = 0;
    while transferred < length {
        let chunk_len = (length - transferred).min(GC_DI_MAX_READ_SIZE);
        let (result, chunk_transferred) = submit(transferred, chunk_len);
        if result != 0 {
            return (
                result,
                transferred.saturating_add(chunk_transferred.min(chunk_len)),
            );
        }
        transferred += chunk_len;
    }
    (0, transferred)
}

fn gc_di_read_chunks(buffer: *mut c_void, length: u32, offset_words: u32) -> (i32, u32) {
    if buffer.is_null() || length == 0 || (buffer as usize & 31) != 0 {
        return (-22, 0);
    }
    run_gc_read_chunks(length, |transferred, chunk_len| {
        let Some(chunk_offset_words) = offset_words.checked_add(transferred >> 2) else {
            return (-22, 0);
        };
        let chunk_buffer = unsafe {
            buffer
                .cast::<u8>()
                .add(transferred as usize)
                .cast::<c_void>()
        };
        gc_di_read(chunk_buffer, chunk_len, chunk_offset_words)
    })
}

fn gc_di_seek(offset_words: u32) -> i32 {
    gc_di_command(0xAB00_0000, offset_words, 0, ptr::null_mut(), 0, false)
}

fn gc_di_request_error() -> Result<u32, i32> {
    let result = gc_di_command(0xE000_0000, 0, 0, ptr::null_mut(), 0, false);
    if result == 0 {
        Ok(unsafe { ptr::read_volatile(gc_di_register(8)) })
    } else {
        Err(result)
    }
}

fn gc_di_stop_motor() -> i32 {
    gc_di_command(0xE300_0000, 0, 0, ptr::null_mut(), 0, false)
}

fn gc_di_spin_motor(mode: u32) -> i32 {
    gc_di_command(
        0xFE11_0000 | (mode & 0x0000_ff00),
        0,
        0,
        ptr::null_mut(),
        0,
        false,
    )
}

fn gc_di_read_disk_id(output: &mut [u8; DVD_DISKIDSIZE]) -> i32 {
    let mut dma = GcDmaBuffer([0; DVD_DISKIDSIZE]);
    let rc = gc_di_command(
        0xA800_0040,
        0,
        DVD_DISKIDSIZE as u32,
        dma.0.as_mut_ptr().cast(),
        DVD_DISKIDSIZE as u32,
        true,
    );
    if rc == 0 {
        output.copy_from_slice(&dma.0);
    }
    rc
}

fn gc_di_inquiry(info: &mut DvdDrvInfo) -> i32 {
    let mut dma = GcDmaBuffer([0; DVD_DRVINFSIZE]);
    let rc = gc_di_command(
        0x1200_0000,
        0,
        DVD_DRVINFSIZE as u32,
        dma.0.as_mut_ptr().cast(),
        DVD_DRVINFSIZE as u32,
        true,
    );
    if rc == 0 {
        *info = DvdDrvInfo {
            rev_level: u16::from_be_bytes([dma.0[0], dma.0[1]]),
            dev_code: u16::from_be_bytes([dma.0[2], dma.0[3]]),
            rel_date: u32::from_be_bytes([dma.0[4], dma.0[5], dma.0[6], dma.0[7]]),
            pad: [0; DVD_DRVINFSIZE - 8],
        };
    }
    rc
}

fn gc_di_drive_status() -> i32 {
    match gc_di_request_error() {
        Ok(status) => gc_drive_state_from_error(status),
        Err(_) => DVD_STATE_FATAL_ERROR,
    }
}

fn gc_status_has_disc(status: u32) -> bool {
    matches!(status >> 24, 0 | 2 | 4 | 5)
}

fn gc_wait_for_cover_close() -> i32 {
    loop {
        match gc_di_request_error() {
            Ok(status) if gc_status_has_disc(status) => return 0,
            Ok(_) => lwp::sleep_ticks(COVER_POLL_TICKS),
            Err(error) => return error,
        }
    }
}

fn gc_cover_is_inserted() -> Result<bool, i32> {
    gc_di_request_error().map(gc_status_has_disc)
}

fn gc_read_current_disk_id() -> [u8; DVD_DISKIDSIZE] {
    let mut disk_id = [0; DVD_DISKIDSIZE];
    unsafe {
        ptr::copy_nonoverlapping(
            ptr::addr_of!(CURRENT_DISK_ID).cast::<u8>(),
            disk_id.as_mut_ptr(),
            DVD_DISKIDSIZE,
        );
    }
    disk_id
}

fn gc_spin_up_and_read_id(output: &mut [u8; DVD_DISKIDSIZE]) -> i32 {
    let mut retries = 0u8;
    loop {
        let spin_result = gc_di_spin_motor(DVD_SPINMOTOR_UP);
        if spin_result != 0 {
            return spin_result;
        }
        let rc = gc_di_read_disk_id(output);
        if rc == 0 {
            return 0;
        }
        let status = match gc_di_request_error() {
            Ok(status) => status,
            Err(error) => return error,
        };
        LAST_ERROR.store(status & 0x00ff_ffff, Ordering::Release);
        match status >> 24 {
            1 | 3 => {
                let _ = gc_di_stop_motor();
                let wait_result = gc_wait_for_cover_close();
                if wait_result != 0 {
                    return wait_result;
                }
                retries = 0;
            }
            0 | 5 if retries < GC_MAX_INTERNAL_RETRIES => {
                retries += 1;
            }
            _ => return rc,
        }
    }
}

fn gc_remount_current_disk() -> i32 {
    let expected = gc_read_current_disk_id();
    let mut disk_id = [0; DVD_DISKIDSIZE];
    let rc = gc_spin_up_and_read_id(&mut disk_id);
    if rc != 0 {
        return rc;
    }
    if disk_id != expected {
        return GC_RC_WRONG_DISK;
    }
    0
}

fn gc_di_mount() -> i32 {
    gc_di_initialize();
    gc_di_reset(DVD_RESETHARD);
    lwp::sleep_ticks((1_150 * lwp::TICKS_PER_SECOND) / 1_000);

    // C probes the disk ID before deciding whether the drive needs the
    // unlock/firmware-extension path. A successful probe means an add-on or
    // already-compatible drive is present and no patch is required.
    let mut disk_id = [0; DVD_DISKIDSIZE];
    let probe_rc = gc_di_read_disk_id(&mut disk_id);
    let mut info = DvdDrvInfo::default();
    if probe_rc != 0 {
        let rc = gc_di_inquiry(&mut info);
        if rc != 0 {
            return rc;
        }
        unsafe { DRIVE_INFO = info };
        let rc = gc_di_prepare_extensions(&info);
        if rc != 0 {
            return rc;
        }
    }

    // libogc's mount callback performs a fresh disk-ID read after spin-up.
    // Keep that result authoritative even when the initial add-on probe worked.
    let rc = gc_read_disk_id_with_recovery(&mut disk_id);
    if rc != 0 {
        return rc;
    }
    unsafe {
        ptr::copy_nonoverlapping(
            disk_id.as_ptr(),
            ptr::addr_of_mut!(CURRENT_DISK_ID).cast::<u8>(),
            DVD_DISKIDSIZE,
        );
    }

    if probe_rc == 0 {
        let rc = gc_di_inquiry(&mut info);
        if rc != 0 {
            return rc;
        }
        unsafe { DRIVE_INFO = info };
    }
    0
}

fn gc_read_chunks_with_recovery(
    buffer: *mut c_void,
    length: u32,
    offset_words: u32,
    command: u32,
) -> (i32, u32) {
    if command != ReadCommand::Absolute.value() {
        return gc_di_read_chunks(buffer, length, offset_words);
    }

    let mut transferred = 0u32;
    let mut retries = 0u8;
    while transferred < length {
        let remaining = length - transferred;
        let Some(current_offset_words) = offset_words.checked_add(transferred >> 2) else {
            return (-22, transferred);
        };
        let current_buffer = unsafe {
            buffer
                .cast::<u8>()
                .add(transferred as usize)
                .cast::<c_void>()
        };
        let (rc, completed) = gc_di_read_chunks(current_buffer, remaining, current_offset_words);
        transferred = transferred.saturating_add(completed.min(remaining));
        if rc == 0 || transferred == length {
            return (0, transferred);
        }

        let status = match gc_di_request_error() {
            Ok(status) => status,
            Err(error) => return (error, transferred),
        };
        LAST_ERROR.store(status & 0x00ff_ffff, Ordering::Release);
        match status >> 24 {
            0 | 5 if retries < GC_MAX_INTERNAL_RETRIES => {
                // C seeks before retrying an unrecoverable read and otherwise
                // reissues the command from the bytes already transferred.
                if (status & 0x00ff_ffff) == 0x031100 {
                    let _ = gc_di_seek(current_offset_words);
                }
                retries += 1;
            }
            1..=3 => {
                if (status >> 24) != 2 {
                    let _ = gc_di_stop_motor();
                    let wait_result = gc_wait_for_cover_close();
                    if wait_result != 0 {
                        return (wait_result, transferred);
                    }
                }
                let remount_result = gc_remount_current_disk();
                if remount_result != 0 {
                    return (remount_result, transferred);
                }
                retries = 0;
            }
            4 if retries < GC_MAX_INTERNAL_RETRIES => {
                let spin_result = gc_di_spin_motor(DVD_SPINMOTOR_UP);
                if spin_result != 0 {
                    return (spin_result, transferred);
                }
                retries += 1;
            }
            _ => return (rc, transferred),
        }
    }
    (0, transferred)
}

fn gc_read_disk_id_with_recovery(output: &mut [u8; DVD_DISKIDSIZE]) -> i32 {
    let mut retries = 0u8;
    loop {
        let rc = gc_di_read_disk_id(output);
        if rc == 0 {
            return 0;
        }
        let status = match gc_di_request_error() {
            Ok(status) => status,
            Err(error) => return error,
        };
        LAST_ERROR.store(status & 0x00ff_ffff, Ordering::Release);
        match status >> 24 {
            0 | 5 if retries < GC_MAX_INTERNAL_RETRIES => {
                let spin_result = gc_di_spin_motor(DVD_SPINMOTOR_UP);
                if spin_result != 0 {
                    return spin_result;
                }
                retries += 1;
            }
            1 | 3 => {
                let _ = gc_di_stop_motor();
                let wait_result = gc_wait_for_cover_close();
                if wait_result != 0 {
                    return wait_result;
                }
                return GC_RC_COVER_CLOSED;
            }
            2 => return GC_RC_COVER_CLOSED,
            _ => return rc,
        }
    }
}

fn gc_seek_with_recovery(offset_words: u32) -> i32 {
    let mut retries = 0u8;
    loop {
        let rc = gc_di_seek(offset_words);
        if rc == 0 {
            return 0;
        }
        let status = match gc_di_request_error() {
            Ok(status) => status,
            Err(error) => return error,
        };
        LAST_ERROR.store(status & 0x00ff_ffff, Ordering::Release);
        match status >> 24 {
            0 | 5 if retries < GC_MAX_INTERNAL_RETRIES => retries += 1,
            1..=3 => {
                if (status >> 24) != 2 {
                    let _ = gc_di_stop_motor();
                    let wait_result = gc_wait_for_cover_close();
                    if wait_result != 0 {
                        return wait_result;
                    }
                }
                let remount_result = gc_remount_current_disk();
                if remount_result != 0 {
                    return remount_result;
                }
                retries = 0;
            }
            4 if retries < GC_MAX_INTERNAL_RETRIES => {
                let spin_result = gc_di_spin_motor(DVD_SPINMOTOR_UP);
                if spin_result != 0 {
                    return spin_result;
                }
                retries += 1;
            }
            _ => return rc,
        }
    }
}

fn cover_state() -> i32 {
    if is_gamecube() {
        return gc_di_drive_status();
    }
    let mut cover = 0u32;
    if di::get_cover_register(&mut cover) != 0 {
        return DVD_STATE_FATAL_ERROR;
    }
    if (cover & COVER_INSERTED) == 0 {
        DVD_STATE_NO_DISK
    } else {
        DVD_STATE_END
    }
}

fn map_error(rc: i32) -> (i32, i32) {
    if rc == GC_RC_COVER_CLOSED {
        return (DVD_STATE_COVER_CLOSED, DVD_ERROR_COVER_CLOSED);
    }
    if rc == GC_RC_WRONG_DISK {
        return (DVD_STATE_FATAL_ERROR, DVD_ERROR_FATAL);
    }
    if rc == 0 {
        LAST_ERROR.store(0, Ordering::Release);
        return (DVD_STATE_END, DVD_ERROR_OK);
    }

    if is_gamecube() {
        if let Ok(drive_error) = gc_di_request_error() {
            LAST_ERROR.store(drive_error & 0x00ff_ffff, Ordering::Release);
            return match gc_drive_state_from_error(drive_error) {
                DVD_STATE_COVER_OPEN => (DVD_STATE_COVER_OPEN, DVD_ERROR_COVER_CLOSED),
                DVD_STATE_COVER_CLOSED => (DVD_STATE_COVER_CLOSED, DVD_ERROR_COVER_CLOSED),
                DVD_STATE_NO_DISK => (DVD_STATE_NO_DISK, DVD_ERROR_COVER_CLOSED),
                DVD_STATE_MOTOR_STOPPED => (DVD_STATE_MOTOR_STOPPED, DVD_ERROR_IGNORED),
                DVD_STATE_WRONG_DISK => (DVD_STATE_WRONG_DISK, DVD_ERROR_IGNORED),
                _ => (DVD_STATE_RETRY, DVD_ERROR_IGNORED),
            };
        }
        LAST_ERROR.store(rc as u32, Ordering::Release);
        return (DVD_STATE_FATAL_ERROR, DVD_ERROR_FATAL);
    }

    let mut drive_error = 0u32;
    if di::get_error(&mut drive_error) == 0 {
        LAST_ERROR.store(drive_error, Ordering::Release);
    } else {
        LAST_ERROR.store(rc as u32, Ordering::Release);
    }

    let status = di::get_status() as u32;
    if status & di::DVD_NO_DISC != 0 || cover_state() == DVD_STATE_NO_DISK {
        (DVD_STATE_NO_DISK, DVD_ERROR_COVER_CLOSED)
    } else if status & di::DVD_IOS_ERROR != 0 {
        FATAL.store(true, Ordering::Release);
        (DVD_STATE_FATAL_ERROR, DVD_ERROR_FATAL)
    } else {
        (DVD_STATE_RETRY, DVD_ERROR_IGNORED)
    }
}

enum BsReadRecovery {
    NotRequired,
    CoverClosed,
    Canceled,
    Failed(i32),
}

fn wait_for_cover_close() -> i32 {
    let fd = match open_file(DVD_DEVICE_PATH, IPC_OPEN_RW) {
        Ok(fd) => fd,
        Err(error) => return error,
    };
    let mut io = CoverWaitIo::zeroed();
    io.command = cover_wait_command();
    let input =
        unsafe { core::slice::from_raw_parts_mut(io.command.as_mut_ptr().cast::<u8>(), 0x20) };
    let rc = fd.ioctl(
        DVD_WAIT_FOR_COVER_CLOSE,
        IoctlBuffers::from_slices(Some(input), Some(&mut io.output)),
    );
    let operation_result = match rc {
        1 => 0,
        2 => -5,
        value if value < 0 => value,
        _ => -5,
    };
    let close_result = fd.close();
    if operation_result != 0 {
        operation_result
    } else {
        close_result.map(|()| 0).unwrap_or_else(|error| error)
    }
}

fn cover_is_inserted() -> Result<bool, i32> {
    if is_gamecube() {
        return gc_cover_is_inserted();
    }
    let mut cover = 0;
    let result = di::get_cover_register(&mut cover);
    if result == 0 {
        Ok((cover & COVER_INSERTED) != 0)
    } else {
        Err(result)
    }
}

fn recover_bs_read(
    command: u32,
    read_result: i32,
    mut stop_motor: impl FnMut() -> i32,
    mut cover_is_inserted: impl FnMut() -> Result<bool, i32>,
    mut wait_cover_close: impl FnMut() -> i32,
    mut canceled: impl FnMut() -> bool,
    mut sleep: impl FnMut(),
) -> BsReadRecovery {
    if read_result == 0 || command != ReadCommand::ForBs.value() {
        return BsReadRecovery::NotRequired;
    }

    if canceled() {
        return BsReadRecovery::Canceled;
    }
    let stop_result = stop_motor();
    if stop_result != 0 {
        return BsReadRecovery::Failed(stop_result);
    }

    // BUG FIX: C routes command 0x0004 read failures through stop-motor and
    // cover-close handling before it completes with DVD_ERROR_COVER_CLOSED.
    // IOS may return success from DVD_WAITFORCOVERCLOSE immediately when a
    // disc is already inserted. First observe that the cover is open, then
    // use the IOS wait operation to wait for the subsequent close event.
    loop {
        if canceled() {
            return BsReadRecovery::Canceled;
        }
        match cover_is_inserted() {
            Ok(false) => break,
            Ok(true) => sleep(),
            Err(error) => return BsReadRecovery::Failed(error),
        }
    }
    if canceled() {
        return BsReadRecovery::Canceled;
    }
    let wait_result = wait_cover_close();
    if canceled() {
        return BsReadRecovery::Canceled;
    }
    if wait_result == 0 {
        BsReadRecovery::CoverClosed
    } else {
        BsReadRecovery::Failed(wait_result)
    }
}

unsafe fn complete(request: Request, state: i32, result: i32, transferred: u32) {
    let mutex = unsafe { QUEUE_MUTEX };
    let cond = unsafe { QUEUE_COND };
    if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
        FATAL.store(true, Ordering::Release);
        return;
    }

    unsafe {
        if !request.block.is_null() {
            (*request.block).state = state;
            (*request.block).currtxsize = transferred;
            (*request.block).txdsize = transferred;
        }
        DRIVE_STATUS.store(state, Ordering::Release);
        if ACTIVE == request.block {
            ACTIVE = ptr::null_mut();
        }
    }
    let _ = lwp::mutex_unlock(mutex);

    // C makes the command block terminal and clears the executing request
    // before the callback, then advances/wakes the scheduler after it. Keep
    // that externally visible callback ordering while avoiding a callback
    // under the queue mutex.
    if let Some(callback) = request.callback {
        unsafe { callback(result, request.block) };
    }
    if lwp::mutex_lock(mutex) == lwp::LWP_SUCCESSFUL {
        let _ = lwp::cond_broadcast(cond);
        let _ = lwp::mutex_unlock(mutex);
    } else {
        FATAL.store(true, Ordering::Release);
    }
}

fn fail_queued_requests_after_bs_cover_close() {
    let mut failed = [Request::empty(); MAX_REQUESTS];
    let mut failed_len = 0usize;

    unsafe {
        let mutex = QUEUE_MUTEX;
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            return;
        }
        for index in 0..QUEUE_LEN {
            failed[failed_len] = REQUESTS[(QUEUE_HEAD + index) % MAX_REQUESTS];
            failed_len += 1;
        }
        REQUESTS = [Request::empty(); MAX_REQUESTS];
        QUEUE_HEAD = 0;
        QUEUE_LEN = 0;
        let _ = lwp::mutex_unlock(mutex);
    }

    // C drops these queued commands when a BS read reaches the cover-close
    // state. Complete them so Rust callers do not retain BUSY command blocks.
    for request in failed.into_iter().take(failed_len) {
        unsafe {
            complete(request, DVD_STATE_COVER_CLOSED, DVD_ERROR_COVER_CLOSED, 0);
        }
    }
}

unsafe fn execute(request: Request) {
    unsafe {
        let mut read_transferred = 0;
        let rc = match request.kind {
            RequestKind::Mount => {
                if MOUNT_PENDING
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    complete(request, DVD_STATE_RETRY, DVD_ERROR_IGNORED, 0);
                    return;
                }
                if is_gamecube() {
                    let rc = gc_di_mount();
                    MOUNT_PENDING.store(false, Ordering::Release);
                    rc
                } else {
                    MOUNT_REQUEST = request;
                    let init_rc = di::init();
                    if init_rc < 0 {
                        MOUNT_PENDING.store(false, Ordering::Release);
                        let (state, result) = map_error(init_rc);
                        complete(request, state, result, 0);
                        return;
                    }
                    di::set_mount_callback(Some(mount_complete_callback));
                    di::mount();
                    return;
                }
            }
            RequestKind::Control => match request.command & 0x0000_ff00 {
                DVD_SPINMOTOR_UP => {
                    if is_gamecube() {
                        gc_di_spin_motor(DVD_SPINMOTOR_UP)
                    } else {
                        di::reset()
                    }
                }
                DVD_SPINMOTOR_DOWN => {
                    if is_gamecube() {
                        gc_di_stop_motor()
                    } else {
                        di::stop_motor()
                    }
                }
                _ => {
                    if (request.command & DVD_SPINMOTOR_CHECKDISK) != 0 {
                        let state = cover_state();
                        if state == DVD_STATE_END { 0 } else { -1 }
                    } else {
                        -22
                    }
                }
            },
            RequestKind::SetOffset => {
                if request.offset < 0 || request.offset >= MAX_OFFSET || (request.offset & 3) != 0 {
                    -22
                } else {
                    let words = (request.offset as u64) >> 2;
                    if words > u32::MAX as u64 {
                        -22
                    } else {
                        GCM_OFFSET_WORDS.store(words as u32, Ordering::Release);
                        0
                    }
                }
            }
            RequestKind::SpinUp => {
                if is_gamecube() {
                    gc_di_spin_motor(DVD_SPINMOTOR_UP)
                } else {
                    di::reset()
                }
            }
            RequestKind::Inquiry => {
                if request.info.is_null() {
                    -22
                } else {
                    let mut info = DvdDrvInfo::default();
                    let rc = if is_gamecube() {
                        gc_di_inquiry(&mut info)
                    } else {
                        let mut id = di::DiDriveId::default();
                        let rc = di::identify(&mut id);
                        if rc == 0 {
                            info.rev_level = id.rev;
                            info.dev_code = id.dev_code;
                            info.rel_date = id.rel_date;
                        }
                        rc
                    };
                    if rc == 0 {
                        *request.info = info;
                        DRIVE_INFO = info;
                    }
                    rc
                }
            }
            RequestKind::DiskId => {
                if request.id.is_null() {
                    -22
                } else {
                    let mut bytes = [0u8; DVD_DISKIDSIZE];
                    let rc = if is_gamecube() {
                        gc_read_disk_id_with_recovery(&mut bytes)
                    } else {
                        di::read_disc_id_bytes(&mut bytes)
                    };
                    if rc == 0 {
                        ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            request.id.cast::<u8>(),
                            DVD_DISKIDSIZE,
                        );
                        CURRENT_DISK_ID = *request.id;
                    }
                    rc
                }
            }
            RequestKind::Read => {
                let mut submitted_to_di = false;
                let read_result = if request.buffer.is_null()
                    || request.len == 0
                    || request.offset < 0
                    || request.offset >= MAX_OFFSET
                    || (request.offset & 3) != 0
                    || (request.buffer as usize & 31) != 0
                {
                    -22
                } else {
                    let absolute_words = ((request.offset as u64) >> 2)
                        .saturating_add(GCM_OFFSET_WORDS.load(Ordering::Acquire) as u64);
                    if absolute_words > u32::MAX as u64 {
                        -22
                    } else {
                        let absolute_bytes = absolute_words << 2;
                        submitted_to_di = true;
                        if is_gamecube() {
                            let (result, transferred) = gc_read_chunks_with_recovery(
                                request.buffer,
                                request.len,
                                absolute_words as u32,
                                request.command,
                            );
                            read_transferred = transferred;
                            result
                        } else {
                            let result =
                                if (absolute_bytes & 0x7ff) == 0 && (request.len & 0x7ff) == 0 {
                                    di::read_dvd(
                                        request.buffer,
                                        request.len >> 11,
                                        (absolute_bytes >> 11) as u32,
                                    )
                                } else {
                                    di::unencrypted_read(
                                        request.buffer,
                                        request.len,
                                        absolute_words as u32,
                                    )
                                };
                            if result == 0 {
                                read_transferred = request.len;
                            }
                            result
                        }
                    }
                };

                match recover_bs_read(
                    if submitted_to_di { request.command } else { 0 },
                    read_result,
                    || {
                        if is_gamecube() {
                            gc_di_stop_motor()
                        } else {
                            di::stop_motor()
                        }
                    },
                    || {
                        if is_gamecube() {
                            gc_cover_is_inserted()
                        } else {
                            cover_is_inserted()
                        }
                    },
                    || {
                        if is_gamecube() {
                            gc_wait_for_cover_close()
                        } else {
                            wait_for_cover_close()
                        }
                    },
                    || false,
                    || lwp::sleep_ticks(COVER_POLL_TICKS),
                ) {
                    BsReadRecovery::NotRequired => read_result,
                    BsReadRecovery::CoverClosed => {
                        fail_queued_requests_after_bs_cover_close();
                        complete(request, DVD_STATE_COVER_CLOSED, DVD_ERROR_COVER_CLOSED, 0);
                        return;
                    }
                    BsReadRecovery::Canceled => {
                        complete(request, DVD_STATE_CANCELED, DVD_ERROR_CANCELED, 0);
                        return;
                    }
                    BsReadRecovery::Failed(error) => error,
                }
            }
            RequestKind::Seek => {
                if request.offset <= 0 || request.offset >= MAX_OFFSET || (request.offset & 3) != 0
                {
                    -22
                } else {
                    // IOS does not expose an independent positioning primitive. A
                    // small real read forces the drive/IOS path to validate and
                    // position at the requested byte offset.
                    let absolute_words = ((request.offset as u64) >> 2)
                        .saturating_add(GCM_OFFSET_WORDS.load(Ordering::Acquire) as u64);
                    if absolute_words > u32::MAX as u64 {
                        -22
                    } else {
                        if is_gamecube() {
                            gc_seek_with_recovery(absolute_words as u32)
                        } else {
                            let scratch = ptr::addr_of_mut!(SEEK_SCRATCH.0).cast::<c_void>();
                            di::unencrypted_read(scratch, 0x20, absolute_words as u32)
                        }
                    }
                }
            }
            RequestKind::StopStream => {
                if is_gamecube() {
                    gc_di_command(0xE100_0000, 0, 0, ptr::null_mut(), 0, false)
                } else {
                    di::audio_stream(0, 0, 0)
                }
            }
            RequestKind::None => -22,
        };

        let (state, mapped_result) = map_error(rc);
        let transferred = match request.kind {
            RequestKind::Read => read_transferred,
            RequestKind::Mount | RequestKind::Inquiry | RequestKind::DiskId if rc == 0 => 0x20,
            RequestKind::Control
            | RequestKind::SetOffset
            | RequestKind::SpinUp
            | RequestKind::Seek
            | RequestKind::StopStream
                if rc == 0 =>
            {
                1
            }
            _ => 0,
        };
        let result = if rc == 0 {
            match request.kind {
                // C completes read-like commands with their transferred byte
                // count. Non-read control commands complete with zero.
                RequestKind::Mount | RequestKind::DiskId => DVD_DISKIDSIZE as i32,
                RequestKind::Read => transferred as i32,
                _ => mapped_result,
            }
        } else {
            mapped_result
        };
        complete(request, state, result, transferred);
    }
}

unsafe extern "C" fn mount_complete_callback(_status: u32, _error: u32) -> i32 {
    if !MOUNT_PENDING.swap(false, Ordering::AcqRel) {
        return 0;
    }
    let request = unsafe { MOUNT_REQUEST };
    if di::get_status() == di::DVD_READY as i32 {
        unsafe {
            complete(
                request,
                DVD_STATE_END,
                DVD_DISKIDSIZE as i32,
                DVD_DISKIDSIZE as u32,
            )
        };
    } else {
        let (state, result) = map_error(-1);
        unsafe { complete(request, state, result, 0) };
    }
    0
}
unsafe extern "C" fn worker_main(_arg: *mut c_void) -> *mut c_void {
    unsafe {
        loop {
            let mutex = QUEUE_MUTEX;
            let cond = QUEUE_COND;
            let _ = lwp::mutex_lock(mutex);
            while QUEUE_LEN == 0 && !WORKER_STOP.load(Ordering::Acquire) {
                let _ = lwp::cond_wait(cond, mutex);
            }
            if QUEUE_LEN == 0 && WORKER_STOP.load(Ordering::Acquire) {
                let _ = lwp::mutex_unlock(mutex);
                return ptr::null_mut();
            }
            let request = REQUESTS[QUEUE_HEAD];
            REQUESTS[QUEUE_HEAD] = Request::empty();
            QUEUE_HEAD = (QUEUE_HEAD + 1) % MAX_REQUESTS;
            QUEUE_LEN -= 1;
            ACTIVE = request.block;
            let _ = lwp::mutex_unlock(mutex);
            execute(request);
        }
    }
}
fn ensure_worker() -> bool {
    if INITIALIZED.load(Ordering::Acquire) {
        return true;
    }
    if STARTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        while STARTING.load(Ordering::Acquire) {
            lwp::yield_thread();
        }
        return INITIALIZED.load(Ordering::Acquire);
    }
    WORKER_STOP.store(false, Ordering::Release);
    let mut ok = true;
    unsafe {
        ok &= lwp::mutex_init(ptr::addr_of_mut!(QUEUE_MUTEX), 0) == lwp::LWP_SUCCESSFUL;
        ok &= lwp::cond_init(ptr::addr_of_mut!(QUEUE_COND)) == lwp::LWP_SUCCESSFUL;
        if ok {
            ok &= lwp::create_thread(
                ptr::addr_of_mut!(WORKER_THREAD),
                Some(worker_main),
                ptr::null_mut(),
                ptr::addr_of_mut!(WORKER_STACK.0).cast(),
                WORKER_STACK_SIZE as u32,
                WORKER_PRIORITY,
            ) == lwp::LWP_SUCCESSFUL;
        }
    }
    INITIALIZED.store(ok, Ordering::Release);
    STARTING.store(false, Ordering::Release);
    ok
}

const fn command_uses_auto_invalidation(command: u32) -> bool {
    matches!(command, 0x0001 | 0x0004 | 0x0005 | 0x000e)
}

unsafe fn auto_invalidate_for_command(block: *mut DvdCmdBlk) {
    if block.is_null() || AUTO_INVALIDATION.load(Ordering::Acquire) == 0 {
        return;
    }
    let command = unsafe { (*block).cmd };
    if command_uses_auto_invalidation(command) {
        let buffer = unsafe { (*block).buf };
        let len = unsafe { (*block).len };
        if !buffer.is_null() && len != 0 {
            unsafe { ogc_cache::dc_invalidate_range(buffer.cast_const(), len) };
        }
    }
}

fn submit(mut request: Request, callback: DvdCbCallback) -> i32 {
    if request.block.is_null() || !ensure_worker() {
        return 0;
    }
    request.callback = callback;
    unsafe {
        auto_invalidate_for_command(request.block);
        if !request.block.is_null() {
            (*request.block).state = DVD_STATE_BUSY;
            (*request.block).currtxsize = 0;
            (*request.block).txdsize = 0;
            (*request.block).cb = callback;
        }
        let mutex = QUEUE_MUTEX;
        let cond = QUEUE_COND;
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            if !request.block.is_null() {
                (*request.block).state = DVD_STATE_FATAL_ERROR;
            }
            return 0;
        }
        if QUEUE_LEN >= MAX_USER_REQUESTS {
            let _ = lwp::mutex_unlock(mutex);
            if !request.block.is_null() {
                (*request.block).state = DVD_STATE_RETRY;
            }
            return 0;
        }
        let mut insertion = QUEUE_LEN;
        while insertion > 0 {
            let previous = (QUEUE_HEAD + insertion - 1) % MAX_REQUESTS;
            if REQUESTS[previous].priority <= request.priority {
                break;
            }
            let destination = (QUEUE_HEAD + insertion) % MAX_REQUESTS;
            REQUESTS[destination] = REQUESTS[previous];
            insertion -= 1;
        }
        REQUESTS[(QUEUE_HEAD + insertion) % MAX_REQUESTS] = request;
        QUEUE_LEN += 1;
        DRIVE_STATUS.store(DVD_STATE_BUSY, Ordering::Release);
        let _ = lwp::cond_signal(cond);
        let _ = lwp::mutex_unlock(mutex);
    }
    1
}

fn wait_block(block: *mut DvdCmdBlk) -> i32 {
    if block.is_null() || !ensure_worker() {
        return -1;
    }
    unsafe {
        let mutex = QUEUE_MUTEX;
        let cond = QUEUE_COND;
        if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
            return -1;
        }
        loop {
            if (*block).state != DVD_STATE_BUSY {
                break;
            }
            if lwp::cond_wait(cond, mutex) != lwp::LWP_SUCCESSFUL {
                let _ = lwp::mutex_unlock(mutex);
                return -1;
            }
        }
        let result = if (*block).state == DVD_STATE_END {
            (*block).txdsize as i32
        } else {
            -1
        };
        let _ = lwp::mutex_unlock(mutex);
        result
    }
}

fn prepare_read_block(
    block: &mut DvdCmdBlk,
    command: u32,
    buf: *mut c_void,
    len: u32,
    offset: i64,
) {
    block.cmd = command;
    block.buf = buf;
    block.len = len;
    block.offset = offset;
}

pub fn init() {
    if ensure_worker() {
        let rc = if is_gamecube() {
            gc_di_initialize();
            0
        } else {
            di::init()
        };
        if rc < 0 {
            FATAL.store(true, Ordering::Release);
            DRIVE_STATUS.store(DVD_STATE_FATAL_ERROR, Ordering::Release);
        } else {
            DRIVE_STATUS.store(cover_state(), Ordering::Release);
        }
    }
}

pub fn pause() {
    PAUSED.store(true, Ordering::Release);
    // C sets the pausing state immediately only when no command is executing.
    // An active command is not canceled by DVD_Pause/DVD_CancelAllAsync.
    if unsafe { ACTIVE.is_null() } {
        PAUSING.store(true, Ordering::Release);
        DRIVE_STATUS.store(DVD_STATE_IGNORED, Ordering::Release);
    }
}

pub fn reset(reset_mode: u32) {
    init();
    let rc = if reset_mode == DVD_RESETNONE {
        if is_gamecube() {
            gc_di_initialize();
        }
        0
    } else if is_gamecube() {
        gc_di_reset(reset_mode);
        0
    } else {
        di::reset()
    };
    PAUSED.store(false, Ordering::Release);
    PAUSING.store(false, Ordering::Release);
    FATAL.store(rc != 0, Ordering::Release);
    DRIVE_STATUS.store(
        if rc == 0 {
            cover_state()
        } else {
            DVD_STATE_FATAL_ERROR
        },
        Ordering::Release,
    );
}

/// Queues an asynchronous drive-mount request.
///
/// # Safety
///
/// `block` must be null or point to a valid `DvdCmdBlk`. Once submitted, it
/// must not be accessed except through [`get_cmd_block_status`] until the
/// request callback runs or cancellation completes.
pub unsafe fn mount_async(block: *mut DvdCmdBlk, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x10;
        }
    }
    submit(
        Request {
            kind: RequestKind::Mount,
            block,
            ..Request::empty()
        },
        cb,
    )
}

pub fn mount() -> i32 {
    let block = ptr::addr_of_mut!(SYNC_BLOCK);
    if unsafe { mount_async(block, None) } == 0 {
        -1
    } else {
        wait_block(block).min(0)
    }
}

pub fn get_drive_status() -> i32 {
    if FATAL.load(Ordering::Acquire) {
        return DVD_STATE_FATAL_ERROR;
    }
    if PAUSING.load(Ordering::Acquire) {
        return DVD_STATE_IGNORED;
    }
    if DRIVE_STATUS.load(Ordering::Acquire) != DVD_STATE_BUSY {
        DRIVE_STATUS.store(cover_state(), Ordering::Release);
    }
    DRIVE_STATUS.load(Ordering::Acquire)
}

/// Queues an asynchronous drive-control request.
///
/// # Safety
///
/// `block` must be null or point to a valid `DvdCmdBlk`. Once submitted, it
/// must not be accessed except through [`get_cmd_block_status`] until the
/// request callback runs or cancellation completes.
pub unsafe fn control_drive_async(block: *mut DvdCmdBlk, cmd: u32, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x11;
            (*block).offset = cmd as i64;
        }
    }
    submit(
        Request {
            kind: RequestKind::Control,
            block,
            command: cmd,
            ..Request::empty()
        },
        cb,
    )
}

/// Executes a synchronous drive-control request.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk` for the
/// duration of the call.
pub unsafe fn control_drive(block: *mut DvdCmdBlk, cmd: u32) -> i32 {
    if unsafe { control_drive_async(block, cmd, None) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

/// Queues an asynchronous GCM-offset update.
///
/// # Safety
///
/// `block` must be null or point to a valid `DvdCmdBlk`. Once submitted, it
/// must not be accessed except through [`get_cmd_block_status`] until the
/// request callback runs or cancellation completes.
pub unsafe fn set_gcm_offset_async(block: *mut DvdCmdBlk, offset: i64, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x12;
            (*block).offset = offset;
        }
    }
    submit(
        Request {
            kind: RequestKind::SetOffset,
            block,
            offset,
            ..Request::empty()
        },
        cb,
    )
}

/// Applies a synchronous GCM-offset update.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk` for the
/// duration of the call.
pub unsafe fn set_gcm_offset(block: *mut DvdCmdBlk, offset: i64) -> i32 {
    if unsafe { set_gcm_offset_async(block, offset, None) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

/// Returns the current state stored in a command block.
///
/// # Safety
///
/// `block` must be null or point to a valid `DvdCmdBlk`. If the block belongs
/// to an asynchronous request, it must remain valid until that request completes.
pub unsafe fn get_cmd_block_status(block: *mut DvdCmdBlk) -> i32 {
    if block.is_null() {
        return -1;
    }
    if !INITIALIZED.load(Ordering::Acquire) {
        return unsafe { (*block).state };
    }

    let mutex = unsafe { QUEUE_MUTEX };
    if mutex == lwp::LWP_MUTEX_NULL || lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
        return -1;
    }
    let state = unsafe { (*block).state };
    if lwp::mutex_unlock(mutex) == lwp::LWP_SUCCESSFUL {
        state
    } else {
        -1
    }
}

/// Queues an asynchronous drive spin-up request.
///
/// # Safety
///
/// `block` must be null or point to a valid `DvdCmdBlk`. Once submitted, it
/// must not be accessed except through [`get_cmd_block_status`] until the
/// request callback runs or cancellation completes.
pub unsafe fn spin_up_drive_async(block: *mut DvdCmdBlk, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x10;
        }
    }
    submit(
        Request {
            kind: RequestKind::SpinUp,
            block,
            ..Request::empty()
        },
        cb,
    )
}

/// Spins up the drive synchronously.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk` for the
/// duration of the call.
pub unsafe fn spin_up_drive(block: *mut DvdCmdBlk) -> i32 {
    if unsafe { spin_up_drive_async(block, None) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

/// Queues an asynchronous drive-information request.
///
/// # Safety
///
/// `block` and `info` must be non-null and valid. Once submitted, neither may
/// be accessed, except for block polling through [`get_cmd_block_status`], until
/// the request callback runs or cancellation completes.
pub unsafe fn inquiry_async(
    block: *mut DvdCmdBlk,
    info: *mut DvdDrvInfo,
    cb: DvdCbCallback,
) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x0e;
            (*block).buf = info.cast();
            (*block).len = DVD_DRVINFSIZE as u32;
        }
    }
    submit(
        Request {
            kind: RequestKind::Inquiry,
            block,
            info,
            ..Request::empty()
        },
        cb,
    )
}

/// Reads drive information synchronously.
///
/// # Safety
///
/// `block` and `info` must point to distinct, valid, exclusively accessible
/// objects for the duration of the call.
pub unsafe fn inquiry(block: *mut DvdCmdBlk, info: *mut DvdDrvInfo) -> i32 {
    if unsafe { inquiry_async(block, info, None) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

/// Queues an asynchronous disc-ID read.
///
/// # Safety
///
/// `block` and `id` must be non-null and valid. Once submitted, neither may be
/// accessed, except for block polling through [`get_cmd_block_status`], until
/// the request callback runs or cancellation completes.
pub unsafe fn read_disk_id(block: *mut DvdCmdBlk, id: *mut DvdDiskId, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x0005;
            (*block).buf = id.cast();
            (*block).len = DVD_DISKIDSIZE as u32;
            (*block).id = id;
        }
    }
    submit(
        Request {
            kind: RequestKind::DiskId,
            block,
            id,
            ..Request::empty()
        },
        cb,
    )
}

/// Queues an asynchronous disc read.
///
/// # Safety
///
/// `block` must be non-null and valid. `buf` must point to `len` writable bytes,
/// be 32-byte aligned, and not overlap `block`. Once submitted, neither may be
/// accessed, except for block polling through [`get_cmd_block_status`], until
/// the callback runs or cancellation completes.
pub unsafe fn read_abs_async_prio(
    block: *mut DvdCmdBlk,
    buf: *mut c_void,
    len: u32,
    offset: i64,
    cb: DvdCbCallback,
    prio: i32,
) -> i32 {
    unsafe { read_async_cmd(block, buf, len, offset, cb, prio, ReadCommand::Absolute) }
}

unsafe fn read_async_cmd(
    block: *mut DvdCmdBlk,
    buf: *mut c_void,
    len: u32,
    offset: i64,
    cb: DvdCbCallback,
    prio: i32,
    command: ReadCommand,
) -> i32 {
    if !(0..=3).contains(&prio) {
        return 0;
    }
    unsafe {
        if let Some(block) = block.as_mut() {
            prepare_read_block(block, command.value(), buf, len, offset);
        }
    }
    submit(
        queued_read_request(block, buf, len, offset, prio as u8, command),
        cb,
    )
}

/// Queues an asynchronous disc read at default priority.
///
/// # Safety
///
/// The pointer validity, alignment, non-aliasing, and lifetime requirements are
/// identical to [`read_abs_async_prio`].
pub unsafe fn read_abs_async_for_bs(
    block: *mut DvdCmdBlk,
    buf: *mut c_void,
    len: u32,
    offset: i64,
    cb: DvdCbCallback,
) -> i32 {
    // BUG FIX: C uses command 0x0004 for DVD_ReadAbsAsyncForBS. The ordinary
    // absolute-read command (0x0001) has different drive state handling.
    unsafe { read_async_cmd(block, buf, len, offset, cb, 2, ReadCommand::ForBs) }
}

/// Queues an asynchronous absolute seek.
///
/// # Safety
///
/// `block` must be non-null and valid. Once submitted, it must not be accessed
/// except through [`get_cmd_block_status`] until the request callback runs or
/// cancellation completes.
pub unsafe fn seek_abs_async_prio(
    block: *mut DvdCmdBlk,
    offset: i64,
    cb: DvdCbCallback,
    prio: i32,
) -> i32 {
    if !(0..=3).contains(&prio) {
        return 0;
    }
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x0002;
            (*block).offset = offset;
        }
    }
    submit(
        Request {
            kind: RequestKind::Seek,
            priority: prio as u8,
            block,
            offset,
            ..Request::empty()
        },
        cb,
    )
}

/// Reads disc data synchronously.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk`. `buf`
/// must point to `len` writable bytes, be 32-byte aligned, and not overlap the
/// command block. Both objects must remain valid for the duration of the call.
pub unsafe fn read_prio(
    block: *mut DvdCmdBlk,
    buf: *mut c_void,
    len: u32,
    offset: i64,
    prio: i32,
) -> i32 {
    if unsafe { read_abs_async_prio(block, buf, len, offset, None, prio) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

/// Seeks synchronously to an absolute disc offset.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk` for the
/// duration of the call.
pub unsafe fn seek_prio(block: *mut DvdCmdBlk, offset: i64, prio: i32) -> i32 {
    if unsafe { seek_abs_async_prio(block, offset, None, prio) } == 0 {
        -1
    } else {
        wait_block(block).min(0)
    }
}

pub fn cancel_all_async(_cb: DvdCbCallback) -> i32 {
    // Current libogc only requests DVD_Pause here. It does not cancel queued
    // or active commands and it does not invoke the supplied callback.
    pause();
    1
}

/// Queues an asynchronous stream-stop request.
///
/// # Safety
///
/// `block` must be non-null and valid. Once submitted, it must not be accessed
/// except through [`get_cmd_block_status`] until the request callback runs or
/// cancellation completes.
pub unsafe fn stop_stream_at_end_async(block: *mut DvdCmdBlk, cb: DvdCbCallback) -> i32 {
    unsafe {
        if !block.is_null() {
            (*block).cmd = 0x08;
        }
    }
    submit(
        Request {
            kind: RequestKind::StopStream,
            block,
            ..Request::empty()
        },
        cb,
    )
}

/// Stops streaming synchronously.
///
/// # Safety
///
/// `block` must point to a valid, exclusively accessible `DvdCmdBlk` for the
/// duration of the call.
pub unsafe fn stop_stream_at_end(block: *mut DvdCmdBlk) -> i32 {
    if unsafe { stop_stream_at_end_async(block, None) } == 0 {
        -1
    } else {
        wait_block(block)
    }
}

pub fn set_auto_invalidation(auto_inv: u32) -> u32 {
    AUTO_INVALIDATION.swap(auto_inv, Ordering::AcqRel)
}

pub fn get_current_disk_id() -> *mut DvdDiskId {
    ptr::addr_of_mut!(CURRENT_DISK_ID)
}

pub fn get_drive_info() -> *mut DvdDrvInfo {
    ptr::addr_of_mut!(DRIVE_INFO)
}

/// Returns the last `/dev/di` drive error word recorded by a failed command.
pub fn get_last_error() -> u32 {
    LAST_ERROR.load(Ordering::Acquire)
}

/// Returns the current cover/media state without reporting synthetic success.
pub fn get_cover_status() -> i32 {
    cover_state()
}

pub fn disk_present() -> Bool {
    (cover_state() == DVD_STATE_END) as Bool
}

pub fn stop_motor() -> i32 {
    let rc = if is_gamecube() {
        gc_di_stop_motor()
    } else {
        di::stop_motor()
    };
    if rc == 0 {
        DRIVE_STATUS.store(DVD_STATE_MOTOR_STOPPED, Ordering::Release);
    }
    rc
}

/// Stops the worker-facing transport and closes `/dev/di` after pending work is cancelled.
pub fn shutdown() -> i32 {
    if cancel_all_async(None) == 0 {
        return -1;
    }
    let mut result = 0;
    if INITIALIZED.load(Ordering::Acquire) {
        unsafe {
            let mutex = QUEUE_MUTEX;
            let cond = QUEUE_COND;
            let thread = WORKER_THREAD;
            if lwp::mutex_lock(mutex) == lwp::LWP_SUCCESSFUL {
                while !ACTIVE.is_null() || QUEUE_LEN != 0 {
                    if lwp::cond_wait(cond, mutex) != lwp::LWP_SUCCESSFUL {
                        result = -1;
                        break;
                    }
                }
                WORKER_STOP.store(true, Ordering::Release);
                if lwp::cond_broadcast(cond) != lwp::LWP_SUCCESSFUL {
                    result = -1;
                }
                if lwp::mutex_unlock(mutex) != lwp::LWP_SUCCESSFUL {
                    result = -1;
                }
            } else {
                result = -1;
            }

            if thread != lwp::LWP_THREAD_NULL
                && lwp::join_thread(thread, ptr::null_mut()) != lwp::LWP_SUCCESSFUL
            {
                result = -1;
            }
            if cond != lwp::LWP_COND_NULL && lwp::cond_destroy(cond) != lwp::LWP_SUCCESSFUL {
                result = -1;
            }
            if mutex != lwp::LWP_MUTEX_NULL && lwp::mutex_destroy(mutex) != lwp::LWP_SUCCESSFUL {
                result = -1;
            }

            QUEUE_MUTEX = lwp::LWP_MUTEX_NULL;
            QUEUE_COND = lwp::LWP_COND_NULL;
            WORKER_THREAD = lwp::LWP_THREAD_NULL;
            REQUESTS = [Request::empty(); MAX_REQUESTS];
            QUEUE_HEAD = 0;
            QUEUE_LEN = 0;
            ACTIVE = ptr::null_mut();
        }
        INITIALIZED.store(false, Ordering::Release);
    }
    if is_gamecube() {
        if gc_di_stop_motor() < 0 {
            result = -1;
        }
    } else {
        if di::stop_motor() < 0 {
            result = -1;
        }
        if di::close() < 0 {
            result = -1;
        }
    }
    FATAL.store(false, Ordering::Release);
    PAUSED.store(false, Ordering::Release);
    LAST_ERROR.store(0, Ordering::Release);
    DRIVE_STATUS.store(DVD_STATE_NO_DISK, Ordering::Release);
    result
}

const _: () = {
    assert!(ReadCommand::ForBs.value() == 0x0004);
    assert!(ReadCommand::Absolute.value() == 0x0001);
};

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::{
        BsReadRecovery, DVD_STATE_COVER_CLOSED, DVD_STATE_COVER_OPEN, DVD_STATE_END,
        DVD_STATE_MOTOR_STOPPED, DVD_STATE_NO_DISK, DVD_STATE_RETRY, DvdCmdBlk,
        GC_DI_MAX_READ_SIZE, ReadCommand, command_uses_auto_invalidation, cover_wait_command,
        gc_drive_state_from_error, gc_read_command_words, queued_read_request, recover_bs_read,
        run_gc_read_chunks,
    };
    use core::ffi::c_void;

    #[test]
    fn failed_bs_read_stops_motor_and_waits_for_cover_close() {
        let mut stop_calls = 0;
        let cover_status = [true, false].into_iter();
        let cover_status = core::cell::RefCell::new(cover_status);
        let step = core::cell::Cell::new(0);
        let result = recover_bs_read(
            ReadCommand::ForBs.value(),
            -5,
            || {
                stop_calls += 1;
                step.set(1);
                0
            },
            || Ok(cover_status.borrow_mut().next().unwrap()),
            || {
                assert_eq!(step.get(), 1);
                assert_eq!(cover_status.borrow().len(), 0);
                step.set(2);
                0
            },
            || false,
            || {},
        );

        assert!(matches!(result, BsReadRecovery::CoverClosed));
        assert_eq!(stop_calls, 1);
        assert_eq!(step.get(), 2);
    }

    #[test]
    fn auto_invalidation_matches_c_command_set() {
        for command in [0x0001, 0x0004, 0x0005, 0x000e] {
            assert!(command_uses_auto_invalidation(command));
        }
        for command in [0x0002, 0x0003, 0x0008, 0x0010] {
            assert!(!command_uses_auto_invalidation(command));
        }
    }

    #[test]
    fn cover_close_ioctl_uses_the_ios_wait_for_cover_command() {
        assert_eq!(cover_wait_command()[0], 0x7900_0000);
    }

    #[test]
    fn gamecube_read_register_words_match_c_di_command() {
        assert_eq!(
            gc_read_command_words(0x1234_5678, 0x2000),
            [0xA800_0000, 0x1234_5678, 0x2000, 0x2000]
        );
    }

    #[test]
    fn gamecube_read_progress_uses_transferred_length_minus_remaining_register() {
        assert_eq!(gc_di_bytes_transferred(0x80000, 0x80000 - 0x1234), 0x1234);
        assert_eq!(gc_di_bytes_transferred(0x80000, 0), 0x80000);
        assert_eq!(gc_di_bytes_transferred(0x80000, 0x90000), 0);
    }

    #[test]
    fn gamecube_reads_use_bounded_chunks_and_advance_offsets() {
        let mut calls = [(0, 0); 3];
        let mut count = 0;
        let length = GC_DI_MAX_READ_SIZE * 2 + 0x1234;
        let (result, transferred) = run_gc_read_chunks(length, |offset, chunk_len| {
            calls[count] = (offset, chunk_len);
            count += 1;
            (0, chunk_len)
        });

        assert_eq!(result, 0);
        assert_eq!(transferred, length);
        assert_eq!(count, 3);
        assert_eq!(
            calls,
            [
                (0, GC_DI_MAX_READ_SIZE),
                (GC_DI_MAX_READ_SIZE, GC_DI_MAX_READ_SIZE),
                (GC_DI_MAX_READ_SIZE * 2, 0x1234),
            ]
        );
    }

    #[test]
    fn gamecube_read_failure_preserves_partial_progress_from_failed_chunk() {
        let mut calls = [(0, 0); 2];
        let mut count = 0;
        let (result, transferred) =
            run_gc_read_chunks(GC_DI_MAX_READ_SIZE * 2, |offset, chunk_len| {
                calls[count] = (offset, chunk_len);
                count += 1;
                if count == 2 {
                    (-5, 0x1234)
                } else {
                    (0, chunk_len)
                }
            });

        assert_eq!(result, -5);
        assert_eq!(transferred, GC_DI_MAX_READ_SIZE + 0x1234);
        assert_eq!(count, 2);
        assert_eq!(
            calls,
            [
                (0, GC_DI_MAX_READ_SIZE),
                (GC_DI_MAX_READ_SIZE, GC_DI_MAX_READ_SIZE),
            ]
        );
    }

    #[test]
    fn gamecube_drive_status_maps_c_status_codes() {
        assert_eq!(gc_drive_state_from_error(0x0000_0000), DVD_STATE_END);
        assert_eq!(gc_drive_state_from_error(0x0100_0000), DVD_STATE_COVER_OPEN);
        assert_eq!(
            gc_drive_state_from_error(0x0200_0000),
            DVD_STATE_COVER_CLOSED
        );
        assert_eq!(gc_drive_state_from_error(0x0300_0000), DVD_STATE_NO_DISK);
        assert_eq!(
            gc_drive_state_from_error(0x0400_0000),
            DVD_STATE_MOTOR_STOPPED
        );
        assert_eq!(gc_drive_state_from_error(0x0500_0000), DVD_STATE_RETRY);
        assert_eq!(gc_drive_state_from_error(0x0600_0000), DVD_STATE_RETRY);
    }

    #[test]
    fn canceled_bs_recovery_does_not_start_cover_wait() {
        let wait_calls = core::cell::Cell::new(0);
        let result = recover_bs_read(
            ReadCommand::ForBs.value(),
            -5,
            || 0,
            || Ok(true),
            || {
                wait_calls.set(wait_calls.get() + 1);
                0
            },
            || true,
            || {},
        );

        assert!(matches!(result, BsReadRecovery::Canceled));
        assert_eq!(wait_calls.get(), 0);
    }

    #[test]
    fn queued_bs_read_carries_the_c_command_value() {
        let request = queued_read_request(
            core::ptr::null_mut::<DvdCmdBlk>(),
            core::ptr::null_mut::<c_void>(),
            0x800,
            0,
            2,
            ReadCommand::ForBs,
        );

        assert!(request.kind == super::RequestKind::Read);
        assert_eq!(request.priority, 2);
        assert_eq!(request.command, 0x0004);
    }

    #[test]
    fn non_bs_read_does_not_enter_motor_recovery() {
        let result = recover_bs_read(
            ReadCommand::Absolute.value(),
            -5,
            || panic!("ordinary read must not stop the motor"),
            || panic!("ordinary read must not poll the cover"),
            || panic!("ordinary read must not wait for the cover"),
            || false,
            || panic!("ordinary read must not sleep"),
        );

        assert!(matches!(result, BsReadRecovery::NotRequired));
    }

    #[test]
    fn canceled_bs_recovery_does_not_wait_for_cover_close() {
        let mut cancel_checks = 0;
        let mut wait_calls = 0;
        let result = recover_bs_read(
            ReadCommand::ForBs.value(),
            -5,
            || 0,
            || Ok(false),
            || {
                wait_calls += 1;
                0
            },
            || {
                cancel_checks += 1;
                cancel_checks > 3
            },
            || {},
        );

        assert!(matches!(result, BsReadRecovery::Canceled));
        assert_eq!(wait_calls, 1);
    }
}
