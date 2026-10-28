#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use ogc_core::powerpc::{hollywood_revision, read_timebase};
use ogc_irq::{self as irq, IRQ_PI_SI};

use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};
#[cfg(feature = "diagnostics")]
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_video::get_current_tv_mode;

mod transfer_control;

pub const SI_CHAN0: u32 = 0;
pub const SI_CHAN1: u32 = 1;
pub const SI_CHAN2: u32 = 2;
pub const SI_CHAN3: u32 = 3;
pub const SI_MAX_CHAN: u32 = 4;

pub const SI_CHAN0_BIT: u32 = 0x8000_0000;
pub const SI_CHAN1_BIT: u32 = 0x4000_0000;
pub const SI_CHAN2_BIT: u32 = 0x2000_0000;
pub const SI_CHAN3_BIT: u32 = 0x1000_0000;
pub const SI_CHAN_BIT: fn(u32) -> u32 = |chn| SI_CHAN0_BIT >> chn;

pub const SI_ERROR_UNDER_RUN: u32 = 0x0001;
pub const SI_ERROR_OVER_RUN: u32 = 0x0002;
pub const SI_ERROR_COLLISION: u32 = 0x0004;
pub const SI_ERROR_NO_RESPONSE: u32 = 0x0008;
pub const SI_ERROR_WRST: u32 = 0x0010;
pub const SI_ERROR_RDST: u32 = 0x0020;
pub const SI_ERROR_UNKNOWN: u32 = 0x0040;
pub const SI_ERROR_BUSY: u32 = 0x0080;

const SI_TRANSFER_TIMER_KEY_BASE: usize = 0x5349_0000;

pub const SI_TYPE_MASK: u32 = 0x1800_0000;
pub const SI_TYPE_N64: u32 = 0x0000_0000;
pub const SI_TYPE_DOLPHIN: u32 = 0x0800_0000;
pub const SI_TYPE_GC: u32 = SI_TYPE_DOLPHIN;

pub const SI_GC_WIRELESS: u32 = 0x8000_0000;
pub const SI_GC_NOMOTOR: u32 = 0x2000_0000;
pub const SI_GC_STANDARD: u32 = 0x0100_0000;

pub const SI_WIRELESS_RECEIVED: u32 = 0x4000_0000;
pub const SI_WIRELESS_IR: u32 = 0x0400_0000;
pub const SI_WIRELESS_STATE: u32 = 0x0200_0000;
pub const SI_WIRELESS_ORIGIN: u32 = 0x0020_0000;
pub const SI_WIRELESS_FIX_ID: u32 = 0x0010_0000;
pub const SI_WIRELESS_TYPE: u32 = 0x000f_0000;
pub const SI_WIRELESS_LITE_MASK: u32 = 0x000c_0000;
pub const SI_WIRELESS_LITE: u32 = 0x0004_0000;
pub const SI_WIRELESS_CONT_MASK: u32 = 0x0008_0000;
pub const SI_WIRELESS_CONT: u32 = 0x0000_0000;
pub const SI_WIRELESS_ID: u32 = 0x00c0_ff00;
pub const SI_WIRELESS_TYPE_ID: u32 = SI_WIRELESS_TYPE | SI_WIRELESS_ID;

pub const SI_N64_CONTROLLER: u32 = SI_TYPE_N64 | 0x0500_0000;
pub const SI_N64_MIC: u32 = SI_TYPE_N64 | 0x0001_0000;
pub const SI_N64_KEYBOARD: u32 = SI_TYPE_N64 | 0x0002_0000;
pub const SI_N64_MOUSE: u32 = SI_TYPE_N64 | 0x0200_0000;
pub const SI_GBA: u32 = SI_TYPE_N64 | 0x0004_0000;
pub const SI_GC_CONTROLLER: u32 = SI_TYPE_GC | SI_GC_STANDARD;
pub const SI_GC_RECEIVER: u32 = SI_TYPE_GC | SI_GC_WIRELESS;
pub const SI_GC_WAVEBIRD: u32 =
    SI_TYPE_GC | SI_GC_WIRELESS | SI_GC_STANDARD | SI_WIRELESS_STATE | SI_WIRELESS_FIX_ID;
pub const SI_GC_KEYBOARD: u32 = SI_TYPE_GC | 0x0020_0000;
pub const SI_GC_STEERING: u32 = SI_TYPE_GC;

pub type SiCallback = Option<unsafe extern "C" fn(chan: i32, type_: u32)>;
pub type RdstHandler = Option<unsafe extern "C" fn(irq: u32, ctx: *mut c_void)>;

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) -> SpinGuard<'_> {
        while self
            .0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a> {
    lock: &'a SpinLock,
}

impl Drop for SpinGuard<'_> {
    fn drop(&mut self) {
        self.lock.0.store(false, Ordering::Release);
    }
}

static SI_LOCK: SpinLock = SpinLock::new();
static mut SI_BUSY_CHAN: i32 = -1;
static mut SI_COMMANDS: [u32; 4] = [0; 4];
static mut SI_RESPONSE: [[u32; 2]; 4] = [[0; 2]; 4];
static mut SI_RESPONSE_VALID: [bool; 4] = [false; 4];
static mut SI_TYPE: [u32; 4] = [SI_ERROR_NO_RESPONSE; 4];
static mut SI_TYPE_TIME: [u64; 4] = [0; 4];
static mut SI_TRANSFER_TIME: [u64; 4] = [0; 4];
static mut SI_TYPE_CALLBACKS: [[SiCallback; 4]; 4] = [[None; 4]; 4];
static mut SI_WIRELESS_ID_ACCESSORS: Option<(WirelessIdGetter, WirelessIdSetter)> = None;
static mut SI_WIRELESS_ID_COMMANDS: [u32; 4] = [0; 4];
static mut SI_POLL: u32 = 0;
static mut SI_RESPONSE_VCOUNT: [u32; 4] = [0; 4];
static mut SI_SAMPLING_RATE: u32 = 0;
static mut SI_RDST_HANDLERS: [RdstHandler; 4] = [None, None, None, None];
static mut SI_ENABLED_RDST: bool = false;
static mut SI_CHANNEL_BUSY: [bool; 4] = [false; 4];
static mut SI_ALIVE: bool = false;

#[derive(Copy, Clone)]
struct SiPacket {
    chan: i32,
    out: *const c_void,
    out_len: u32,
    input: *mut c_void,
    in_len: u32,
    callback: SiCallback,
    fire: u64,
}

impl SiPacket {
    const fn empty() -> Self {
        Self {
            chan: -1,
            out: core::ptr::null(),
            out_len: 0,
            input: core::ptr::null_mut(),
            in_len: 0,
            callback: None,
            fire: 0,
        }
    }
}

fn queued_packet_index(queue: &[SiPacket; 4], after_chan: i32, now: u64) -> Option<usize> {
    (1..=4).find_map(|offset| {
        let chan = ((after_chan + offset) & 3) as usize;
        let packet = queue[chan];
        (packet.chan == chan as i32 && now.wrapping_sub(packet.fire) as i64 >= 0).then_some(chan)
    })
}

fn queued_packet_for_alarm(queue: &[SiPacket; 4], chan: i32, now: u64) -> Option<usize> {
    let index = usize::try_from(chan).ok()?;
    let packet = queue.get(index)?;
    (packet.chan == chan && now.wrapping_sub(packet.fire) as i64 >= 0).then_some(index)
}

