use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::{CARD_IO_BYTE_ADDRESSING, CARD_IO_SECTOR_ADDRESSING};
use ogc_lwp_sys as lwp;

use ogc_exi::{
    self as exi, EXI_DEVICE_0, EXI_FLAG_ATTACH, EXI_READWRITE, EXI_SPEED16MHZ, EXI_WRITE,
};

macro_rules! exi_imm_ex {
    ($($arg:expr_2021),+ $(,)?) => {
        unsafe { exi::imm_ex($($arg),+) }
    };
}

const MAX_DRIVE: usize = 3;
const MAX_INIT_RETRIES: u32 = 5;
const PAGE_SIZE512: u32 = 512;
const CARDIO_ERROR_READY: i32 = 0;
const CARDIO_ERROR_NOCARD: i32 = -3;
const CARDIO_ERROR_IOERROR: i32 = -5;
const CARDIO_ERROR_IOTIMEOUT: i32 = -6;
const CARDIO_ERROR_INTERNAL: i32 = -126;
const CARDIO_ERROR_FATALERROR: i32 = -128;
const CARDIO_OP_IOERR_IDLE: u32 = 0x2000;
const CARDIO_OP_IOERR_PARAM: u32 = 0x1000;
const CARDIO_OP_IOERR_WRITE: u32 = 0x0200;
const CARDIO_OP_IOERR_ADDR: u32 = 0x0100;
const CARDIO_OP_IOERR_CRC: u32 = 0x0002;
const CARDIO_OP_IOERR_ILL: u32 = 0x0001;
const CARDIO_OP_IOERR_FATAL: u32 = CARDIO_OP_IOERR_PARAM
    | CARDIO_OP_IOERR_WRITE
    | CARDIO_OP_IOERR_ADDR
    | CARDIO_OP_IOERR_CRC
    | CARDIO_OP_IOERR_ILL;
const NOT_INITIALIZED: u32 = 0;
const INITIALIZING: u32 = 1;
const INITIALIZED: u32 = 2;
const TYPE_SD: u32 = 0;
const TYPE_SDHC: u32 = 1;

static INIT: AtomicBool = AtomicBool::new(false);
static RETRY_COUNT: AtomicU32 = AtomicU32::new(0);
static TIMER_CLOCK_HZ: AtomicU32 = AtomicU32::new(0);
static STATE: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(NOT_INITIALIZED) },
    const { AtomicU32::new(NOT_INITIALIZED) },
    const { AtomicU32::new(NOT_INITIALIZED) },
];
static INSERTED: [AtomicBool; MAX_DRIVE] = [
    const { AtomicBool::new(false) },
    const { AtomicBool::new(false) },
    const { AtomicBool::new(false) },
];
static PAGE_SIZE: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
];
static CARD_FREQ: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
];
static ADDRESSING: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(CARD_IO_SECTOR_ADDRESSING) },
    const { AtomicU32::new(CARD_IO_SECTOR_ADDRESSING) },
    const { AtomicU32::new(CARD_IO_SECTOR_ADDRESSING) },
];
static INIT_TYPE: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(TYPE_SD) },
    const { AtomicU32::new(TYPE_SD) },
    const { AtomicU32::new(TYPE_SD) },
];
static WRITE_PROTECT_MODE: [AtomicBool; MAX_DRIVE] = [
    const { AtomicBool::new(false) },
    const { AtomicBool::new(false) },
    const { AtomicBool::new(false) },
];
static IO_ERROR: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
    const { AtomicU32::new(0) },
];
static EXI_WAIT_QUEUE: [AtomicU32; MAX_DRIVE] = [
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
    const { AtomicU32::new(lwp::LWP_TQUEUE_NULL) },
];

fn slot_valid(slot: usize) -> bool {
    slot < MAX_DRIVE
}

pub fn get_speed(slot: usize) -> Option<u32> {
    slot_valid(slot).then(|| CARD_FREQ[slot].load(Ordering::Acquire))
}

pub fn set_speed(slot: usize, frequency: u32) -> bool {
    if !slot_valid(slot) {
        return false;
    }
    CARD_FREQ[slot].store(frequency, Ordering::Release);
    true
}

pub fn get_page_size(slot: usize) -> Option<u32> {
    slot_valid(slot).then(|| PAGE_SIZE[slot].load(Ordering::Acquire))
}

fn update_page_size(
    slot: usize,
    size: u32,
    set_block_length: impl FnOnce(usize, u32) -> i32,
) -> Option<i32> {
    if !slot_valid(slot) {
        return None;
    }
    PAGE_SIZE[slot].store(size, Ordering::Release);
    Some(set_block_length(slot, size))
}

pub fn set_page_size(slot: usize, size: u32) -> Option<i32> {
    update_page_size(slot, size, set_blocklen)
}

pub fn get_addressing_type(slot: usize) -> Option<u32> {
    slot_valid(slot).then(|| ADDRESSING[slot].load(Ordering::Acquire))
}

#[cfg(test)]
fn prepare_transfer(
    slot: usize,
    num_sectors: u32,
    buffer_valid: bool,
    initialize: impl FnOnce(usize) -> bool,
) -> bool {
    if !slot_valid(slot) || !initialize(slot) {
        return false;
    }
    num_sectors != 0 && buffer_valid
}

fn ensure_transfer_ready(slot: usize) -> bool {
    STATE[slot].load(Ordering::Acquire) == INITIALIZED || init_io(slot) == CARDIO_ERROR_READY
}

fn retry_limit_reached(retry_count: u32) -> bool {
    retry_count > MAX_INIT_RETRIES
}

fn is_sd_gecko_id(id: u32) -> bool {
    id == u32::MAX
}