pub type WirelessIdGetter = unsafe extern "C" fn(chan: u32) -> u16;
pub type WirelessIdSetter = unsafe extern "C" fn(chan: u32, id: u16);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum WirelessIdAction {
    ClearAndDispatch,
    Dispatch,
    Transfer { command: u32, set_id: Option<u16> },
}

fn wireless_id_action(type_: u32, current_id: u16) -> WirelessIdAction {
    if type_ & 0x0f != 0
        || type_ & SI_TYPE_MASK != SI_TYPE_GC
        || type_ & SI_GC_WIRELESS == 0
        || type_ & SI_WIRELESS_IR != 0
    {
        return WirelessIdAction::ClearAndDispatch;
    }

    let mut id = (current_id as u32) << 8;
    if type_ & SI_WIRELESS_FIX_ID != 0 {
        if id & SI_WIRELESS_TYPE_ID == type_ & SI_WIRELESS_TYPE_ID {
            return WirelessIdAction::Dispatch;
        }
        let set_id = if id & SI_WIRELESS_FIX_ID == 0 {
            id = SI_WIRELESS_FIX_ID | (type_ & SI_WIRELESS_TYPE_ID);
            Some((id >> 8) as u16)
        } else {
            None
        };
        return WirelessIdAction::Transfer {
            command: 0x4e00_0000 | id,
            set_id,
        };
    }

    if type_ & SI_WIRELESS_RECEIVED != 0 {
        id = SI_WIRELESS_FIX_ID | (type_ & SI_WIRELESS_TYPE_ID);
        return WirelessIdAction::Transfer {
            command: 0x4e00_0000 | id,
            set_id: Some((id >> 8) as u16),
        };
    }

    WirelessIdAction::ClearAndDispatch
}

fn rdst_handlers_ready(poll: u32, response_lines: &[u32; 4], current_line: u32) -> bool {
    let configured_line = (poll >> 16) & 0x03ff;
    (0..4).all(|channel| {
        let enabled = poll & (0x80 >> channel) != 0;
        !enabled
            || (response_lines[channel] != 0
                && ((configured_line >> 1) + response_lines[channel]) >= current_line)
    })
}

fn status_error_mask(chan: usize) -> u32 {
    0x0f00_0000_u32 >> (chan * 8)
}

fn status_after_completion(status: u32, chan: usize) -> u32 {
    status & status_error_mask(chan)
}

fn clear_transfer_start(control: u32) -> u32 {
    (control | (1 << 31)) & !1
}

fn polling_interrupt_control(control: u32, enable: bool) -> (u32, u32) {
    let previous = u32::from(control & (1 << 27) != 0);
    let control = if enable {
        control | (1 << 27)
    } else {
        control & !(1 << 27)
    };
    // BUG FIX: C clears TCINT and TSTART when it updates polling interrupts.
    (control & 0x7fff_fffe, previous)
}

fn response_lines_after_polling_interrupt_change(
    lines: [u32; SI_MAX_CHAN as usize],
    enable: bool,
) -> [u32; SI_MAX_CHAN as usize] {
    if enable {
        [0; SI_MAX_CHAN as usize]
    } else {
        lines
    }
}

static mut SI_ACTIVE: SiPacket = SiPacket::empty();
static mut SI_QUEUED: [SiPacket; 4] = [SiPacket::empty(); 4];
static SI_TYPE_COMMAND: u32 = 0;

#[cfg(feature = "diagnostics")]
pub const SI_FAULT_OPERATION_TYPE_ASYNC: u32 = 1;
#[cfg(feature = "diagnostics")]
pub const SI_FAULT_OPERATION_TRANSFER: u32 = 2;
#[cfg(feature = "diagnostics")]
pub const SI_FAULT_COMPLETE: u32 = 1;
#[cfg(feature = "diagnostics")]
pub const SI_FAULT_REJECT: u32 = 2;
#[cfg(feature = "diagnostics")]
pub const SI_FAULT_HOLD: u32 = 3;

#[cfg(feature = "diagnostics")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SiFaultStep {
    pub operation: u32,
    pub channel: i32,
    /// The first command byte for transfers, or `0xff` as a wildcard.
    pub command: u8,
    pub _reserved: [u8; 3],
    /// One-based matching-operation ordinal, or zero for the next match.
    pub occurrence: u32,
    pub outcome: u32,
    /// Callback result for a transfer, or the complete SI type value.
    pub result: u32,
    pub payload_len: u32,
    pub payload: [u8; 10],
    pub _padding: [u8; 2],
}

#[cfg(feature = "diagnostics")]
impl SiFaultStep {
    pub const fn new(
        operation: u32,
        channel: i32,
        command: u8,
        occurrence: u32,
        outcome: u32,
        result: u32,
        payload_len: u32,
        payload: [u8; 10],
    ) -> Self {
        Self {
            operation,
            channel,
            command,
            _reserved: [0; 3],
            occurrence,
            outcome,
            result,
            payload_len,
            payload,
            _padding: [0; 2],
        }
    }
}

#[cfg(feature = "diagnostics")]
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SiFaultReport {
    pub installed: u32,
    pub consumed: u32,
    pub unmatched_operations: u32,
    pub dropped_steps: u32,
    pub held_operations: u32,
}

#[cfg(feature = "diagnostics")]
const SI_FAULT_CAPACITY: usize = 32;
#[cfg(feature = "diagnostics")]
const SI_FAULT_HELD_CAPACITY: usize = 4;

#[cfg(feature = "diagnostics")]
#[derive(Copy, Clone)]
struct SiFaultSlot {
    active: bool,
    consumed: bool,
    step: SiFaultStep,
}

#[cfg(feature = "diagnostics")]
impl SiFaultSlot {
    const fn empty() -> Self {
        Self {
            active: false,
            consumed: false,
            step: SiFaultStep::new(0, -1, 0xff, 0, 0, 0, 0, [0; 10]),
        }
    }
}

#[cfg(feature = "diagnostics")]
#[derive(Copy, Clone)]
struct SiHeldOperation {
    active: bool,
    operation: u32,
    channel: i32,
    in_: *mut c_void,
    in_len: u32,
    callback: SiCallback,
    step: SiFaultStep,
}

#[cfg(feature = "diagnostics")]
impl SiHeldOperation {
    const fn empty() -> Self {
        Self {
            active: false,
            operation: 0,
            channel: -1,
            in_: core::ptr::null_mut(),
            in_len: 0,
            callback: None,
            step: SiFaultStep::new(0, -1, 0xff, 0, 0, 0, 0, [0; 10]),
        }
    }
}

#[cfg(feature = "diagnostics")]
static SI_FAULT_LOCK: SpinLock = SpinLock::new();
#[cfg(feature = "diagnostics")]
static SI_FAULT_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "diagnostics")]
static mut SI_FAULT_SLOTS: [SiFaultSlot; SI_FAULT_CAPACITY] =
    [SiFaultSlot::empty(); SI_FAULT_CAPACITY];
#[cfg(feature = "diagnostics")]
static mut SI_FAULT_HELD: [SiHeldOperation; SI_FAULT_HELD_CAPACITY] =
    [SiHeldOperation::empty(); SI_FAULT_HELD_CAPACITY];
#[cfg(feature = "diagnostics")]
static mut SI_FAULT_OCCURRENCES: [[u32; 4]; 2] = [[0; 4]; 2];
#[cfg(feature = "diagnostics")]
static SI_FAULT_INSTALLED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "diagnostics")]
static SI_FAULT_CONSUMED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "diagnostics")]
static SI_FAULT_UNMATCHED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "diagnostics")]
static SI_FAULT_DROPPED: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "diagnostics")]
fn si_fault_take(operation: u32, channel: i32, command: u8) -> Option<SiFaultStep> {
    if !SI_FAULT_ENABLED.load(Ordering::Acquire) || !chan_valid(channel) {
        return None;
    }
    let _guard = SI_FAULT_LOCK.lock();
    let operation_index = match operation {
        SI_FAULT_OPERATION_TYPE_ASYNC => 0,
        SI_FAULT_OPERATION_TRANSFER => 1,
        _ => return None,
    };
    let occurrence = unsafe {
        let counter = &mut SI_FAULT_OCCURRENCES[operation_index][channel as usize];
        *counter = counter.wrapping_add(1);
        *counter
    };
    let slots = core::ptr::addr_of_mut!(SI_FAULT_SLOTS).cast::<SiFaultSlot>();
    for index in 0..SI_FAULT_CAPACITY {
        let slot = unsafe { &mut *slots.add(index) };
        if !slot.active
            || slot.consumed
            || slot.step.operation != operation
            || (slot.step.channel >= 0 && slot.step.channel != channel)
            || (slot.step.command != 0xff && slot.step.command != command)
            || (slot.step.occurrence != 0 && slot.step.occurrence != occurrence)
        {
            continue;
        }
        slot.consumed = true;
        SI_FAULT_CONSUMED.fetch_add(1, Ordering::Relaxed);
        return Some(slot.step);
    }
    SI_FAULT_UNMATCHED.fetch_add(1, Ordering::Relaxed);
    None
}

#[cfg(feature = "diagnostics")]
unsafe fn si_fault_copy_payload(step: &SiFaultStep, in_: *mut c_void, in_len: u32) {
    if in_.is_null() {
        return;
    }
    let count = (step.payload_len as usize)
        .min(step.payload.len())
        .min(in_len as usize);
    unsafe { core::ptr::copy_nonoverlapping(step.payload.as_ptr(), in_.cast::<u8>(), count) };
}

#[cfg(feature = "diagnostics")]
fn si_fault_hold(
    operation: u32,
    channel: i32,
    in_: *mut c_void,
    in_len: u32,
    callback: SiCallback,
    step: SiFaultStep,
) -> bool {
    let _guard = SI_FAULT_LOCK.lock();
    let held = core::ptr::addr_of_mut!(SI_FAULT_HELD).cast::<SiHeldOperation>();
    for index in 0..SI_FAULT_HELD_CAPACITY {
        let slot = unsafe { &mut *held.add(index) };
        if !slot.active {
            *slot = SiHeldOperation {
                active: true,
                operation,
                channel,
                in_,
                in_len,
                callback,
                step,
            };
            return true;
        }
    }
    SI_FAULT_DROPPED.fetch_add(1, Ordering::Relaxed);
    false
}

/// Installs a bounded fault script at the real SI operation boundary.
#[cfg(feature = "diagnostics")]
pub fn si_fault_install(steps: &[SiFaultStep]) -> usize {
    let _guard = SI_FAULT_LOCK.lock();
    unsafe {
        SI_FAULT_SLOTS = [SiFaultSlot::empty(); SI_FAULT_CAPACITY];
        SI_FAULT_HELD = [SiHeldOperation::empty(); SI_FAULT_HELD_CAPACITY];
        SI_FAULT_OCCURRENCES = [[0; 4]; 2];
    }
    SI_FAULT_INSTALLED.store(0, Ordering::Release);
    SI_FAULT_CONSUMED.store(0, Ordering::Release);
    SI_FAULT_UNMATCHED.store(0, Ordering::Release);
    SI_FAULT_DROPPED.store(0, Ordering::Release);

    let count = steps.len().min(SI_FAULT_CAPACITY);
    let slots = core::ptr::addr_of_mut!(SI_FAULT_SLOTS).cast::<SiFaultSlot>();
    for (index, step) in steps.iter().take(count).enumerate() {
        unsafe {
            *slots.add(index) = SiFaultSlot {
                active: true,
                consumed: false,
                step: *step,
            };
        }
    }
    if steps.len() > count {
        SI_FAULT_DROPPED.store((steps.len() - count) as u32, Ordering::Release);
    }
    SI_FAULT_INSTALLED.store(count as u32, Ordering::Release);
    SI_FAULT_ENABLED.store(count != 0, Ordering::Release);
    count
}

#[cfg(feature = "diagnostics")]
pub fn si_fault_clear() {
    let _guard = SI_FAULT_LOCK.lock();
    SI_FAULT_ENABLED.store(false, Ordering::Release);
    unsafe {
        SI_FAULT_SLOTS = [SiFaultSlot::empty(); SI_FAULT_CAPACITY];
        SI_FAULT_HELD = [SiHeldOperation::empty(); SI_FAULT_HELD_CAPACITY];
        SI_FAULT_OCCURRENCES = [[0; 4]; 2];
    }
}

/// Completes the oldest held operation for `channel` through its real callback.
#[cfg(feature = "diagnostics")]
pub fn si_fault_release(channel: i32) -> u32 {
    let operation = {
        let _guard = SI_FAULT_LOCK.lock();
        let held = core::ptr::addr_of_mut!(SI_FAULT_HELD).cast::<SiHeldOperation>();
        let mut selected = SiHeldOperation::empty();
        for index in 0..SI_FAULT_HELD_CAPACITY {
            let slot = unsafe { &mut *held.add(index) };
            if slot.active && (channel < 0 || slot.channel == channel) {
                selected = *slot;
                *slot = SiHeldOperation::empty();
                break;
            }
        }
        selected
    };
    if !operation.active {
        return 0;
    }
    unsafe { si_fault_copy_payload(&operation.step, operation.in_, operation.in_len) };
    if operation.operation == SI_FAULT_OPERATION_TYPE_ASYNC && chan_valid(operation.channel) {
        unsafe {
            SI_TYPE[operation.channel as usize] = operation.step.result;
            SI_TYPE_TIME[operation.channel as usize] = read_timebase();
        }
    }
    if let Some(callback) = operation.callback {
        unsafe { callback(operation.channel, operation.step.result) };
    }
    1
}

#[cfg(feature = "diagnostics")]
pub fn si_fault_report() -> SiFaultReport {
    let _guard = SI_FAULT_LOCK.lock();
    let held = core::ptr::addr_of!(SI_FAULT_HELD).cast::<SiHeldOperation>();
    let mut held_count = 0;
    for index in 0..SI_FAULT_HELD_CAPACITY {
        if unsafe { (*held.add(index)).active } {
            held_count += 1;
        }
    }
    SiFaultReport {
        installed: SI_FAULT_INSTALLED.load(Ordering::Acquire),
        consumed: SI_FAULT_CONSUMED.load(Ordering::Acquire),
        unmatched_operations: SI_FAULT_UNMATCHED.load(Ordering::Acquire),
        dropped_steps: SI_FAULT_DROPPED.load(Ordering::Acquire),
        held_operations: held_count,
    }
}

fn reg_base() -> usize {
    if hollywood_revision() == 0 {
        0xCC00_6400
    } else {
        0xCD00_6400
    }
}

#[inline]
fn reg(index: usize) -> *mut u32 {
    (reg_base() as *mut u32).wrapping_add(index)
}

#[inline]
unsafe fn read(index: usize) -> u32 {
    unsafe { read_volatile(reg(index)) }
}

#[inline]
unsafe fn write(index: usize, value: u32) {
    unsafe { write_volatile(reg(index), value) }
}

#[inline]
unsafe fn rdst_handler_ptr() -> *mut RdstHandler {
    core::ptr::addr_of_mut!(SI_RDST_HANDLERS).cast()
}