fn initialize_read_buffer(buffer: &mut [u8], idle_byte: u8) {
    buffer.fill(idle_byte);
}

fn poll_data_read_token(
    idle_byte: u8,
    mut transfer_byte: impl FnMut(u8) -> Result<u8, i32>,
    mut timeout: impl FnMut() -> bool,
) -> Result<i32, i32> {
    loop {
        let token = transfer_byte(idle_byte)?;
        if token == 0xfe {
            return Ok(CARDIO_ERROR_READY);
        }
        if timeout() {
            // C clocks one more byte after timeout, then drains payload and CRC.
            let final_token = transfer_byte(idle_byte)?;
            return Ok(if final_token == 0xfe {
                CARDIO_ERROR_READY
            } else {
                CARDIO_ERROR_IOTIMEOUT
            });
        }
    }
}

fn drain_after_data_token(
    token_result: Result<i32, i32>,
    drain: impl FnOnce() -> Result<(), i32>,
) -> Result<i32, i32> {
    let result = token_result?;
    drain()?;
    Ok(result)
}

fn data_read_idle_byte(write_protect: bool) -> u8 {
    if write_protect { 0x00 } else { 0xff }
}

fn multiwrite_stop_clock_bytes(write_protect: bool) -> [u8; 4] {
    [if write_protect { 0x00 } else { 0xff }; 4]
}

fn timer_clock_hz() -> u32 {
    let cached = TIMER_CLOCK_HZ.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }
    let detected = super::timeout::timer_clock_hz(ogc_system::get_hollywood_revision());
    TIMER_CLOCK_HZ.store(detected, Ordering::Release);
    detected
}