#[inline]
fn chan_valid(chan: i32) -> bool {
    (0..4).contains(&chan)
}

fn queue_type_callback(callbacks: &mut [SiCallback; 4], callback: SiCallback) {
    let Some(callback) = callback else {
        return;
    };
    if let Some(slot) = callbacks.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(callback);
    }
}

fn tv_xy() -> &'static [(u16, u8); 12] {
    static NTSC: [(u16, u8); 12] = [
        (0x00F6, 0x02),
        (0x000F, 0x12),
        (0x001E, 0x09),
        (0x002C, 0x06),
        (0x0034, 0x05),
        (0x0041, 0x04),
        (0x0057, 0x03),
        (0x0057, 0x03),
        (0x0057, 0x03),
        (0x0083, 0x02),
        (0x0083, 0x02),
        (0x0083, 0x02),
    ];
    static PAL: [(u16, u8); 12] = [
        (0x0128, 0x02),
        (0x000F, 0x15),
        (0x001D, 0x0B),
        (0x002D, 0x07),
        (0x0034, 0x06),
        (0x003F, 0x05),
        (0x004E, 0x04),
        (0x0068, 0x03),
        (0x0068, 0x03),
        (0x0068, 0x03),
        (0x0068, 0x03),
        (0x009C, 0x02),
    ];
    match get_current_tv_mode() {
        1 => &PAL,
        _ => &NTSC,
    }
}

#[inline]
fn sync_wait() {
    while unsafe { read(13) } & 1 != 0 {
        core::hint::spin_loop();
    }
}

unsafe fn complete_transfer(chan: i32, in_: *mut c_void, in_bytes: u32) -> u32 {
    let sisr = unsafe { read(14) };
    unsafe {
        write(13, clear_transfer_start(read(13)));
    }

    if !chan_valid(chan) {
        return sisr;
    }

    unsafe {
        if !in_.is_null() {
            let in_words = in_bytes / 4;
            let ptr = in_ as *mut u32;
            for i in 0..in_words as usize {
                *ptr.add(i) = read(32 + i);
            }
            if in_bytes & 0x03 != 0 {
                let val = read(32 + in_words as usize);
                let tail = in_.cast::<u8>();
                for i in 0..(in_bytes & 0x03) as usize {
                    *tail.add((in_words * 4) as usize + i) = ((val >> ((3 - i) * 8)) & 0xff) as u8;
                }
            }
        }
        SI_CHANNEL_BUSY[chan as usize] = false;
        SI_TRANSFER_TIME[chan as usize] = read_timebase();
        SI_BUSY_CHAN = -1;
    }

    let type_time = if (unsafe { read(13) } & (1 << 29)) == 0 {
        Some(read_timebase())
    } else {
        None
    };
    unsafe {
        let chan = chan as usize;
        SI_TYPE_TIME[chan] = type_time_after_transfer(SI_TYPE_TIME[chan], type_time);
    }

    if type_time.is_none() {
        let mut error = (sisr >> ((3 - chan as usize) * 8)) & 0x0f;
        if error & SI_ERROR_NO_RESPONSE != 0
            && (unsafe { SI_TYPE[chan as usize] } & SI_ERROR_BUSY) == 0
        {
            unsafe {
                SI_TYPE[chan as usize] = SI_ERROR_NO_RESPONSE;
            }
        }
        if error == 0 {
            error = SI_ERROR_COLLISION;
        }
        return error;
    }

    // BUG FIX: C refreshes typeTime after every successful SI transfer. Rust
    // only refreshed it after type probes, which caused unnecessary re-probes.
    0
}

unsafe fn start_packet(packet: SiPacket) {
    unsafe {
        let chan = packet.chan;
        write(14, status_after_completion(read(14), chan as usize));

        SI_BUSY_CHAN = chan;
        SI_ACTIVE = packet;
        SI_CHANNEL_BUSY[chan as usize] = true;
        let out_words = packet.out_len.wrapping_add(3) / 4;
        for i in 0..out_words as usize {
            let value = if packet.out.is_null() {
                0
            } else {
                *(packet.out.cast::<u32>()).add(i)
            };
            write(32 + i, value);
        }
        let control =
            transfer_control::transfer_control(read(13), chan, packet.out_len, packet.in_len)
                | (1 << 31)
                | if packet.callback.is_some() {
                    1 << 30
                } else {
                    0
                };
        write(13, control);
    }
}

unsafe fn start_next_packet(after_chan: i32) {
    if unsafe { SI_BUSY_CHAN != -1 } {
        return;
    }
    let now = read_timebase();
    let queue = unsafe { SI_QUEUED };
    if let Some(slot) = queued_packet_index(&queue, after_chan, now) {
        let packet = queue[slot];
        unsafe {
            SI_QUEUED[slot] = SiPacket::empty();
            ogc_lwp_sys::timer_cancel(SI_TRANSFER_TIMER_KEY_BASE | slot);
            start_packet(packet);
        }
    }
}

unsafe fn start_packet_for_alarm(chan: i32) {
    if !chan_valid(chan) || unsafe { SI_BUSY_CHAN != -1 } {
        return;
    }
    let now = read_timebase();
    let queue = unsafe { SI_QUEUED };
    let Some(slot) = queued_packet_for_alarm(&queue, chan, now) else {
        return;
    };
    let packet = queue[slot];
    unsafe {
        SI_QUEUED[slot] = SiPacket::empty();
        start_packet(packet);
    }
}

unsafe extern "C" fn si_transfer_alarm(key: u32, _arg: *mut c_void) {
    let chan = (key as usize & 0x3) as i32;
    // BUG FIX: C's alarm handler only starts the packet owned by that alarm.
    // Scanning other channels here lets a stale alarm launch unrelated work.
    unsafe { start_packet_for_alarm(chan) };
}

unsafe fn complete_active_transfer(invoke_callback: bool) -> u32 {
    let packet = unsafe { SI_ACTIVE };
    if !chan_valid(packet.chan) {
        let sisr = unsafe { read(14) };
        unsafe {
            // BUG FIX: C preserves TCINTMSK when it completes with no active
            // transfer; clearing it here could suppress later SI interrupts.
            write(13, clear_transfer_start(read(13)));
            SI_BUSY_CHAN = -1;
            if invoke_callback {
                start_next_packet(-1);
            }
        }
        return sisr;
    }
    let result = unsafe { complete_transfer(packet.chan, packet.input, packet.in_len) };
    unsafe {
        SI_ACTIVE = SiPacket::empty();
        if invoke_callback {
            start_next_packet(packet.chan);
        }
    }
    if invoke_callback {
        if let Some(callback) = packet.callback {
            unsafe { callback(packet.chan, result) };
        }
        // BUG FIX: C masks status to the completed channel after every IRQ
        // completion. Without this, idle channels can expose stale errors.
        unsafe {
            write(14, status_after_completion(read(14), packet.chan as usize));
        }
    }
    if should_retry_type_probe(
        invoke_callback,
        unsafe { SI_TYPE[packet.chan as usize] },
        is_chan_busy(packet.chan) != 0,
    ) {
        // BUG FIX: C retries a type query after an interrupt-driven transfer
        // when its cache remains busy. Without this retry, a later transfer
        // cannot recover a type query that did not enter the queue.
        let _ = queue_type_probe(packet.chan, 65);
    }
    result
}

fn should_retry_type_probe(interrupt_completion: bool, type_: u32, channel_busy: bool) -> bool {
    interrupt_completion && type_ == SI_ERROR_BUSY && !channel_busy
}

fn queue_type_probe(chan: i32, us_delay: u32) -> u32 {
    if !chan_valid(chan) {
        return 0;
    }
    unsafe {
        transfer(
            chan,
            core::ptr::addr_of!(SI_TYPE_COMMAND).cast(),
            1,
            core::ptr::addr_of_mut!(SI_TYPE[chan as usize]).cast(),
            3,
            Some(gettype_callback),
            us_delay,
        )
    }
}

unsafe extern "C" fn si_irq_handler(_irq: u32, _ctx: *mut c_void) {
    let csr = unsafe { read(13) };
    if csr & (1 << 31) != 0 && csr & (1 << 30) != 0 {
        unsafe {
            complete_active_transfer(true);
        }
    }
    if csr & (1 << 28) != 0 && csr & (1 << 27) != 0 {
        let current_line = ogc_video::get_current_line().wrapping_add(1);
        let poll = unsafe { SI_POLL };
        let mut response_lines = unsafe { SI_RESPONSE_VCOUNT };
        for channel in 0..4 {
            if get_response_raw(channel as i32) != 0 {
                response_lines[channel] = current_line;
                unsafe {
                    SI_RESPONSE_VCOUNT[channel] = current_line;
                }
            }
        }
        if !rdst_handlers_ready(poll, &response_lines, current_line) {
            return;
        }
        unsafe {
            SI_RESPONSE_VCOUNT = [0; 4];
        }
        let handlers = core::ptr::addr_of!(SI_RDST_HANDLERS).cast::<RdstHandler>();
        for index in 0..4 {
            let handler = unsafe { *handlers.add(index) };
            if let Some(handler) = handler {
                unsafe {
                    handler(_irq, _ctx);
                }
            }
        }
    }
}

unsafe fn transfer_now(
    chan: i32,
    out: *const c_void,
    out_len: u32,
    in_: *mut c_void,
    in_len: u32,
    cb: SiCallback,
    us_delay: u32,
) -> u32 {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    if !chan_valid(chan) || unsafe { SI_BUSY_CHAN == chan || SI_QUEUED[chan as usize].chan != -1 } {
        return 0;
    }
    let now = read_timebase();
    let fire = if us_delay == 0 {
        now
    } else {
        unsafe { SI_TRANSFER_TIME[chan as usize] }.wrapping_add(si_microsecs_to_ticks(us_delay))
    };
    let packet = SiPacket {
        chan,
        out,
        out_len,
        input: in_,
        in_len,
        callback: cb,
        fire,
    };
    unsafe {
        SI_QUEUED[chan as usize] = packet;
        start_next_packet(chan - 1);
        if SI_QUEUED[chan as usize].chan != -1 {
            let remaining = fire.wrapping_sub(read_timebase());
            if remaining != 0 && remaining < (1_u64 << 63) {
                let result = ogc_lwp_sys::timer_start_with_key(
                    SI_TRANSFER_TIMER_KEY_BASE | chan as usize,
                    Some(si_transfer_alarm),
                    core::ptr::null_mut(),
                    remaining,
                    0,
                );
                if result != ogc_lwp_sys::LWP_SUCCESSFUL {
                    SI_QUEUED[chan as usize] = SiPacket::empty();
                    return 0;
                }
            }
        }
    }
    1
}

fn timebase_ticks_per_second() -> u64 {
    if hollywood_revision() == 0 {
        40_500_000
    } else {
        60_750_000
    }
}

fn si_millisecs_to_ticks(ms: u64) -> u64 {
    ms.saturating_mul(timebase_ticks_per_second()) / 1_000
}

fn si_microsecs_to_ticks(us: u32) -> u64 {
    u64::from(us).saturating_mul(timebase_ticks_per_second()) / 1_000_000
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TypeCacheDecision {
    Return { value: u32, refresh_time: bool },
    Probe { return_value: u32 },
}

fn type_cache_decision(cached: u32, age: u64, polling: bool) -> TypeCacheDecision {
    if polling && cached != SI_ERROR_NO_RESPONSE {
        return TypeCacheDecision::Return {
            value: cached,
            refresh_time: true,
        };
    }
    if !polling && age <= si_millisecs_to_ticks(50) && cached != SI_ERROR_NO_RESPONSE {
        return TypeCacheDecision::Return {
            value: cached,
            refresh_time: false,
        };
    }
    let return_value = if !polling && age <= si_millisecs_to_ticks(75) {
        cached
    } else {
        SI_ERROR_BUSY
    };
    TypeCacheDecision::Probe { return_value }
}

fn type_time_after_transfer(previous: u64, successful_completion: Option<u64>) -> u64 {
    successful_completion.unwrap_or(previous)
}

unsafe extern "C" fn gettype_callback(chan: i32, type_: u32) {
    if !chan_valid(chan) {
        return;
    }

    unsafe {
        let idx = chan as usize;
        SI_TYPE[idx] = (SI_TYPE[idx] & !SI_ERROR_BUSY) | type_;
        SI_TYPE_TIME[idx] = read_timebase();

        let final_type = SI_TYPE[idx];
        // BUG FIX: C fixes wireless IDs before it calls queued type callbacks.
        // Without this handshake, receivers keep reporting an unfixed ID.
        if let Some((get_id, set_id)) = SI_WIRELESS_ID_ACCESSORS {
            match wireless_id_action(final_type, get_id(idx as u32)) {
                WirelessIdAction::ClearAndDispatch => set_id(idx as u32, 0),
                WirelessIdAction::Dispatch => {}
                WirelessIdAction::Transfer {
                    command,
                    set_id: new_id,
                } => {
                    if let Some(id) = new_id {
                        set_id(idx as u32, id);
                    }
                    SI_WIRELESS_ID_COMMANDS[idx] = command;
                    SI_TYPE[idx] = SI_ERROR_BUSY;
                    let command_ptr =
                        core::ptr::addr_of!(SI_WIRELESS_ID_COMMANDS[idx]).cast::<c_void>();
                    let response_ptr = core::ptr::addr_of_mut!(SI_TYPE[idx]).cast::<c_void>();
                    let _ = transfer(
                        chan,
                        command_ptr,
                        3,
                        response_ptr,
                        3,
                        Some(gettype_callback),
                        0,
                    );
                    return;
                }
            }
        }
        let callbacks = core::ptr::addr_of_mut!(SI_TYPE_CALLBACKS[idx]).cast::<SiCallback>();
        for slot in 0..4 {
            let entry = callbacks.add(slot);
            let cb = *entry;
            *entry = None;
            if let Some(cb) = cb {
                cb(chan, final_type);
            }
        }
    }
}

/// Registers SRAM backed wireless ID access for the SI type completion path.
///
/// The system crate calls this after SRAM initialization and before SI starts.
///
/// # Safety
///
/// Both functions must accept only channels 0 through 3 and must be safe to call
/// from an SI completion callback, including interrupt context.
pub unsafe fn register_wireless_id_accessors(get: WirelessIdGetter, set: WirelessIdSetter) {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        SI_WIRELESS_ID_ACCESSORS = Some((get, set));
    }
}

pub fn sync() -> u32 {
    sync_wait();
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    let result = unsafe { complete_active_transfer(false) };
    // BUG FIX: C calls __si_transfernext(4) after SI_Sync, even when no transfer
    // was active. Interrupt completions instead start after the completed channel.
    unsafe { start_next_packet(4) };
    result
}

pub fn busy() -> u32 {
    unsafe { (SI_BUSY_CHAN != -1) as u32 }
}

pub fn is_chan_busy(chan: i32) -> u32 {
    unsafe {
        (chan_valid(chan)
            && (SI_CHANNEL_BUSY[chan as usize] || SI_QUEUED[chan as usize].chan != -1))
            as u32
    }
}