fn delay_microseconds(microseconds: u32) {
    if microseconds == 0 {
        return;
    }
    let ticks = (u64::from(timer_clock_hz()) * u64::from(microseconds) / 1_000_000).max(1);
    let start = ogc_system::time();
    while ogc_system::time().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

fn exi_wait_queue(slot: usize) -> Option<lwp::LwpQueue> {
    if !slot_valid(slot) {
        return None;
    }
    let current = EXI_WAIT_QUEUE[slot].load(Ordering::Acquire);
    if current != lwp::LWP_TQUEUE_NULL {
        return Some(current);
    }

    let mut candidate = lwp::LWP_TQUEUE_NULL;
    if unsafe { lwp::init_queue(&mut candidate) } != lwp::LWP_SUCCESSFUL {
        return None;
    }
    match EXI_WAIT_QUEUE[slot].compare_exchange(
        lwp::LWP_TQUEUE_NULL,
        candidate,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => Some(candidate),
        Err(existing) => {
            lwp::close_queue(candidate);
            Some(existing)
        }
    }
}

unsafe extern "C" fn exi_unlock_callback(chn: i32, _dev: i32) -> i32 {
    if chn >= 0 {
        let slot = chn as usize;
        if slot_valid(slot) {
            let queue = EXI_WAIT_QUEUE[slot].load(Ordering::Acquire);
            if queue != lwp::LWP_TQUEUE_NULL {
                lwp::thread_broadcast(queue);
            }
        }
    }
    1
}

fn wait_for_exi_lock(slot: usize) -> bool {
    let Some(queue) = exi_wait_queue(slot) else {
        return false;
    };
    loop {
        if unsafe { exi::lock(channel(slot), EXI_DEVICE_0, Some(exi_unlock_callback)) } != 0 {
            return true;
        }
        if lwp::thread_sleep(queue) != lwp::LWP_SUCCESSFUL {
            return false;
        }
    }
}

fn cmd0_wire_format(write_protect: bool) -> (u8, [u8; 5], u8) {
    let mut cmd = [0x40, 0, 0, 0, 0];
    let mut crc = crc7(&cmd);
    let idle_byte = if write_protect { 0x00 } else { 0xff };
    if write_protect {
        for byte in &mut cmd {
            *byte = !*byte;
        }
        crc = !crc;
    }
    (idle_byte, cmd, crc)
}

fn command_wire_format(cmd: &mut [u8; 5], write_protect: bool) -> (u8, u8) {
    cmd[0] |= 0x40;
    let mut crc = crc7(cmd);
    let idle_byte = if write_protect { 0x00 } else { 0xff };
    if write_protect {
        for byte in cmd {
            *byte = !*byte;
        }
        crc = !crc;
    }
    (idle_byte, crc)
}

unsafe extern "C" fn card_ext_handler(chn: i32, _dev: i32) -> i32 {
    if slot_valid(chn as usize) {
        INSERTED[chn as usize].store(false, Ordering::Release);
        STATE[chn as usize].store(NOT_INITIALIZED, Ordering::Release);
    }
    1
}

fn detect_card(slot: usize) -> bool {
    if slot == 2 {
        return true;
    }

    loop {
        let probe = exi::probe_ex(channel(slot));
        if probe == 0 {
            continue;
        }
        if probe != 1 {
            return false;
        }
        break;
    }

    !(exi::get_state(channel(slot)) & EXI_FLAG_ATTACH as i32 == 0
        && exi::attach(channel(slot), Some(card_ext_handler)) == 0)
}

fn channel(slot: usize) -> i32 {
    slot as i32
}

pub fn init_io_default() {
    RETRY_COUNT.store(0, Ordering::Release);
    for slot in 0..MAX_DRIVE {
        IO_ERROR[slot].store(0, Ordering::Release);
        INSERTED[slot].store(false, Ordering::Release);
        STATE[slot].store(NOT_INITIALIZED, Ordering::Release);
        ADDRESSING[slot].store(CARD_IO_BYTE_ADDRESSING, Ordering::Release);
        INIT_TYPE[slot].store(TYPE_SD, Ordering::Release);
        CARD_FREQ[slot].store(EXI_SPEED16MHZ as u32, Ordering::Release);
    }
}

fn ensure_initialized() {
    if !INIT.swap(true, Ordering::AcqRel) {
        crate::init_buffer_pool();
        init_io_default();
        exi::init();
    }
}

fn reset_slot(slot: usize) {
    STATE[slot].store(NOT_INITIALIZED, Ordering::Release);
    INSERTED[slot].store(false, Ordering::Release);
    // C preserves _ioPageSize across sdgecko_initIO attempts. It is updated
    // only after a successful CSD read or by sdgecko_setPageSize.
    CARD_FREQ[slot].store(EXI_SPEED16MHZ as u32, Ordering::Release);
    ADDRESSING[slot].store(CARD_IO_BYTE_ADDRESSING, Ordering::Release);
    INIT_TYPE[slot].store(TYPE_SD, Ordering::Release);
    IO_ERROR[slot].store(0, Ordering::Release);
}

fn timeout_elapsed(start: u64, timeout_ms: u64) -> bool {
    let elapsed = ogc_system::time().wrapping_sub(start);
    super::timeout::timeout_elapsed_ticks(elapsed, timeout_ms, u64::from(timer_clock_hz() / 1_000))
}

fn crc7(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        let mut current = byte;
        for _ in 0..8 {
            crc <<= 1;
            if ((crc ^ current) & 0x80) != 0 {
                crc ^= 0x09;
            }
            current <<= 1;
        }
    }
    (crc << 1) | 1
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn write_bl_len(csd: &[u8; 16]) -> u32 {
    (((csd[12] & 0x03) << 2) | ((csd[13] >> 6) & 0x03)) as u32
}

fn convert_sector(slot: usize, sector: u32) -> [u8; 4] {
    match ADDRESSING[slot].load(Ordering::Acquire) {
        CARD_IO_BYTE_ADDRESSING => [
            (sector >> 15) as u8,
            (sector >> 7) as u8,
            (sector << 1) as u8,
            (sector << 9) as u8,
        ],
        _ => [
            (sector >> 24) as u8,
            (sector >> 16) as u8,
            (sector >> 8) as u8,
            sector as u8,
        ],
    }
}

struct SelectGuard {
    slot: usize,
    selected: bool,
}

impl SelectGuard {
    fn new(slot: usize) -> Self {
        Self {
            slot,
            selected: false,
        }
    }

    fn arm(&mut self) {
        self.selected = true;
    }

    fn deselect(&mut self) {
        if self.selected {
            let _ = exi::deselect(channel(self.slot));
            self.selected = false;
        }
    }
}

impl Drop for SelectGuard {
    fn drop(&mut self) {
        if self.selected {
            self.deselect();
        }
        let _ = exi::unlock(channel(self.slot));
    }
}

fn select_slot(slot: usize, freq: i32) -> Option<SelectGuard> {
    if !slot_valid(slot) {
        return None;
    }
    // BUG FIX: C waits on a per-channel LWP queue until EXI becomes available.
    if !wait_for_exi_lock(slot) {
        return None;
    }
    // BUG FIX: C uses EXI_Select for normal data and command transfers.
    if exi::select(channel(slot), EXI_DEVICE_0, freq) == 0 {
        let _ = exi::unlock(channel(slot));
        return None;
    }
    let mut guard = SelectGuard::new(slot);
    guard.arm();
    Some(guard)
}

fn read_response(slot: usize, response: &mut [u8; 128], len: usize) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    let mut idx = 0usize;
    let mut byte = [0xffu8; 1];
    while idx < 16 {
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        response[0] = byte[0];
        if (response[0] & 0x80) == 0 {
            break;
        }
        idx += 1;
    }
    if idx >= 16 {
        return CARDIO_ERROR_IOTIMEOUT;
    }
    if len > 1
        && exi_imm_ex!(
            channel(slot),
            response[1..len].as_mut_ptr().cast::<c_void>(),
            (len - 1) as u32,
            EXI_READWRITE as i32,
        ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    CARDIO_ERROR_READY
}

fn response1(slot: usize, response: &mut [u8; 128]) -> i32 {
    read_response(slot, response, 1)
}

fn check_response(slot: usize, response: u8) -> i32 {
    IO_ERROR[slot].store(0, Ordering::Release);
    if STATE[slot].load(Ordering::Acquire) == INITIALIZING && (response & 0x01) != 0 {
        IO_ERROR[slot].fetch_or(CARDIO_OP_IOERR_IDLE, Ordering::AcqRel);
        return CARDIO_ERROR_READY;
    }
    if (response & 0x40) != 0 {
        IO_ERROR[slot].fetch_or(CARDIO_OP_IOERR_PARAM, Ordering::AcqRel);
    }
    if (response & 0x20) != 0 {
        IO_ERROR[slot].fetch_or(CARDIO_OP_IOERR_ADDR, Ordering::AcqRel);
    }
    if (response & 0x08) != 0 {
        IO_ERROR[slot].fetch_or(CARDIO_OP_IOERR_CRC, Ordering::AcqRel);
    }
    if (response & 0x04) != 0 {
        IO_ERROR[slot].fetch_or(CARDIO_OP_IOERR_ILL, Ordering::AcqRel);
    }
    if IO_ERROR[slot].load(Ordering::Acquire) & CARDIO_OP_IOERR_FATAL != 0 {
        CARDIO_ERROR_INTERNAL
    } else {
        CARDIO_ERROR_READY
    }
}

fn stop_response(slot: usize, response: &mut [u8; 128]) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    let mut byte = [0xffu8; 1];
    if exi_imm_ex!(
        channel(slot),
        byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_READWRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    byte[0] = 0xff;
    if exi_imm_ex!(
        channel(slot),
        byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_READWRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    response[0] = byte[0];

    let start = ogc_system::time();
    while (response[0] & 0x80) != 0 {
        byte[0] = 0xff;
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        response[0] = byte[0];
        if (response[0] & 0x80) == 0 {
            break;
        }
        if timeout_elapsed(start, 1500) {
            return CARDIO_ERROR_IOTIMEOUT;
        }
    }

    let status = response[0];
    while response[0] != 0xff {
        byte[0] = 0xff;
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        response[0] = byte[0];
        if response[0] == 0xff {
            break;
        }
        if timeout_elapsed(start, 1500) {
            return CARDIO_ERROR_IOTIMEOUT;
        }
    }
    response[0] = status;
    check_response(slot, status)
}

fn response2(slot: usize, response: &mut [u8; 128]) -> i32 {
    let ret = read_response(slot, response, 2);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    if response[0] & 0x7c == 0 && response[1] & 0x9e == 0 {
        CARDIO_ERROR_READY
    } else {
        CARDIO_ERROR_FATALERROR
    }
}

fn data_response(slot: usize, response: &mut [u8; 128]) -> i32 {
    let ret = data_status(slot, response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let code = (response[0] >> 1) & 0x07;
    if code == 0x05 {
        CARDIO_OP_IOERR_CRC as i32
    } else if code == 0x06 {
        CARDIO_OP_IOERR_WRITE as i32
    } else {
        CARDIO_ERROR_READY
    }
}

fn data_status(slot: usize, response: &mut [u8; 128]) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    let mut byte = [0xffu8; 1];
    if exi_imm_ex!(
        channel(slot),
        byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_READWRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    response[0] = byte[0];

    let start = ogc_system::time();
    while (response[0] & 0x10) != 0 {
        byte[0] = 0xff;
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        response[0] = byte[0];
        if (response[0] & 0x10) == 0 {
            break;
        }
        if timeout_elapsed(start, 1500) {
            return CARDIO_ERROR_IOTIMEOUT;
        }
    }

    byte[0] = 0xff;
    if exi_imm_ex!(
        channel(slot),
        byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_READWRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    response[1] = byte[0];

    let start = ogc_system::time();
    while response[1] == 0 {
        byte[0] = 0xff;
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        response[1] = byte[0];
        if response[1] != 0 {
            break;
        }
        if timeout_elapsed(start, 1500) {
            return CARDIO_ERROR_IOTIMEOUT;
        }
    }
    CARDIO_ERROR_READY
}

fn data_read(slot: usize, buffer: *mut u8, len: usize) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    if len == 0 {
        return CARDIO_ERROR_INTERNAL;
    }

    let idle_byte = data_read_idle_byte(WRITE_PROTECT_MODE[slot].load(Ordering::Acquire));
    // C fills the transfer buffer with _ioClrFlag before it polls for the data
    // token. EXI read/write transfers transmit these bytes as clock output.
    let read_buffer = unsafe { core::slice::from_raw_parts_mut(buffer, len) };
    initialize_read_buffer(read_buffer, idle_byte);

    let start = ogc_system::time();
    let token_poll = poll_data_read_token(
        idle_byte,
        |clock_byte| {
            let mut byte = [clock_byte; 1];
            if exi_imm_ex!(
                channel(slot),
                byte.as_mut_ptr().cast::<c_void>(),
                1,
                EXI_READWRITE as i32,
            ) == 0
            {
                Err(CARDIO_ERROR_IOERROR)
            } else {
                Ok(byte[0])
            }
        },
        || timeout_elapsed(start, 1500),
    );

    let mut response = [idle_byte; 2];
    let token_result = match drain_after_data_token(token_poll, || {
        if exi_imm_ex!(
            channel(slot),
            buffer.cast::<c_void>(),
            len as u32,
            EXI_READWRITE as i32,
        ) == 0
        {
            return Err(CARDIO_ERROR_IOERROR);
        }
        // C waits 1 microsecond between the block transfer and its CRC bytes.
        delay_microseconds(1);
        if exi_imm_ex!(
            channel(slot),
            response.as_mut_ptr().cast::<c_void>(),
            2,
            EXI_READWRITE as i32,
        ) == 0
        {
            return Err(CARDIO_ERROR_IOERROR);
        }
        Ok(())
    }) {
        Ok(result) => result,
        Err(error) => return error,
    };

    let crc_expected = u16::from_be_bytes(response);
    let crc_actual = unsafe { crc16(core::slice::from_raw_parts(buffer, len)) };
    if crc_expected != crc_actual {
        CARDIO_OP_IOERR_CRC as i32
    } else {
        token_result
    }
}

fn data_write(slot: usize, buffer: *const u8, len: usize) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    if len == 0 {
        return CARDIO_ERROR_INTERNAL;
    }

    let mut token = [0xfcu8; 1];
    if exi_imm_ex!(
        channel(slot),
        token.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    if exi_imm_ex!(
        channel(slot),
        buffer.cast::<c_void>() as *mut c_void,
        len as u32,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }

    let crc = unsafe { crc16(core::slice::from_raw_parts(buffer, len)) };
    let mut crc_bytes = crc.to_be_bytes();
    // C waits 1 microsecond between the block transfer and its CRC bytes.
    delay_microseconds(1);
    if exi_imm_ex!(
        channel(slot),
        crc_bytes.as_mut_ptr().cast::<c_void>(),
        2,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    CARDIO_ERROR_READY
}

fn multiwrite_stop(slot: usize) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    let write_protect = WRITE_PROTECT_MODE[slot].load(Ordering::Acquire);
    let idle_byte = if write_protect { 0x00 } else { 0xff };
    let mut token = [if write_protect { 0x02 } else { 0xfd }; 1];
    if exi_imm_ex!(
        channel(slot),
        token.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }

    // BUG FIX: C clocks four idle bytes between the stop token and busy poll.
    for mut byte in multiwrite_stop_clock_bytes(write_protect) {
        if exi_imm_ex!(
            channel(slot),
            (&mut byte as *mut u8).cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
    }

    let mut byte = [idle_byte; 1];
    let start = ogc_system::time();
    loop {
        if exi_imm_ex!(
            channel(slot),
            byte.as_mut_ptr().cast::<c_void>(),
            1,
            EXI_READWRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
        if byte[0] != 0 {
            break;
        }
        if timeout_elapsed(start, 1500) {
            return CARDIO_ERROR_IOTIMEOUT;
        }
    }

    CARDIO_ERROR_READY
}

fn send_cmd(slot: usize, cmd: &mut [u8; 5]) -> i32 {
    let _guard = match select_slot(slot, CARD_FREQ[slot].load(Ordering::Acquire) as i32) {
        Some(guard) => guard,
        None => return CARDIO_ERROR_NOCARD,
    };

    let write_protect = WRITE_PROTECT_MODE[slot].load(Ordering::Acquire);
    let (idle_byte, crc) = command_wire_format(cmd, write_protect);
    let mut dummy = [idle_byte; 10];
    if exi_imm_ex!(
        channel(slot),
        dummy.as_mut_ptr().cast::<c_void>(),
        dummy.len() as u32,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    if exi_imm_ex!(
        channel(slot),
        cmd.as_mut_ptr().cast::<c_void>(),
        cmd.len() as u32,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }

    let mut crc_byte = [crc];
    if exi_imm_ex!(
        channel(slot),
        crc_byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    CARDIO_ERROR_READY
}

fn send_cmd0(slot: usize) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    if !wait_for_exi_lock(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    if exi::select_sd(
        channel(slot),
        EXI_DEVICE_0,
        CARD_FREQ[slot].load(Ordering::Acquire) as i32,
    ) == 0
    {
        let _ = exi::unlock(channel(slot));
        return CARDIO_ERROR_NOCARD;
    }
    let mut guard = SelectGuard {
        slot,
        selected: true,
    };

    // BUG FIX: C clocks 20 blocks of 128 idle bytes under EXI_SelectSD before
    // it deselects and sends CMD0 under the normal EXI_Select mode.
    let write_protect = WRITE_PROTECT_MODE[slot].load(Ordering::Acquire);
    let (idle_byte, mut cmd, crc) = cmd0_wire_format(write_protect);
    let mut idle = [idle_byte; 128];
    for _ in 0..20 {
        if exi_imm_ex!(
            channel(slot),
            idle.as_mut_ptr().cast::<c_void>(),
            idle.len() as u32,
            EXI_WRITE as i32,
        ) == 0
        {
            return CARDIO_ERROR_IOERROR;
        }
    }
    guard.deselect();
    if exi::select(
        channel(slot),
        EXI_DEVICE_0,
        CARD_FREQ[slot].load(Ordering::Acquire) as i32,
    ) == 0
    {
        return CARDIO_ERROR_NOCARD;
    }
    guard.arm();

    if exi_imm_ex!(
        channel(slot),
        cmd.as_mut_ptr().cast::<c_void>(),
        cmd.len() as u32,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    let mut crc_byte = [crc];
    if exi_imm_ex!(
        channel(slot),
        crc_byte.as_mut_ptr().cast::<c_void>(),
        1,
        EXI_WRITE as i32,
    ) == 0
    {
        return CARDIO_ERROR_IOERROR;
    }
    CARDIO_ERROR_READY
}

fn send_appcmd(slot: usize) -> i32 {
    let mut cmd = [0x37, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    check_response(slot, response[0])
}

pub(super) fn send_opcond_with_attempts(
    mut attempt: impl FnMut(bool) -> Result<bool, i32>,
    mut timed_out: impl FnMut() -> bool,
) -> i32 {
    loop {
        let idle = match attempt(false) {
            Ok(idle) => idle,
            Err(rc) => return rc,
        };
        if !idle {
            return CARDIO_ERROR_READY;
        }
        if timed_out() {
            break;
        }
    }

    match attempt(true) {
        Ok(true) => CARDIO_ERROR_IOERROR,
        Ok(false) => CARDIO_ERROR_READY,
        Err(rc) => rc,
    }
}

fn send_opcond(slot: usize) -> i32 {
    let start = ogc_system::time();
    let mut response = [0u8; 128];
    send_opcond_with_attempts(
        |final_attempt| {
            let mut cmd = [0u8; 5];
            if INIT_TYPE[slot].load(Ordering::Acquire) == TYPE_SDHC {
                let ret = send_appcmd(slot);
                // C ignores the app-command result for its final operating-condition attempt.
                if !final_attempt && ret != CARDIO_ERROR_READY {
                    return Err(ret);
                }
                cmd[0] = 0x29;
                cmd[1] = 0x40;
            } else {
                cmd[0] = 0x01;
            }

            let ret = send_cmd(slot, &mut cmd);
            if ret != CARDIO_ERROR_READY {
                return Err(ret);
            }
            let ret = response1(slot, &mut response);
            if ret != CARDIO_ERROR_READY {
                return Err(ret);
            }
            let ret = check_response(slot, response[0]);
            if ret != CARDIO_ERROR_READY {
                return Err(ret);
            }
            Ok(IO_ERROR[slot].load(Ordering::Acquire) & CARDIO_OP_IOERR_IDLE != 0)
        },
        || timeout_elapsed(start, 1500),
    )
}

fn send_cmd8(slot: usize, response: &mut [u8; 128]) -> i32 {
    let mut cmd = [0x08, 0, 0, 0x01, 0xAA];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = read_response(slot, response, 5);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    check_response(slot, response[0])
}

fn send_cmd58(slot: usize, response: &mut [u8; 128]) -> i32 {
    let mut cmd = [0x3A, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = read_response(slot, response, 5);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    check_response(slot, response[0])
}

fn set_blocklen(slot: usize, block_len: u32) -> i32 {
    let mut len = block_len;
    if len > PAGE_SIZE512 {
        len = PAGE_SIZE512;
    }
    let mut cmd = [
        0x10,
        (len >> 24) as u8,
        (len >> 16) as u8,
        (len >> 8) as u8,
        len as u8,
    ];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    check_response(slot, response[0])
}

fn read_csd(slot: usize) -> i32 {
    let mut cmd = [0x09, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = check_response(slot, response[0]);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut csd = [0u8; 16];
    let ret = data_read(slot, csd.as_mut_ptr(), csd.len());
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    unsafe {
        core::ptr::addr_of_mut!(crate::g_CSD)
            .cast::<[u8; 16]>()
            .add(slot)
            .write(csd);
    }
    let bl_len = write_bl_len(&csd);
    PAGE_SIZE[slot].store(1u32 << bl_len, Ordering::Release);
    CARDIO_ERROR_READY
}

fn read_cid(slot: usize) -> i32 {
    let mut cmd = [0x0A, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = check_response(slot, response[0]);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut cid = [0u8; 16];
    let ret = data_read(slot, cid.as_mut_ptr(), cid.len());
    if ret == CARDIO_ERROR_READY {
        unsafe {
            core::ptr::addr_of_mut!(crate::g_CID)
                .cast::<[u8; 16]>()
                .add(slot)
                .write(cid);
        }
    }
    ret
}

fn sd_status(slot: usize) -> i32 {
    if PAGE_SIZE[slot].load(Ordering::Acquire) != 64 {
        PAGE_SIZE[slot].store(64, Ordering::Release);
        let ret = set_blocklen(slot, 64);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
    }
    let ret = send_appcmd(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut cmd = [0x0D, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response2(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut status = [0u8; 64];
    let ret = data_read(slot, status.as_mut_ptr(), status.len());
    if ret == CARDIO_ERROR_READY {
        unsafe {
            core::ptr::addr_of_mut!(crate::g_CardStatus)
                .cast::<[u8; 64]>()
                .add(slot)
                .write(status);
        }
    }
    ret
}

fn softreset(slot: usize) -> i32 {
    let ret = send_cmd0(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    check_response(slot, response[0])
}

fn unmount(slot: usize) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    let was_inserted = INSERTED[slot].swap(false, Ordering::AcqRel);
    STATE[slot].store(NOT_INITIALIZED, Ordering::Release);
    if was_inserted && slot != 2 {
        let _ = exi::detach(channel(slot));
    }
    CARDIO_ERROR_READY
}

fn initialize_detected_card(slot: usize) -> i32 {
    reset_slot(slot);
    WRITE_PROTECT_MODE[slot].store(false, Ordering::Release);
    INSERTED[slot].store(true, Ordering::Release);
    STATE[slot].store(INITIALIZING, Ordering::Release);

    let mut ret = softreset(slot);
    if ret != CARDIO_ERROR_READY {
        // BUG FIX: C retries CMD0 in write-protect mode after normal reset fails.
        WRITE_PROTECT_MODE[slot].store(true, Ordering::Release);
        ret = softreset(slot);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
    }

    let mut response = [0u8; 128];
    ret = send_cmd8(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    if response[3] == 1 && response[4] == 0xAA {
        INIT_TYPE[slot].store(TYPE_SDHC, Ordering::Release);
    }

    ret = send_opcond(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    ret = read_csd(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    ret = read_cid(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    if INIT_TYPE[slot].load(Ordering::Acquire) == TYPE_SDHC {
        ret = send_cmd58(slot, &mut response);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
        if response[1] & 0x40 != 0 {
            ADDRESSING[slot].store(CARD_IO_SECTOR_ADDRESSING, Ordering::Release);
        }
    }

    let page_size = PAGE_SIZE[slot].load(Ordering::Acquire);
    ret = set_blocklen(slot, page_size);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    ret = sd_status(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    STATE[slot].store(INITIALIZED, Ordering::Release);
    CARDIO_ERROR_READY
}

pub fn init_io(slot: usize) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    ensure_initialized();

    loop {
        let Ok(id) = exi::read_id(channel(slot), EXI_DEVICE_0) else {
            return CARDIO_ERROR_NOCARD;
        };
        if !is_sd_gecko_id(id) {
            return CARDIO_ERROR_NOCARD;
        }
        if retry_limit_reached(RETRY_COUNT.load(Ordering::Acquire)) {
            RETRY_COUNT.store(0, Ordering::Release);
            return CARDIO_ERROR_IOERROR;
        }
        if !detect_card(slot) {
            return CARDIO_ERROR_NOCARD;
        }

        let ret = initialize_detected_card(slot);
        if ret == CARDIO_ERROR_READY {
            RETRY_COUNT.store(0, Ordering::Release);
            return CARDIO_ERROR_READY;
        }

        let _ = unmount(slot);
        RETRY_COUNT.fetch_add(1, Ordering::AcqRel);
    }
}

pub fn pre_io(slot: usize) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    if STATE[slot].load(Ordering::Acquire) != INITIALIZED {
        return init_io(slot);
    }
    CARDIO_ERROR_READY
}

pub fn read_cid_api(slot: usize) -> i32 {
    let ret = pre_io(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    read_cid(slot)
}

pub fn read_csd_api(slot: usize) -> i32 {
    let ret = pre_io(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    read_csd(slot)
}

pub fn read_status_api(slot: usize) -> i32 {
    let ret = pre_io(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    sd_status(slot)
}

pub fn do_unmount(slot: usize) -> i32 {
    unmount(slot)
}

pub fn startup(slot: usize) -> bool {
    if !slot_valid(slot) {
        return false;
    }
    ensure_transfer_ready(slot)
}

pub fn is_inserted(slot: usize) -> bool {
    if !slot_valid(slot) {
        return false;
    }
    if !ensure_transfer_ready(slot) {
        return false;
    }
    sd_status(slot) == CARDIO_ERROR_READY
}

pub fn read_sectors_result(slot: usize, sector: u32, num_sectors: u32, buffer: *mut u8) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    let ret = pre_io(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    if num_sectors == 0 || buffer.is_null() {
        // C rejects a zero sector count. A null buffer would become undefined
        // behavior in C; Rust rejects it at the same boundary.
        return CARDIO_ERROR_INTERNAL;
    }
    if PAGE_SIZE[slot].load(Ordering::Acquire) != PAGE_SIZE512 {
        PAGE_SIZE[slot].store(PAGE_SIZE512, Ordering::Release);
        let ret = set_blocklen(slot, PAGE_SIZE512);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
    }

    let mut cmd = [0x12, 0, 0, 0, 0];
    let arg = convert_sector(slot, sector);
    cmd[1..].copy_from_slice(&arg);
    let ret = send_cmd(slot, &mut cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = check_response(slot, response[0]);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    for idx in 0..num_sectors as usize {
        let ptr = unsafe { buffer.add(idx * PAGE_SIZE512 as usize) };
        let ret = data_read(slot, ptr, PAGE_SIZE512 as usize);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
    }

    let mut stop = [0x0C, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut stop);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    stop_response(slot, &mut response)
}

pub fn read_sectors(slot: usize, sector: u32, num_sectors: u32, buffer: *mut u8) -> bool {
    read_sectors_result(slot, sector, num_sectors, buffer) == CARDIO_ERROR_READY
}

pub fn write_sectors_result(slot: usize, sector: u32, num_sectors: u32, buffer: *const u8) -> i32 {
    if !slot_valid(slot) {
        return CARDIO_ERROR_NOCARD;
    }
    let ret = pre_io(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    if num_sectors == 0 || buffer.is_null() {
        return CARDIO_ERROR_INTERNAL;
    }
    if PAGE_SIZE[slot].load(Ordering::Acquire) != PAGE_SIZE512 {
        PAGE_SIZE[slot].store(PAGE_SIZE512, Ordering::Release);
        let ret = set_blocklen(slot, PAGE_SIZE512);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
    }

    let mut appcmd = [0x17, 0, 0, 0, 0];
    let num = num_sectors.to_be_bytes();
    appcmd[1..].copy_from_slice(&num);
    let ret = send_appcmd(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = send_cmd(slot, &mut appcmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut response = [0u8; 128];
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = check_response(slot, response[0]);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    let mut write_cmd = [0x19, 0, 0, 0, 0];
    let arg = convert_sector(slot, sector);
    write_cmd[1..].copy_from_slice(&arg);
    let ret = send_cmd(slot, &mut write_cmd);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = response1(slot, &mut response);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let ret = check_response(slot, response[0]);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }

    for idx in 0..num_sectors as usize {
        let ptr = unsafe { buffer.add(idx * PAGE_SIZE512 as usize) };
        let ret = data_write(slot, ptr, PAGE_SIZE512 as usize);
        if ret != CARDIO_ERROR_READY {
            return ret;
        }
        let ret = data_response(slot, &mut response);
        if ret != CARDIO_ERROR_READY {
            let mut stop = [0x0C, 0, 0, 0, 0];
            let stop_ret = send_cmd(slot, &mut stop);
            if stop_ret != CARDIO_ERROR_READY {
                return stop_ret;
            }
            return stop_response(slot, &mut response);
        }
    }

    let ret = multiwrite_stop(slot);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    let mut stop = [0x0D, 0, 0, 0, 0];
    let ret = send_cmd(slot, &mut stop);
    if ret != CARDIO_ERROR_READY {
        return ret;
    }
    response2(slot, &mut response)
}

pub fn write_sectors(slot: usize, sector: u32, num_sectors: u32, buffer: *const u8) -> bool {
    write_sectors_result(slot, sector, num_sectors, buffer) == CARDIO_ERROR_READY
}

pub fn clear_status(slot: usize) -> bool {
    // C leaves this as FIXME; keep Rust's existing status reset behavior.
    if !slot_valid(slot) || STATE[slot].load(Ordering::Acquire) != INITIALIZED {
        return false;
    }

    IO_ERROR[slot].store(0, Ordering::Release);
    sd_status(slot) == CARDIO_ERROR_READY
}

pub fn shutdown(slot: usize) -> bool {
    unmount(slot) == CARDIO_ERROR_READY
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{
        CARD_FREQ, MAX_DRIVE, PAGE_SIZE, cmd0_wire_format, command_wire_format,
        data_read_idle_byte, drain_after_data_token, get_addressing_type, get_page_size, get_speed,
        initialize_read_buffer, is_sd_gecko_id, multiwrite_stop_clock_bytes, poll_data_read_token,
        prepare_transfer, retry_limit_reached, set_speed, update_page_size,
    };
    use core::cell::Cell;

    #[test]
    fn accepts_only_the_sd_gecko_device_id() {
        assert!(is_sd_gecko_id(u32::MAX));
        assert!(!is_sd_gecko_id(0));
        assert!(!is_sd_gecko_id(0xFFFF_FFFE));
    }

    #[test]
    fn card_io_accessors_use_per_slot_state_and_reject_bad_slots() {
        let slot = 2;
        let old_speed = get_speed(slot).unwrap();
        let old_page_size = get_page_size(slot).unwrap();

        assert!(set_speed(slot, 7));
        assert_eq!(get_speed(slot), Some(7));
        assert_eq!(
            get_addressing_type(slot),
            Some(super::CARD_IO_BYTE_ADDRESSING)
        );
        assert_eq!(get_speed(MAX_DRIVE), None);
        assert!(!set_speed(MAX_DRIVE, 9));
        assert_eq!(get_page_size(MAX_DRIVE), None);
        assert_eq!(get_addressing_type(MAX_DRIVE), None);

        CARD_FREQ[slot].store(old_speed, super::Ordering::Release);
        PAGE_SIZE[slot].store(old_page_size, super::Ordering::Release);
    }

    #[test]
    fn page_size_is_stored_before_block_length_command_and_kept_on_failure() {
        let slot = 2;
        let old_page_size = get_page_size(slot).unwrap();
        let result = update_page_size(slot, 256, |called_slot, size| {
            assert_eq!(called_slot, slot);
            assert_eq!(size, 256);
            assert_eq!(get_page_size(slot), Some(size));
            -5
        });
        assert_eq!(result, Some(-5));
        assert_eq!(get_page_size(slot), Some(256));
        PAGE_SIZE[slot].store(old_page_size, super::Ordering::Release);

        assert_eq!(update_page_size(MAX_DRIVE, 128, |_, _| panic!()), None);
    }

    #[test]
    fn zero_sector_request_initializes_before_rejection() {
        let initialized = Cell::new(false);
        assert!(!prepare_transfer(0, 0, true, |_| {
            initialized.set(true);
            true
        }));
        assert!(initialized.get());
    }

    #[test]
    fn initialization_allows_six_attempts() {
        assert!(!retry_limit_reached(5));
        assert!(retry_limit_reached(6));
    }

    #[test]
    fn read_transfer_bytes_start_at_the_idle_value() {
        let mut buffer = [0x42; 4];
        initialize_read_buffer(&mut buffer, 0xff);
        assert_eq!(buffer, [0xff; 4]);
    }

    #[test]
    fn data_read_timeout_clocks_one_final_token_byte_before_drain() {
        let mut clocks = std::vec::Vec::new();
        let mut tokens = [0x00, 0x00].into_iter();
        let result = poll_data_read_token(
            0xff,
            |clock_byte| {
                clocks.push(clock_byte);
                Ok(tokens.next().expect("two token responses"))
            },
            || true,
        );
        assert_eq!(result, Ok(super::CARDIO_ERROR_IOTIMEOUT));
        assert_eq!(clocks, [0xff, 0xff]);
        let drained = Cell::new(false);
        assert_eq!(
            drain_after_data_token(result, || {
                drained.set(true);
                Ok(())
            }),
            Ok(super::CARDIO_ERROR_IOTIMEOUT)
        );
        assert!(drained.get());

        let mut tokens = [0x00, 0xfe].into_iter();
        let result = poll_data_read_token(0x00, |_| Ok(tokens.next().unwrap()), || true);
        assert_eq!(result, Ok(super::CARDIO_ERROR_READY));
    }

    #[test]
    fn data_read_selects_mode_specific_clock_byte_and_initializes_buffer() {
        let mut normal_buffer = [0x42; 4];
        let normal_idle = data_read_idle_byte(false);
        initialize_read_buffer(&mut normal_buffer, normal_idle);
        assert_eq!(normal_idle, 0xff);
        assert_eq!(normal_buffer, [0xff; 4]);

        let mut protected_buffer = [0x42; 4];
        let protected_idle = data_read_idle_byte(true);
        initialize_read_buffer(&mut protected_buffer, protected_idle);
        assert_eq!(protected_idle, 0x00);
        assert_eq!(protected_buffer, [0x00; 4]);
    }

    #[test]
    fn multiwrite_stop_sends_four_mode_specific_clock_bytes() {
        assert_eq!(multiwrite_stop_clock_bytes(false), [0xff; 4]);
        assert_eq!(multiwrite_stop_clock_bytes(true), [0x00; 4]);
    }

    #[test]
    fn generic_command_wire_format_matches_write_protect_encoding() {
        let mut normal = [0x11, 1, 2, 3, 4];
        let mut protected = normal;
        let (normal_idle, normal_crc) = command_wire_format(&mut normal, false);
        let (protected_idle, protected_crc) = command_wire_format(&mut protected, true);
        assert_eq!(normal_idle, 0xff);
        assert_eq!(protected_idle, 0x00);
        assert_eq!(protected, normal.map(|byte| !byte));
        assert_eq!(protected_crc, !normal_crc);
    }

    #[test]
    fn cmd0_wire_format_matches_normal_and_write_protect_modes() {
        assert_eq!(cmd0_wire_format(false), (0xff, [0x40, 0, 0, 0, 0], 0x95));
        assert_eq!(
            cmd0_wire_format(true),
            (0x00, [0xbf, 0xff, 0xff, 0xff, 0xff], !0x95)
        );
    }
}