pub fn set_xy(line: u16, cnt: u8) {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let poll = (SI_POLL & !0x03ff_ff00)
            | (((line as u32) & 0x03ff) << 16)
            | (((cnt as u32) & 0xff) << 8);
        SI_POLL = poll;
        write(12, SI_POLL);
    }
}

pub fn enable_polling(poll: u32) {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let mut shifted = poll >> 24;
        let mask = (shifted >> 4) & 0x0f;

        SI_POLL &= !mask;
        shifted &= 0x03ff_fff0 | mask;
        SI_POLL |= shifted & !0x03ff_ff00;

        transfer_commands();
        write(12, SI_POLL);
    }
}

pub fn disable_polling(poll: u32) {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let mask = (poll >> 24) & 0xf0;
        SI_POLL &= !mask;
        write(12, SI_POLL);
    }
}

pub fn set_sampling_rate(samplingrate: u32) {
    let sr = samplingrate.min(11);
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        SI_SAMPLING_RATE = sr;
    }
    let (line, cnt) = tv_xy()[sr as usize];
    let div = if unsafe { read(54) } & 1 != 0 { 2 } else { 1 };
    set_xy(line * div, cnt);
}

pub fn refresh_sampling_rate() {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    set_sampling_rate(unsafe { SI_SAMPLING_RATE });
}

pub fn get_status(chan: i32) -> u32 {
    if !chan_valid(chan) {
        return 0;
    }
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    let status = (unsafe { read(14) }) >> ((3 - chan) * 8);
    unsafe {
        if status & SI_ERROR_NO_RESPONSE != 0 && (SI_TYPE[chan as usize] & SI_ERROR_BUSY) == 0 {
            SI_TYPE[chan as usize] = SI_ERROR_NO_RESPONSE;
        }
    }
    status
}

pub fn get_response_raw(chan: i32) -> u32 {
    if !chan_valid(chan) {
        return 0;
    }
    unsafe {
        if (get_status(chan) & SI_ERROR_RDST) != 0 {
            SI_RESPONSE[chan as usize][0] = read(chan as usize * 3 + 1);
            SI_RESPONSE[chan as usize][1] = read(chan as usize * 3 + 2);
            SI_RESPONSE_VALID[chan as usize] = true;
            1
        } else {
            0
        }
    }
}

/// # Safety
///
/// `buf` must be null or valid, properly aligned, and writable for two consecutive `u32` values.
pub unsafe fn get_response(chan: i32, buf: *mut c_void) -> u32 {
    if !chan_valid(chan) || buf.is_null() {
        return 0;
    }
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let refreshed = get_response_raw(chan) != 0;
        let valid = refreshed || SI_RESPONSE_VALID[chan as usize];
        SI_RESPONSE_VALID[chan as usize] = false;
        if valid {
            let out = buf as *mut u32;
            *out = SI_RESPONSE[chan as usize][0];
            *out.add(1) = SI_RESPONSE[chan as usize][1];
        }
        valid as u32
    }
}

pub fn set_command(chan: i32, cmd: u32) {
    if !chan_valid(chan) {
        return;
    }
    unsafe {
        SI_COMMANDS[chan as usize] = cmd;
        write(chan as usize * 3, cmd);
    }
}

pub fn get_command(chan: i32) -> u32 {
    if !chan_valid(chan) {
        return 0;
    }
    unsafe { read(chan as usize * 3) }
}

/// Returns the last cached polling response without reading controller registers.
/// The words can be stale if the last PAD read failed, or zero before a response.
pub fn debug_last_response(chan: i32) -> Option<[u32; 2]> {
    if !chan_valid(chan) {
        return None;
    }
    Some(unsafe { SI_RESPONSE[chan as usize] })
}

/// # Safety
///
/// `out` and `in_` must be valid for the transfer lengths passed in, matching
/// the C API's raw-buffer contract.
pub unsafe fn transfer(
    chan: i32,
    out: *const c_void,
    out_len: u32,
    in_: *mut c_void,
    in_len: u32,
    cb: SiCallback,
    us_delay: u32,
) -> u32 {
    #[cfg(feature = "diagnostics")]
    {
        let command = if out.is_null() || out_len == 0 {
            0xff
        } else {
            unsafe { *out.cast::<u8>() }
        };
        if let Some(step) = si_fault_take(SI_FAULT_OPERATION_TRANSFER, chan, command) {
            match step.outcome {
                SI_FAULT_REJECT => return 0,
                SI_FAULT_COMPLETE => {
                    unsafe { si_fault_copy_payload(&step, in_, in_len) };
                    if let Some(callback) = cb {
                        unsafe { callback(chan, step.result) };
                    }
                    return 1;
                }
                SI_FAULT_HOLD => {
                    return si_fault_hold(SI_FAULT_OPERATION_TRANSFER, chan, in_, in_len, cb, step)
                        as u32;
                }
                _ => {}
            }
        }
    }
    unsafe { transfer_now(chan, out, out_len, in_, in_len, cb, us_delay) }
}

pub fn get_type(chan: i32) -> u32 {
    if !chan_valid(chan) {
        return SI_ERROR_UNKNOWN;
    }

    // C masks interrupts while it reads and updates the shared type cache and
    // queues the transfer. Keep this state transition atomic with SI IRQs.
    let _interrupt_guard = ogc_sys::InterruptGuard::new();

    unsafe {
        let now = read_timebase();
        let cached = SI_TYPE[chan as usize];
        let diff = now.wrapping_sub(SI_TYPE_TIME[chan as usize]);
        let polling = (SI_POLL & (0x80 >> chan as u32)) != 0;
        let return_value = match type_cache_decision(cached, diff, polling) {
            TypeCacheDecision::Return {
                value,
                refresh_time,
            } => {
                if refresh_time {
                    SI_TYPE_TIME[chan as usize] = now;
                }
                return value;
            }
            TypeCacheDecision::Probe { return_value } => return_value,
        };

        SI_TYPE[chan as usize] = SI_ERROR_BUSY;
        SI_TYPE_TIME[chan as usize] = now;

        // C keeps the cache in the BUSY state when this probe cannot be queued.
        // BUG FIX: The asynchronous SI request can read its output after this
        // function returns. Keep its command byte in static storage, as C does.
        if queue_type_probe(chan, 65) == 0 {
            SI_TYPE[chan as usize] = SI_ERROR_BUSY;
        }
        return_value
    }
}

pub fn get_type_async(chan: i32, cb: SiCallback) -> u32 {
    if !chan_valid(chan) {
        return SI_ERROR_UNKNOWN;
    }

    #[cfg(feature = "diagnostics")]
    if let Some(step) = si_fault_take(SI_FAULT_OPERATION_TYPE_ASYNC, chan, 0xff) {
        match step.outcome {
            SI_FAULT_REJECT => return SI_ERROR_UNKNOWN,
            SI_FAULT_COMPLETE => {
                unsafe {
                    SI_TYPE[chan as usize] = step.result;
                    SI_TYPE_TIME[chan as usize] = read_timebase();
                    if let Some(callback) = cb {
                        callback(chan, step.result);
                    }
                }
                return step.result;
            }
            SI_FAULT_HOLD => {
                if si_fault_hold(
                    SI_FAULT_OPERATION_TYPE_ASYNC,
                    chan,
                    core::ptr::null_mut(),
                    0,
                    cb,
                    step,
                ) {
                    return SI_ERROR_BUSY;
                }
                return SI_ERROR_UNKNOWN;
            }
            _ => {}
        }
    }

    // Match C's outer critical section: the type query, busy-state check, and
    // callback publication must remain one operation with respect to SI IRQs.
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    let type_ = get_type(chan);

    unsafe {
        let idx = chan as usize;
        if (SI_TYPE[idx] & SI_ERROR_BUSY) != 0 {
            let callbacks = &mut *core::ptr::addr_of_mut!(SI_TYPE_CALLBACKS[idx]);
            queue_type_callback(callbacks, cb);
            return type_;
        }

        if let Some(cb) = cb {
            cb(chan, type_);
        }
    }

    type_
}

pub fn decode_type(type_: u32) -> u32 {
    if type_ & SI_ERROR_NO_RESPONSE != 0 {
        return SI_ERROR_NO_RESPONSE;
    }
    if type_ & 0x0f != 0 {
        return SI_ERROR_UNKNOWN;
    }
    if type_ & 0xff != 0 {
        return SI_ERROR_BUSY;
    }
    let base = type_ & !0xffff;
    match base & SI_TYPE_MASK {
        SI_TYPE_N64 => match base {
            SI_N64_CONTROLLER | SI_N64_MIC | SI_N64_KEYBOARD | SI_N64_MOUSE | SI_GBA => base,
            _ => SI_ERROR_UNKNOWN,
        },
        SI_TYPE_GC => {
            if base == SI_GC_STEERING || base == SI_GC_KEYBOARD {
                base
            } else if (base & SI_GC_WIRELESS) != 0 && (base & SI_WIRELESS_IR) == 0 {
                if (base & SI_GC_WAVEBIRD) == SI_GC_WAVEBIRD {
                    SI_GC_WAVEBIRD
                } else if (base & SI_WIRELESS_STATE) == 0 {
                    SI_GC_RECEIVER
                } else if (base & SI_GC_CONTROLLER) == SI_GC_CONTROLLER {
                    SI_GC_CONTROLLER
                } else {
                    SI_ERROR_UNKNOWN
                }
            } else if (base & SI_GC_CONTROLLER) == SI_GC_CONTROLLER {
                SI_GC_CONTROLLER
            } else {
                SI_ERROR_UNKNOWN
            }
        }
        _ => SI_ERROR_UNKNOWN,
    }
}

pub fn probe(chan: i32) -> u32 {
    decode_type(get_type(chan))
}

pub fn transfer_commands() {
    unsafe {
        write(14, 0x8000_0000);
    }
}

pub fn register_polling_handler(handler: RdstHandler) -> u32 {
    let Some(handler) = handler else {
        return 1;
    };
    unsafe {
        let handlers = rdst_handler_ptr();
        for i in 0..4 {
            if let Some(existing) = *handlers.add(i)
                && core::ptr::fn_addr_eq(existing, handler)
            {
                return 1;
            }
        }
        for i in 0..4 {
            let slot = handlers.add(i);
            if (*slot).is_none() {
                *slot = Some(handler);
                SI_ENABLED_RDST = true;
                enable_polling_interrupt(1);
                return 1;
            }
        }
    }
    0
}

pub fn unregister_polling_handler(handler: RdstHandler) -> u32 {
    let Some(handler) = handler else {
        return 0;
    };
    unsafe {
        let handlers = rdst_handler_ptr();
        for i in 0..4 {
            let slot = handlers.add(i);
            if let Some(existing) = *slot {
                if !core::ptr::fn_addr_eq(existing, handler) {
                    continue;
                }
                *slot = None;
                let mut any = false;
                for next in 0..4 {
                    if (*handlers.add(next)).is_some() {
                        any = true;
                        break;
                    }
                }
                if !any {
                    SI_ENABLED_RDST = false;
                    enable_polling_interrupt(0);
                }
                return 1;
            }
        }
    }
    0
}

pub fn enable_polling_interrupt(enable: i32) -> u32 {
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let (csr, previous) = polling_interrupt_control(read(13), enable != 0);
        // BUG FIX: C clears these timestamps when enabling polling interrupts,
        // so old responses cannot satisfy the next retrace gate.
        SI_RESPONSE_VCOUNT =
            response_lines_after_polling_interrupt_change(SI_RESPONSE_VCOUNT, enable != 0);
        write(13, csr);
        previous
    }
}

pub fn init() {
    let _guard = SI_LOCK.lock();
    unsafe {
        if SI_ALIVE {
            return;
        }
        SI_ALIVE = true;
        SI_BUSY_CHAN = -1;
        SI_POLL = 0;
        SI_RESPONSE_VCOUNT = [0; 4];
        SI_SAMPLING_RATE = 0;
        SI_TYPE = [SI_ERROR_NO_RESPONSE; 4];
        SI_TYPE_TIME = [0; 4];
        SI_TRANSFER_TIME = [0; 4];
        SI_TYPE_CALLBACKS = [[None; 4]; 4];
        SI_RESPONSE_VALID = [false; 4];
        SI_CHANNEL_BUSY = [false; 4];
        SI_ACTIVE = SiPacket::empty();
        SI_QUEUED = [SiPacket::empty(); 4];
        set_sampling_rate(0);
        while read(13) & 1 != 0 {}
        write(13, 0x8000_0000);
        write(15, read(15) & !0x8000_0000);
        let _ = irq::request(IRQ_PI_SI, Some(si_irq_handler), core::ptr::null_mut());
        irq::unmask(1 << IRQ_PI_SI);
        for chan in 0..4 {
            let type_ = get_type(chan);
            if type_ != SI_ERROR_BUSY {
                SI_TYPE[chan as usize] = type_;
            }
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn queued_si_packets_follow_round_robin_and_wait_for_deadline() {
        let mut queue = [SiPacket::empty(); 4];
        queue[1] = SiPacket {
            chan: 1,
            fire: 20,
            ..SiPacket::empty()
        };
        queue[3] = SiPacket {
            chan: 3,
            fire: 10,
            ..SiPacket::empty()
        };
        assert_eq!(queued_packet_index(&queue, 0, 15), Some(3));
        assert_eq!(queued_packet_index(&queue, 0, 25), Some(1));
        assert_eq!(queued_packet_index(&queue, 2, 25), Some(3));
        assert_eq!(queued_packet_index(&queue, 4, 25), Some(1));
        assert_eq!(queued_packet_index(&queue, 0, 9), None);
    }

    #[test]
    fn si_alarm_only_selects_its_own_due_channel() {
        let mut queue = [SiPacket::empty(); 4];
        queue[1] = SiPacket {
            chan: 1,
            fire: 10,
            ..SiPacket::empty()
        };
        queue[3] = SiPacket {
            chan: 3,
            fire: 10,
            ..SiPacket::empty()
        };

        assert_eq!(queued_packet_for_alarm(&queue, 1, 10), Some(1));
        assert_eq!(queued_packet_for_alarm(&queue, 2, 10), None);
        assert_eq!(queued_packet_for_alarm(&queue, 3, 9), None);
    }

    #[test]
    fn wireless_id_action_matches_c_handshake_cases() {
        let wavebird_type = SI_GC_WAVEBIRD | 0x0042_3100;
        let wavebird_id = ((wavebird_type & SI_WIRELESS_TYPE_ID) >> 8) as u16;
        assert_eq!(
            wireless_id_action(wavebird_type, wavebird_id),
            WirelessIdAction::Dispatch
        );
        assert_eq!(
            wireless_id_action(wavebird_type, 0),
            WirelessIdAction::Transfer {
                command: 0x4e00_0000 | SI_WIRELESS_FIX_ID | (wavebird_type & SI_WIRELESS_TYPE_ID),
                set_id: Some(
                    ((SI_WIRELESS_FIX_ID | (wavebird_type & SI_WIRELESS_TYPE_ID)) >> 8) as u16,
                ),
            }
        );

        let receiver_type = SI_GC_RECEIVER | SI_WIRELESS_RECEIVED | 0x0023_4500;
        let fixed_id = (SI_WIRELESS_FIX_ID | (receiver_type & SI_WIRELESS_TYPE_ID)) >> 8;
        assert_eq!(
            wireless_id_action(receiver_type, 0),
            WirelessIdAction::Transfer {
                command: 0x4e00_0000 | SI_WIRELESS_FIX_ID | (receiver_type & SI_WIRELESS_TYPE_ID),
                set_id: Some(fixed_id as u16),
            }
        );

        assert_eq!(
            wireless_id_action(SI_ERROR_NO_RESPONSE, 0x1234),
            WirelessIdAction::ClearAndDispatch
        );
    }

    #[test]
    fn rdst_gating_waits_for_every_enabled_channel_and_expires_by_line() {
        let configured_poll = (20 << 16) | 0x80 | 0x20;
        let mut response_lines = [0; 4];

        assert!(!rdst_handlers_ready(configured_poll, &response_lines, 5));
        response_lines[0] = 3;
        assert!(!rdst_handlers_ready(configured_poll, &response_lines, 5));
        response_lines[2] = 3;
        assert!(rdst_handlers_ready(configured_poll, &response_lines, 13));
        assert!(!rdst_handlers_ready(configured_poll, &response_lines, 14));

        response_lines = [0; 4];
        assert!(rdst_handlers_ready(20 << 16, &response_lines, 500));
    }

    #[test]
    fn completed_transfer_keeps_only_its_channel_status_bits() {
        assert_eq!(status_after_completion(u32::MAX, 0), 0x0f00_0000);
        assert_eq!(status_after_completion(u32::MAX, 3), 0x0000_000f);
    }

    #[test]
    fn completion_clears_start_and_acknowledges_without_changing_interrupt_mask() {
        let before = (1 << 30) | (1 << 29) | 1;
        let after = clear_transfer_start(before);

        assert_eq!(after & (1 << 31), 1 << 31);
        assert_eq!(after & (1 << 30), 1 << 30);
        assert_eq!(after & (1 << 29), 1 << 29);
        assert_eq!(after & 1, 0);
    }

    #[test]
    fn polling_interrupt_updates_acknowledge_completion_and_keep_previous_state() {
        let before = (1 << 31) | (1 << 30) | (1 << 29) | (1 << 27) | 1;
        let (disabled, previous) = polling_interrupt_control(before, false);

        assert_eq!(previous, 1);
        assert_eq!(disabled & (1 << 31), 0);
        assert_eq!(disabled & (1 << 30), 1 << 30);
        assert_eq!(disabled & (1 << 29), 1 << 29);
        assert_eq!(disabled & (1 << 27), 0);
        assert_eq!(disabled & 1, 0);

        let (enabled, previous) = polling_interrupt_control(disabled, true);
        assert_eq!(previous, 0);
        assert_ne!(enabled & (1 << 27), 0);
        assert_eq!(enabled & (1 << 31), 0);

        assert_eq!(
            response_lines_after_polling_interrupt_change([1, 2, 3, 4], true),
            [0; 4]
        );
        assert_eq!(
            response_lines_after_polling_interrupt_change([1, 2, 3, 4], false),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn constants_exist() {
        assert_eq!(SI_MAX_CHAN, 4);
        assert_eq!(SI_GC_CONTROLLER, 0x0900_0000);
        assert_eq!(
            si_millisecs_to_ticks(50),
            timebase_ticks_per_second() * 50 / 1_000
        );
    }

    #[test]
    fn get_type_preserves_the_c_cache_grace_period() {
        assert_eq!(
            type_cache_decision(SI_GC_CONTROLLER, si_millisecs_to_ticks(50), false),
            TypeCacheDecision::Return {
                value: SI_GC_CONTROLLER,
                refresh_time: false,
            }
        );
        assert_eq!(
            type_cache_decision(SI_GC_CONTROLLER, si_millisecs_to_ticks(50) + 1, false),
            TypeCacheDecision::Probe {
                return_value: SI_GC_CONTROLLER,
            }
        );
        assert_eq!(
            type_cache_decision(SI_GC_CONTROLLER, si_millisecs_to_ticks(75), false),
            TypeCacheDecision::Probe {
                return_value: SI_GC_CONTROLLER,
            }
        );
        assert_eq!(
            type_cache_decision(SI_GC_CONTROLLER, si_millisecs_to_ticks(75) + 1, false),
            TypeCacheDecision::Probe {
                return_value: SI_ERROR_BUSY,
            }
        );
        assert_eq!(
            type_cache_decision(SI_GC_CONTROLLER, 0, true),
            TypeCacheDecision::Return {
                value: SI_GC_CONTROLLER,
                refresh_time: true,
            }
        );
        assert_eq!(
            type_cache_decision(SI_ERROR_NO_RESPONSE, 0, true),
            TypeCacheDecision::Probe {
                return_value: SI_ERROR_BUSY,
            }
        );
    }

    #[test]
    fn only_successful_transfers_refresh_the_type_cache_time() {
        assert_eq!(type_time_after_transfer(10, Some(20)), 20);
        assert_eq!(type_time_after_transfer(10, None), 10);
    }

    #[test]
    fn type_probe_retries_only_when_the_cache_is_busy_and_channel_is_idle() {
        assert!(should_retry_type_probe(true, SI_ERROR_BUSY, false));
        assert!(!should_retry_type_probe(false, SI_ERROR_BUSY, false));
        assert!(!should_retry_type_probe(true, SI_ERROR_BUSY, true));
        assert!(!should_retry_type_probe(true, SI_GC_CONTROLLER, false));
    }

    unsafe extern "C" fn type_callback_test_callback(_: i32, _: u32) {}

    #[test]
    fn get_type_async_keeps_duplicate_callback_registrations() {
        let mut callbacks = [None; 4];
        let callback = Some(type_callback_test_callback as unsafe extern "C" fn(i32, u32));

        queue_type_callback(&mut callbacks, callback);
        queue_type_callback(&mut callbacks, callback);

        assert_eq!(callbacks, [callback, callback, None, None]);
    }

    #[test]
    fn get_type_async_stops_at_the_c_callback_capacity() {
        let mut callbacks = [None; 4];
        let callback = Some(type_callback_test_callback as unsafe extern "C" fn(i32, u32));

        for _ in 0..5 {
            queue_type_callback(&mut callbacks, callback);
        }

        assert_eq!(callbacks, [callback; 4]);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn fault_script_is_bounded_and_reports_consumption() {
        let step = SiFaultStep::new(
            SI_FAULT_OPERATION_TRANSFER,
            0,
            0x42,
            0,
            SI_FAULT_REJECT,
            0,
            0,
            [0; 10],
        );
        assert_eq!(si_fault_install(&[step]), 1);
        assert!(si_fault_take(SI_FAULT_OPERATION_TRANSFER, 0, 0x41).is_none());
        assert_eq!(
            si_fault_take(SI_FAULT_OPERATION_TRANSFER, 0, 0x42),
            Some(step)
        );
        assert_eq!(
            si_fault_report(),
            SiFaultReport {
                installed: 1,
                consumed: 1,
                unmatched_operations: 1,
                dropped_steps: 0,
                held_operations: 0,
            }
        );
        si_fault_clear();
    }
}
