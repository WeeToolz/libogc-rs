#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ogc_si as si;
use ogc_system::{LwpNode, SysResetInfo, register_reset_func, set_wireless_id};

pub use ogc_sys::Bool;

pub const PAD_CHAN0: u32 = 0;
pub const PAD_CHAN1: u32 = 1;
pub const PAD_CHAN2: u32 = 2;
pub const PAD_CHAN3: u32 = 3;
pub const PAD_CHANMAX: u32 = 4;

pub const PAD_MOTOR_STOP: u32 = 0;
pub const PAD_MOTOR_RUMBLE: u32 = 1;
pub const PAD_MOTOR_STOP_HARD: u32 = 2;

pub const PAD_ERR_NONE: i32 = 0;
pub const PAD_ERR_NO_CONTROLLER: i32 = -1;
pub const PAD_ERR_NOT_READY: i32 = -2;
pub const PAD_ERR_TRANSFER: i32 = -3;

pub const PAD_BUTTON_LEFT: u16 = 0x0001;
pub const PAD_BUTTON_RIGHT: u16 = 0x0002;
pub const PAD_BUTTON_DOWN: u16 = 0x0004;
pub const PAD_BUTTON_UP: u16 = 0x0008;
pub const PAD_TRIGGER_Z: u16 = 0x0010;
pub const PAD_TRIGGER_R: u16 = 0x0020;
pub const PAD_TRIGGER_L: u16 = 0x0040;
pub const PAD_BUTTON_A: u16 = 0x0100;
pub const PAD_BUTTON_B: u16 = 0x0200;
pub const PAD_BUTTON_X: u16 = 0x0400;
pub const PAD_BUTTON_Y: u16 = 0x0800;
pub const PAD_BUTTON_MENU: u16 = 0x1000;
pub const PAD_BUTTON_START: u16 = 0x1000;

pub const PAD_CHAN0_BIT: u32 = 0x8000_0000;
pub const PAD_CHAN1_BIT: u32 = 0x4000_0000;
pub const PAD_CHAN2_BIT: u32 = 0x2000_0000;
pub const PAD_CHAN3_BIT: u32 = 0x1000_0000;
pub const PAD_VALID_MASK: u32 = PAD_CHAN0_BIT | PAD_CHAN1_BIT | PAD_CHAN2_BIT | PAD_CHAN3_BIT;

pub type SamplingCallback = Option<unsafe extern "C" fn()>;
pub type DebugHook = Option<unsafe fn(stage: u32)>;
pub type PadTraceHook = Option<unsafe fn(event: PadDebugEvent)>;

/// A bounded diagnostic event emitted by the production PAD state machine.
///
/// The meaning of `a` through `d` is selected by `kind`. The event stays a
/// fixed-size value so interrupt callbacks can record it without allocating.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PadDebugEvent {
    pub sequence: u32,
    pub kind: u32,
    pub channel: i32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

pub const PAD_TRACE_REQUEST_RESET: u32 = 1;
pub const PAD_TRACE_REQUEST_RECALIBRATE: u32 = 2;
pub const PAD_TRACE_PREPARE_MASKS: u32 = 3;
pub const PAD_TRACE_PREPARE_STATE: u32 = 4;
pub const PAD_TRACE_DISABLE_POLLING: u32 = 5;
pub const PAD_TRACE_SELECT_CHANNEL: u32 = 6;
pub const PAD_TRACE_TYPE_RESULT: u32 = 7;
pub const PAD_TRACE_TRANSFER: u32 = 8;
pub const PAD_TRACE_TRANSFER_RESULT: u32 = 9;
pub const PAD_TRACE_CALLBACK: u32 = 10;
pub const PAD_TRACE_ORIGIN_RAW_0: u32 = 11;
pub const PAD_TRACE_ORIGIN_RAW_1: u32 = 12;
pub const PAD_TRACE_ORIGIN_TRANSFORMED: u32 = 13;
pub const PAD_TRACE_ENABLE_CHANNEL: u32 = 14;
pub const PAD_TRACE_DISABLE_CHANNEL: u32 = 15;
pub const PAD_TRACE_FINAL_STATE: u32 = 16;
pub const PAD_TRACE_SYNC: u32 = 17;
pub const PAD_TRACE_MOTOR: u32 = 18;
pub const PAD_TRACE_RESET_CALLBACK: u32 = 19;
pub const PAD_TRACE_STATE_RECAL_ENABLE: u32 = 20;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PadDebugCounters {
    pub produced: u32,
    pub drained: u32,
    pub dropped: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PadDebugState {
    pub spec: u32,
    pub resetting_bits: u32,
    pub waiting_bits: u32,
    pub pending_bits: u32,
    pub checking_bits: u32,
    pub recalibrate_bits: u32,
    pub resetting_chan: u32,
    pub enable_bits: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct PadStatus {
    pub button: u16,
    pub stick_x: i8,
    pub stick_y: i8,
    pub substick_x: i8,
    pub substick_y: i8,
    pub trigger_l: u8,
    pub trigger_r: u8,
    pub analog_a: u8,
    pub analog_b: u8,
    pub err: i8,
}

#[derive(Copy, Clone)]
struct KeyInput {
    stick_x: i8,
    stick_y: i8,
    substick_x: i8,
    substick_y: i8,
    trigger_l: u8,
    trigger_r: u8,
    up: u16,
    down: u16,
    state: u16,
    chan: i32,
}

impl KeyInput {
    const fn new() -> Self {
        Self {
            stick_x: 0,
            stick_y: 0,
            substick_x: 0,
            substick_y: 0,
            trigger_l: 0,
            trigger_r: 0,
            up: 0,
            down: 0,
            state: 0,
            chan: -1,
        }
    }
}

#[derive(Copy, Clone)]
enum MakeStatus {
    Spec0,
    Spec1,
    Spec2,
}

impl MakeStatus {
    fn apply(self, chan: usize, data: &[u32; 2], status: &mut PadStatus, origin: &[[u8; 12]; 4]) {
        match self {
            Self::Spec0 => spec0_make_status(data, status),
            Self::Spec1 => spec1_make_status(data, status),
            Self::Spec2 => spec2_make_status(chan, data, status, origin),
        }
    }
}

struct PadState {
    initialized: bool,
    recalibrated: bool,
    spec: u32,
    make_status: MakeStatus,
    analog_mode: u32,
    cmd_read_origin: u32,
    cmd_calibrate: u32,
    xpatch_bits: u32,
    resetting_bits: u32,
    waiting_bits: u32,
    pending_bits: u32,
    checking_bits: u32,
    recalibrate_bits: u32,
    resetting_chan: u32,
    enable_bits: u32,
    keys: [KeyInput; 4],
    origin: [[u8; 12]; 4],
    cmd_probe_device: [u32; 4],
    pad_type: [u32; 4],
    clamp_region: [u8; 8],
}

impl PadState {
    const fn new() -> Self {
        Self {
            initialized: false,
            recalibrated: false,
            spec: 5,
            make_status: MakeStatus::Spec2,
            analog_mode: 0x0000_0300,
            cmd_read_origin: 0x4100_0000,
            cmd_calibrate: 0x4200_0000,
            xpatch_bits: 0xf000_0000,
            resetting_bits: 0,
            waiting_bits: 0,
            pending_bits: 0,
            checking_bits: 0,
            recalibrate_bits: 0,
            resetting_chan: 32,
            enable_bits: 0,
            keys: [
                KeyInput::new(),
                KeyInput::new(),
                KeyInput::new(),
                KeyInput::new(),
            ],
            origin: [[0; 12]; 4],
            cmd_probe_device: [0; 4],
            pad_type: [0; 4],
            clamp_region: [30, 180, 15, 72, 40, 15, 59, 31],
        }
    }
}

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) -> SpinGuard<'_> {
        let interrupt_guard = ogc_sys::InterruptGuard::new();
        while self
            .0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinGuard {
            lock: self,
            _interrupt_guard: interrupt_guard,
        }
    }
}

struct SpinGuard<'a> {
    lock: &'a SpinLock,
    _interrupt_guard: ogc_sys::InterruptGuard,
}

impl Drop for SpinGuard<'_> {
    fn drop(&mut self) {
        self.lock.0.store(false, Ordering::Release);
    }
}

static PAD_LOCK: SpinLock = SpinLock::new();
static mut PAD_STATE: PadState = PadState::new();
static mut PAD_SAMPLING_CALLBACK: SamplingCallback = None;
static mut PAD_DEBUG_HOOK: DebugHook = None;
static mut PAD_TRACE_HOOK: PadTraceHook = None;

const PAD_TRACE_CAPACITY: u32 = 4_096;

struct PadTraceSlot {
    ready_sequence: AtomicU32,
    kind: AtomicU32,
    channel: AtomicU32,
    a: AtomicU32,
    b: AtomicU32,
    c: AtomicU32,
    d: AtomicU32,
}

impl PadTraceSlot {
    const fn new() -> Self {
        Self {
            ready_sequence: AtomicU32::new(0),
            kind: AtomicU32::new(0),
            channel: AtomicU32::new(0),
            a: AtomicU32::new(0),
            b: AtomicU32::new(0),
            c: AtomicU32::new(0),
            d: AtomicU32::new(0),
        }
    }
}

static PAD_TRACE_NEXT: AtomicU32 = AtomicU32::new(0);
static PAD_TRACE_DRAINED: AtomicU32 = AtomicU32::new(0);
static PAD_TRACE_DROPPED: AtomicU32 = AtomicU32::new(0);
static PAD_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
static PAD_TRACE_SLOTS: [PadTraceSlot; PAD_TRACE_CAPACITY as usize] =
    [const { PadTraceSlot::new() }; PAD_TRACE_CAPACITY as usize];
static mut PAD_RESET_INFO: SysResetInfo = SysResetInfo {
    node: LwpNode {
        next: core::ptr::null_mut(),
        prev: core::ptr::null_mut(),
    },
    func: Some(pad_on_reset),
    prio: 127,
};

pub static mut PAD_FIX_BITS: u32 = 0;

const LOWMEM_RECALIBRATION_DISABLE: usize = 0x8000_30e3;

fn pad_state_ptr() -> *mut PadState {
    core::ptr::addr_of_mut!(PAD_STATE)
}

fn pad_debug(stage: u32) {
    unsafe {
        if let Some(hook) = PAD_DEBUG_HOOK {
            hook(stage);
        }
    }
}

fn pad_trace(kind: u32, channel: i32, a: u32, b: u32, c: u32, d: u32) {
    if !PAD_TRACE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut sequence = PAD_TRACE_NEXT.load(Ordering::Acquire);
    loop {
        let drained = PAD_TRACE_DRAINED.load(Ordering::Acquire);
        if sequence.wrapping_sub(drained) >= PAD_TRACE_CAPACITY {
            PAD_TRACE_DROPPED.fetch_add(1, Ordering::Relaxed);
            let event = PadDebugEvent {
                sequence,
                kind,
                channel,
                a,
                b,
                c,
                d,
            };
            unsafe {
                if let Some(hook) = PAD_TRACE_HOOK {
                    hook(event);
                }
            }
            return;
        }
        match PAD_TRACE_NEXT.compare_exchange_weak(
            sequence,
            sequence.wrapping_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => break,
            Err(current) => sequence = current,
        }
    }
    let event = PadDebugEvent {
        sequence,
        kind,
        channel,
        a,
        b,
        c,
        d,
    };

    let slot = &PAD_TRACE_SLOTS[(sequence % PAD_TRACE_CAPACITY) as usize];
    slot.kind.store(kind, Ordering::Relaxed);
    slot.channel.store(channel as u32, Ordering::Relaxed);
    slot.a.store(a, Ordering::Relaxed);
    slot.b.store(b, Ordering::Relaxed);
    slot.c.store(c, Ordering::Relaxed);
    slot.d.store(d, Ordering::Relaxed);
    slot.ready_sequence
        .store(sequence.wrapping_add(1), Ordering::Release);

    unsafe {
        if let Some(hook) = PAD_TRACE_HOOK {
            hook(event);
        }
    }
}

fn pad_trace_state(kind: u32, channel: i32, state: &PadState) {
    pad_trace(
        kind,
        channel,
        state.resetting_bits,
        state.pending_bits,
        state.waiting_bits,
        state.checking_bits,
    );
    pad_trace(
        PAD_TRACE_STATE_RECAL_ENABLE,
        channel,
        state.recalibrate_bits,
        state.enable_bits,
        state.resetting_chan,
        state.spec,
    );
}

fn pad_trace_origin(kind: u32, chan: usize) {
    let origin = unsafe { (*pad_state_ptr()).origin[chan] };
    pad_trace(
        kind,
        chan as i32,
        u32::from_be_bytes([origin[0], origin[1], origin[2], origin[3]]),
        u32::from_be_bytes([origin[4], origin[5], origin[6], origin[7]]),
        u32::from_be_bytes([origin[8], origin[9], 0, 0]),
        10,
    );
}

fn pad_enabled_mask(chan: usize) -> u32 {
    PAD_CHAN0_BIT >> chan
}

fn pad_chan_valid(chan: i32) -> bool {
    (0..4).contains(&chan)
}

fn pad_cntlzw(value: u32) -> u32 {
    value.leading_zeros()
}

fn pad_recalibration_disabled() -> bool {
    let reg = LOWMEM_RECALIBRATION_DISABLE as *const u8;
    unsafe { core::ptr::read_volatile(reg) & 0x40 != 0 }
}

fn update_recalibration_disable(value: u8, disable: bool) -> (bool, u8) {
    let was_disabled = value & 0x40 != 0;
    let value = (value & !0x40) | (u8::from(disable) << 6);
    (was_disabled, value)
}

fn origin_needs_xpatch(origin: u8) -> bool {
    (origin as i8) > 64
}

fn spec0_make_status(data: &[u32; 2], status: &mut PadStatus) {
    status.button = 0;

    if data[0] & 0x0008_0000 != 0 {
        status.button |= PAD_BUTTON_A;
    }
    if data[0] & 0x0020_0000 != 0 {
        status.button |= PAD_BUTTON_B;
    }
    if data[0] & 0x0100_0000 != 0 {
        status.button |= PAD_BUTTON_X;
    }
    if data[0] & 0x0001_0000 != 0 {
        status.button |= PAD_BUTTON_Y;
    }
    if data[0] & 0x0010_0000 != 0 {
        status.button |= PAD_BUTTON_MENU;
    }

    status.stick_x = (data[1] >> 16) as i8;
    status.stick_y = (data[1] >> 24) as i8;
    status.substick_x = data[1] as i8;
    status.substick_y = (data[1] >> 8) as i8;
    status.trigger_l = ((data[0] >> 8) & 0xff) as u8;
    status.trigger_r = (data[0] & 0xff) as u8;
    status.analog_a = 0;
    status.analog_b = 0;

    if status.trigger_l >= 0xaa {
        status.button |= PAD_TRIGGER_L;
    }
    if status.trigger_r >= 0xaa {
        status.button |= PAD_TRIGGER_R;
    }

    status.stick_x = ((status.stick_x as i16) - 128) as i8;
    status.stick_y = ((status.stick_y as i16) - 128) as i8;
    status.substick_x = ((status.substick_x as i16) - 128) as i8;
    status.substick_y = ((status.substick_y as i16) - 128) as i8;
}

fn spec1_make_status(data: &[u32; 2], status: &mut PadStatus) {
    status.button = 0;

    if data[0] & 0x0080_0000 != 0 {
        status.button |= PAD_BUTTON_A;
    }
    if data[0] & 0x0100_0000 != 0 {
        status.button |= PAD_BUTTON_B;
    }
    if data[0] & 0x0020_0000 != 0 {
        status.button |= PAD_BUTTON_X;
    }
    if data[0] & 0x0010_0000 != 0 {
        status.button |= PAD_BUTTON_Y;
    }
    if data[0] & 0x0200_0000 != 0 {
        status.button |= PAD_BUTTON_MENU;
    }

    status.stick_x = (data[1] >> 16) as i8;
    status.stick_y = (data[1] >> 24) as i8;
    status.substick_x = data[1] as i8;
    status.substick_y = (data[1] >> 8) as i8;
    status.trigger_l = ((data[0] >> 8) & 0xff) as u8;
    status.trigger_r = (data[0] & 0xff) as u8;
    status.analog_a = 0;
    status.analog_b = 0;

    if status.trigger_l >= 0xaa {
        status.button |= PAD_TRIGGER_L;
    }
    if status.trigger_r >= 0xaa {
        status.button |= PAD_TRIGGER_R;
    }

    status.stick_x = ((status.stick_x as i16) - 128) as i8;
    status.stick_y = ((status.stick_y as i16) - 128) as i8;
    status.substick_x = ((status.substick_x as i16) - 128) as i8;
    status.substick_y = ((status.substick_y as i16) - 128) as i8;
}

fn pad_clamp_s8(var: i8, org: u8) -> i8 {
    let mut value = var as i32;
    let org = org as i8 as i32;
    if org > 0 {
        let min = org - 128;
        if value < min {
            value = min;
        }
    } else if org < 0 {
        let max = org + 127;
        if max < value {
            value = max;
        }
    }
    (value - org) as i8
}

fn pad_clamp_u8(var: u8, org: u8) -> u8 {
    var.saturating_sub(org)
}

fn spec2_make_status(chan: usize, data: &[u32; 2], status: &mut PadStatus, origin: &[[u8; 12]; 4]) {
    status.button = ((data[0] >> 16) & 0x3fff) as u16;
    status.stick_x = (data[0] >> 8) as i8;
    status.stick_y = data[0] as i8;
    let mode = unsafe { (*pad_state_ptr()).analog_mode & 0x0700 };
    if mode == 0x0100 {
        status.substick_x = ((data[1] >> 24) & 0xf0) as i8;
        status.substick_y = ((data[1] >> 8) & 0xff) as i8;
        status.trigger_l = ((data[1] >> 16) & 0xff) as u8;
        status.trigger_r = ((data[1] >> 8) & 0xff) as u8;
        status.analog_a = (data[1] & 0xf0) as u8;
        status.analog_b = ((data[1] << 4) & 0xf0) as u8;
    } else if mode == 0x0200 {
        status.substick_x = ((data[1] >> 24) & 0xf0) as i8;
        status.substick_y = ((data[1] >> 20) & 0xf0) as i8;
        status.trigger_l = ((data[1] >> 16) & 0xf0) as u8;
        status.trigger_r = ((data[1] >> 12) & 0xf0) as u8;
        status.analog_a = ((data[1] >> 8) & 0xff) as u8;
        status.analog_b = (data[1] & 0xff) as u8;
    } else if mode == 0x0300 {
        status.substick_x = ((data[1] >> 24) & 0xff) as i8;
        status.substick_y = ((data[1] >> 16) & 0xff) as i8;
        status.trigger_l = ((data[1] >> 8) & 0xff) as u8;
        status.trigger_r = (data[1] & 0xff) as u8;
        status.analog_a = 0;
        status.analog_b = 0;
    } else if mode == 0x0400 {
        status.substick_x = ((data[1] >> 24) & 0xff) as i8;
        status.substick_y = ((data[1] >> 16) & 0xff) as i8;
        status.trigger_l = 0;
        status.trigger_r = 0;
        status.analog_a = ((data[1] >> 8) & 0xff) as u8;
        status.analog_b = (data[1] & 0xff) as u8;
    } else {
        status.substick_x = ((data[1] >> 24) & 0xff) as i8;
        status.substick_y = ((data[1] >> 16) & 0xff) as i8;
        status.trigger_l = ((data[1] >> 8) & 0xf0) as u8;
        status.trigger_r = ((data[1] >> 4) & 0xf0) as u8;
        status.analog_a = (data[1] & 0xf0) as u8;
        status.analog_b = ((data[1] << 4) & 0xf0) as u8;
    }

    status.stick_x = ((status.stick_x as i16) - 128) as i8;
    status.stick_y = ((status.stick_y as i16) - 128) as i8;
    status.substick_x = ((status.substick_x as i16) - 128) as i8;
    status.substick_y = ((status.substick_y as i16) - 128) as i8;
    status.stick_x = pad_clamp_s8(status.stick_x, origin[chan][2]);
    status.stick_y = pad_clamp_s8(status.stick_y, origin[chan][3]);
    status.substick_x = pad_clamp_s8(status.substick_x, origin[chan][4]);
    status.substick_y = pad_clamp_s8(status.substick_y, origin[chan][5]);
    status.trigger_l = pad_clamp_u8(status.trigger_l, origin[chan][6]);
    status.trigger_r = pad_clamp_u8(status.trigger_r, origin[chan][7]);
}

fn pad_clampstick(px: &mut i8, py: &mut i8, max: i8, xy: i8, min: i8) {
    let mut x = *px as i32;
    let mut y = *py as i32;
    let sign_x = if x >= 0 {
        1
    } else {
        x = -x;
        -1
    };
    let sign_y = if y >= 0 {
        1
    } else {
        y = -y;
        -1
    };
    let mut x = if x <= min as i32 { 0 } else { x - min as i32 };
    let mut y = if y <= min as i32 { 0 } else { y - min as i32 };

    if x != 0 || y != 0 {
        let xx = x * xy as i32;
        let yy = y * xy as i32;
        let maxy = max as i32 * xy as i32;
        let d = if yy <= xx {
            let d = x * xy as i32 + y * (max as i32 - xy as i32);
            if maxy < d {
                x = x * maxy / d;
                y = y * maxy / d;
            }
            d
        } else {
            let d = y * xy as i32 + x * (max as i32 - xy as i32);
            if maxy < d {
                x = x * maxy / d;
                y = y * maxy / d;
            }
            d
        };
        let _ = d;
        *px = (x * sign_x) as i8;
        *py = (y * sign_y) as i8;
    } else {
        *px = 0;
        *py = 0;
    }
}

fn pad_clamptrigger(trigger: &mut u8) {
    let state = unsafe { &*pad_state_ptr() };
    let min = state.clamp_region[0];
    let max = state.clamp_region[1];
    if min > *trigger {
        *trigger = 0;
    } else if max < *trigger {
        *trigger = max - min;
    } else {
        *trigger -= min;
    }
}

fn pad_update_origin(chan: usize) {
    let state = unsafe { &mut *pad_state_ptr() };
    let mode = state.analog_mode & 0x0700;
    if mode == 0x0100 {
        state.origin[chan][4] &= !0x0f;
        state.origin[chan][5] &= !0x0f;
        state.origin[chan][8] &= !0x0f;
        state.origin[chan][9] &= !0x0f;
    } else if mode == 0x0200 {
        state.origin[chan][4] &= !0x0f;
        state.origin[chan][5] &= !0x0f;
        state.origin[chan][6] &= !0x0f;
        state.origin[chan][7] &= !0x0f;
    }
    state.origin[chan][2] = state.origin[chan][2].wrapping_sub(128);
    state.origin[chan][3] = state.origin[chan][3].wrapping_sub(128);
    state.origin[chan][4] = state.origin[chan][4].wrapping_sub(128);
    state.origin[chan][5] = state.origin[chan][5].wrapping_sub(128);
    if (state.xpatch_bits & pad_enabled_mask(chan)) != 0
        && origin_needs_xpatch(state.origin[chan][2])
    {
        let type_ = si::get_type(chan as i32) & !0xffff;
        if (type_ & !0x09ff_ffff) == 0 {
            state.origin[chan][2] = 0;
        }
    }
}

unsafe extern "C" fn pad_sampling_handler(_irq: u32, _ctx: *mut c_void) {
    unsafe {
        let cb = { PAD_SAMPLING_CALLBACK };
        if let Some(cb) = cb {
            cb();
        }
    }
}

unsafe fn pad_transfer(
    chan: i32,
    out: *const c_void,
    out_len: u32,
    in_: *mut c_void,
    in_len: u32,
    callback: si::SiCallback,
    delay: u32,
) -> u32 {
    let command = if out.is_null() || out_len == 0 {
        0
    } else {
        unsafe { *(out.cast::<u8>()) as u32 }
    };
    pad_trace(PAD_TRACE_TRANSFER, chan, command, out_len, in_len, delay);
    let result = unsafe { si::transfer(chan, out, out_len, in_, in_len, callback, delay) };
    pad_trace(
        PAD_TRACE_TRANSFER_RESULT,
        chan,
        command,
        result,
        out_len,
        in_len,
    );
    result
}

unsafe extern "C" fn pad_on_reset(final_: i32) -> i32 {
    pad_trace(
        PAD_TRACE_RESET_CALLBACK,
        -1,
        final_ as u32,
        unsafe { (*pad_state_ptr()).recalibrated as u32 },
        0,
        0,
    );
    if unsafe { PAD_SAMPLING_CALLBACK }.is_some() {
        let _ = pad_set_sampling_callback(None);
    }
    if final_ == 0 {
        let ret = pad_sync();
        let state = unsafe { &mut *pad_state_ptr() };
        if !state.recalibrated && ret != 0 {
            state.recalibrated = pad_recalibrate(0xf000_0000) != 0;
            return 0;
        }
        return ret as i32;
    }
    unsafe {
        (*pad_state_ptr()).recalibrated = false;
    }
    1
}

unsafe extern "C" fn pad_probecallback(chan: i32, type_: u32) {
    let state = unsafe { &mut *pad_state_ptr() };
    pad_trace(PAD_TRACE_CALLBACK, chan, 1, type_, type_ & 0xff, 0);
    if type_ & 0x0f == 0 {
        pad_enable(state, state.resetting_chan as usize);
        state.waiting_bits |= pad_enabled_mask(state.resetting_chan as usize);
    }
    pad_doreset(state);
    let _ = chan;
}

unsafe extern "C" fn pad_origincallback(chan: i32, type_: u32) {
    let state = unsafe { &mut *pad_state_ptr() };

    let reset_chan = state.resetting_chan as usize;
    pad_trace(PAD_TRACE_CALLBACK, chan, 2, type_, type_ & 0xff, 0);

    if type_ & 0x0f == 0 && reset_chan < PAD_CHANMAX as usize {
        pad_trace_origin(PAD_TRACE_ORIGIN_RAW_0, reset_chan);
        pad_update_origin(reset_chan);
        pad_trace_origin(PAD_TRACE_ORIGIN_TRANSFORMED, reset_chan);
        pad_enable(state, reset_chan);
    }

    pad_doreset(state);

    let _ = chan;
    let _ = type_;
}

unsafe extern "C" fn pad_originupdatecallback(chan: i32, type_: u32) {
    let state = unsafe { &mut *pad_state_ptr() };
    pad_trace(PAD_TRACE_CALLBACK, chan, 3, type_, type_ & 0xff, 0);
    let mask = pad_enabled_mask(chan as usize);
    if (state.enable_bits & mask) != 0 {
        if type_ & 0x0f == 0 {
            pad_trace_origin(PAD_TRACE_ORIGIN_RAW_1, chan as usize);
            pad_update_origin(chan as usize);
            pad_trace_origin(PAD_TRACE_ORIGIN_TRANSFORMED, chan as usize);
        }
        if type_ & si::SI_ERROR_NO_RESPONSE != 0 {
            pad_disable(state, chan as usize);
        }
    }
}

unsafe extern "C" fn pad_typeandstatuscallback(chan: i32, type_: u32) {
    let state = unsafe { &mut *pad_state_ptr() };
    pad_trace(
        PAD_TRACE_TYPE_RESULT,
        chan,
        type_,
        state.resetting_chan,
        state.recalibrate_bits,
        state.spec,
    );
    let mask = pad_enabled_mask(state.resetting_chan as usize);
    let recal_bits = state.recalibrate_bits & mask;
    state.recalibrate_bits &= !mask;

    if type_ & 0x0f != 0 {
        pad_doreset(state);
        return;
    }

    state.pad_type[state.resetting_chan as usize] = type_ & !0xff;
    if (type_ & si::SI_TYPE_MASK) != si::SI_TYPE_GC || (type_ & si::SI_GC_STANDARD) == 0 {
        pad_doreset(state);
        return;
    }

    if state.spec < 2 {
        pad_enable(state, state.resetting_chan as usize);
        pad_doreset(state);
        return;
    }

    let mut ret = 0;
    if (type_ & si::SI_GC_WIRELESS) == 0 || (type_ & si::SI_WIRELESS_IR) != 0 {
        if recal_bits != 0 {
            ret = unsafe {
                pad_transfer(
                    state.resetting_chan as i32,
                    (&state.cmd_calibrate as *const u32).cast(),
                    3,
                    state.origin[state.resetting_chan as usize]
                        .as_mut_ptr()
                        .cast(),
                    10,
                    Some(pad_origincallback),
                    0,
                )
            };
        } else {
            ret = unsafe {
                pad_transfer(
                    state.resetting_chan as i32,
                    (&state.cmd_read_origin as *const u32).cast(),
                    1,
                    state.origin[state.resetting_chan as usize]
                        .as_mut_ptr()
                        .cast(),
                    10,
                    Some(pad_origincallback),
                    0,
                )
            };
        }
    } else if (type_ & si::SI_GC_WIRELESS) != 0
        && (type_ & si::SI_WIRELESS_FIX_ID) != 0
        && (type_ & si::SI_WIRELESS_CONT_MASK) == 0
        && (type_ & si::SI_WIRELESS_LITE) == 0
    {
        if (type_ & si::SI_WIRELESS_RECEIVED) != 0 {
            ret = unsafe {
                pad_transfer(
                    state.resetting_chan as i32,
                    (&state.cmd_read_origin as *const u32).cast(),
                    1,
                    state.origin[state.resetting_chan as usize]
                        .as_mut_ptr()
                        .cast(),
                    10,
                    Some(pad_origincallback),
                    0,
                )
            };
        } else {
            ret = unsafe {
                pad_transfer(
                    state.resetting_chan as i32,
                    (&state.cmd_probe_device[state.resetting_chan as usize] as *const u32).cast(),
                    3,
                    state.origin[state.resetting_chan as usize]
                        .as_mut_ptr()
                        .cast(),
                    8,
                    Some(pad_probecallback),
                    0,
                )
            };
        }
    }

    if ret == 0 {
        state.pending_bits |= mask;
        pad_doreset(state);
        let _ = chan;
        return;
    }

    let _ = chan;
}

unsafe extern "C" fn pad_receivecheckcallback(chan: i32, type_: u32) {
    let state = unsafe { &mut *pad_state_ptr() };
    pad_trace(PAD_TRACE_CALLBACK, chan, 4, type_, type_ & 0xff, 0);
    let mask = pad_enabled_mask(chan as usize);
    if (state.enable_bits & mask) == 0 {
        return;
    }
    let tmp = type_ & 0xff;
    let type_ = type_ & !0xff;
    state.waiting_bits &= !mask;
    state.checking_bits &= !mask;
    if tmp & 0x0f == 0
        && (type_ & si::SI_GC_WIRELESS) != 0
        && (type_ & si::SI_WIRELESS_RECEIVED) != 0
        && (type_ & si::SI_WIRELESS_FIX_ID) != 0
        && (type_ & si::SI_WIRELESS_IR) == 0
        && (type_ & si::SI_WIRELESS_CONT_MASK) == 0
        && (type_ & si::SI_WIRELESS_LITE) == 0
    {
        let _ = unsafe {
            pad_transfer(
                chan,
                (&state.cmd_read_origin as *const u32).cast(),
                1,
                state.origin[chan as usize].as_mut_ptr().cast(),
                10,
                Some(pad_originupdatecallback),
                0,
            )
        };
    } else {
        pad_disable(state, chan as usize);
    }
}

#[inline]
fn si_get_response(chan: i32, buffer: &mut [u32; 2]) -> u32 {
    // SAFETY: `buffer` is properly aligned and valid for writes to exactly
    // two consecutive `u32` values for the full duration of this call.
    unsafe { si::get_response(chan, buffer.as_mut_ptr().cast()) }
}

fn pad_enable(state: &mut PadState, chan: usize) {
    let mut buf = [0u32; 2];
    state.enable_bits |= pad_enabled_mask(chan);
    let _ = si_get_response(chan as i32, &mut buf);
    si::set_command(chan as i32, state.analog_mode | 0x0040_0000);
    si::enable_polling(state.enable_bits);
    pad_trace(
        PAD_TRACE_ENABLE_CHANNEL,
        chan as i32,
        state.enable_bits,
        state.waiting_bits,
        state.checking_bits,
        0,
    );
}

fn pad_disable(state: &mut PadState, chan: usize) {
    let mask = pad_enabled_mask(chan);
    si::disable_polling(mask);
    state.enable_bits &= !mask;
    state.waiting_bits &= !mask;
    state.pending_bits &= !mask;
    state.checking_bits &= !mask;
    set_wireless_id(chan as u32, 0);
    pad_trace(
        PAD_TRACE_DISABLE_CHANNEL,
        chan as i32,
        mask,
        state.enable_bits,
        state.waiting_bits,
        state.checking_bits,
    );
}

fn pad_doreset(state: &mut PadState) {
    state.resetting_chan = pad_cntlzw(state.resetting_bits);
    if state.resetting_chan == 32 {
        pad_trace_state(PAD_TRACE_FINAL_STATE, -1, state);
        return;
    }
    let selected = state.resetting_chan as usize;
    debug_assert!(selected < PAD_CHANMAX as usize);
    state.resetting_bits &= !pad_enabled_mask(state.resetting_chan as usize);
    state.origin[state.resetting_chan as usize] = [0; 12];
    pad_trace_state(PAD_TRACE_SELECT_CHANNEL, state.resetting_chan as i32, state);
    let _ = si::get_type_async(state.resetting_chan as i32, Some(pad_typeandstatuscallback));
}

fn pad_current_state() -> &'static mut PadState {
    unsafe { &mut *pad_state_ptr() }
}

fn pad_set_spec_inner(state: &mut PadState, spec: u32) {
    if spec == 0 {
        state.make_status = MakeStatus::Spec0;
    } else if spec == 1 {
        state.make_status = MakeStatus::Spec1;
    } else if spec < 6 {
        state.make_status = MakeStatus::Spec2;
    }
    state.spec = spec;
}

struct ResetPreparation {
    disable_bits: u32,
    start_reset: bool,
}

fn pad_prepare_reset_inner(
    state: &mut PadState,
    mask: u32,
    mark_recalibrate: bool,
) -> ResetPreparation {
    let valid_mask = mask & PAD_VALID_MASK;
    let pending = state.pending_bits & PAD_VALID_MASK;
    let mut pend_bits = pending | valid_mask;
    state.pending_bits = 0;
    pend_bits &= !(state.waiting_bits | state.checking_bits);
    pend_bits &= PAD_VALID_MASK;
    state.resetting_bits |= pend_bits;
    state.resetting_bits &= PAD_VALID_MASK;
    let disable_bits = state.resetting_bits & state.enable_bits;
    state.enable_bits &= !pend_bits;
    if mark_recalibrate {
        state.recalibrate_bits |= pend_bits;
    }
    state.recalibrate_bits &= PAD_VALID_MASK;

    pad_trace(
        PAD_TRACE_PREPARE_MASKS,
        -1,
        mask,
        valid_mask,
        pending,
        pend_bits,
    );
    pad_trace_state(PAD_TRACE_PREPARE_STATE, -1, state);

    ResetPreparation {
        disable_bits,
        start_reset: state.resetting_chan == 32,
    }
}

fn pad_start_reset_inner(state: &mut PadState, mask: u32, mark_recalibrate: bool) {
    let preparation = pad_prepare_reset_inner(state, mask, mark_recalibrate);
    pad_trace(
        PAD_TRACE_DISABLE_POLLING,
        -1,
        preparation.disable_bits,
        state.enable_bits,
        mask,
        mark_recalibrate as u32,
    );
    si::disable_polling(preparation.disable_bits);
    if preparation.start_reset {
        pad_doreset(state);
    }
}

fn pad_reset_inner(state: &mut PadState, mask: u32) {
    let mark_recalibrate = state.spec == 4;
    pad_start_reset_inner(state, mask, mark_recalibrate);
}

pub fn pad_init() -> u32 {
    pad_debug(0);
    si::init();
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    if state.initialized {
        pad_debug(1);
        return 1;
    }
    if state.spec != 0 {
        let spec = state.spec;
        pad_set_spec_inner(state, spec);
    }
    pad_debug(2);
    for key in state.keys.iter_mut() {
        *key = KeyInput::new();
    }
    state.recalibrate_bits = 0xf000_0000;
    for chan in 0..4 {
        state.keys[chan].chan = -1;
        state.cmd_probe_device[chan] = 0x4d00_0000 | ((chan as u32) << 22) | ((6u32) << 8);
    }
    pad_debug(3);
    si::refresh_sampling_rate();
    pad_debug(4);
    unsafe { register_reset_func(core::ptr::addr_of_mut!(PAD_RESET_INFO)) };
    pad_debug(5);
    state.initialized = true;
    pad_debug(6);
    drop(_guard);
    pad_debug(7);
    pad_reset(0xf000_0000)
}

pub fn set_debug_hook(hook: DebugHook) {
    unsafe {
        PAD_DEBUG_HOOK = hook;
    }
}

/// Installs a passive PAD trace observer and returns the previous observer.
///
/// The observer can run from an SI callback. It must not allocate, block, or
/// call PAD or SI functions.
pub fn set_trace_hook(hook: PadTraceHook) -> PadTraceHook {
    let _guard = PAD_LOCK.lock();
    unsafe {
        let previous = PAD_TRACE_HOOK;
        if hook.is_some() && !PAD_TRACE_ENABLED.load(Ordering::Acquire) {
            PAD_TRACE_NEXT.store(0, Ordering::Release);
            PAD_TRACE_DRAINED.store(0, Ordering::Release);
            PAD_TRACE_DROPPED.store(0, Ordering::Release);
            for slot in &PAD_TRACE_SLOTS {
                slot.ready_sequence.store(0, Ordering::Release);
            }
        }
        PAD_TRACE_HOOK = hook;
        PAD_TRACE_ENABLED.store(hook.is_some(), Ordering::Release);
        previous
    }
}

/// Drains ready PAD trace events in sequence order into `events`.
pub fn pad_debug_drain(events: &mut [PadDebugEvent]) -> usize {
    let mut drained = PAD_TRACE_DRAINED.load(Ordering::Acquire);
    let produced = PAD_TRACE_NEXT.load(Ordering::Acquire);
    let available = produced.wrapping_sub(drained).min(events.len() as u32);
    let mut count = 0usize;
    while count < available as usize {
        let slot = &PAD_TRACE_SLOTS[(drained % PAD_TRACE_CAPACITY) as usize];
        if slot.ready_sequence.load(Ordering::Acquire) != drained.wrapping_add(1) {
            break;
        }
        events[count] = PadDebugEvent {
            sequence: drained,
            kind: slot.kind.load(Ordering::Relaxed),
            channel: slot.channel.load(Ordering::Relaxed) as i32,
            a: slot.a.load(Ordering::Relaxed),
            b: slot.b.load(Ordering::Relaxed),
            c: slot.c.load(Ordering::Relaxed),
            d: slot.d.load(Ordering::Relaxed),
        };
        slot.ready_sequence.store(0, Ordering::Release);
        drained = drained.wrapping_add(1);
        count += 1;
    }
    PAD_TRACE_DRAINED.store(drained, Ordering::Release);
    count
}

pub fn pad_debug_counters() -> PadDebugCounters {
    PadDebugCounters {
        produced: PAD_TRACE_NEXT.load(Ordering::Acquire),
        drained: PAD_TRACE_DRAINED.load(Ordering::Acquire),
        dropped: PAD_TRACE_DROPPED.load(Ordering::Acquire),
    }
}

pub fn pad_debug_state() -> PadDebugState {
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    PadDebugState {
        spec: state.spec,
        resetting_bits: state.resetting_bits,
        waiting_bits: state.waiting_bits,
        pending_bits: state.pending_bits,
        checking_bits: state.checking_bits,
        recalibrate_bits: state.recalibrate_bits,
        resetting_chan: state.resetting_chan,
        enable_bits: state.enable_bits,
    }
}

/// Applies the active production PAD status transform to diagnostic SI words.
///
/// This helper does not change PAD state. It exists so origin probes can check
/// all analog fields with deterministic SI payloads.
pub fn pad_debug_make_status(chan: i32, data: [u32; 2]) -> PadStatus {
    if !pad_chan_valid(chan) {
        return PadStatus {
            err: PAD_ERR_NO_CONTROLLER as i8,
            ..PadStatus::default()
        };
    }
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    let mut status = PadStatus::default();
    state
        .make_status
        .apply(chan as usize, &data, &mut status, &state.origin);
    status
}

/// # Safety
///
/// `status` must point to at least `PAD_CHANMAX` contiguous `PadStatus`
/// values.
pub unsafe fn pad_read(status: *mut PadStatus) -> u32 {
    if status.is_null() {
        return 0;
    }
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    let mut connected = 0;
    for chan in 0..4 {
        let mask = pad_enabled_mask(chan);
        let slot = unsafe { &mut *status.add(chan) };
        if (state.pending_bits & mask) != 0 {
            pad_reset_inner(state, 0);
            *slot = PadStatus {
                err: PAD_ERR_NOT_READY as i8,
                ..PadStatus::default()
            };
        } else if (state.resetting_bits & mask) != 0 || state.resetting_chan == chan as u32 {
            *slot = PadStatus {
                err: PAD_ERR_NOT_READY as i8,
                ..PadStatus::default()
            };
        } else if (state.enable_bits & mask) == 0 {
            *slot = PadStatus {
                err: PAD_ERR_NO_CONTROLLER as i8,
                ..PadStatus::default()
            };
        } else if si::is_chan_busy(chan as i32) != 0 {
            *slot = PadStatus {
                err: PAD_ERR_TRANSFER as i8,
                ..PadStatus::default()
            };
        } else {
            let sistatus = si::get_status(chan as i32);
            if (sistatus & si::SI_ERROR_NO_RESPONSE) != 0 {
                let mut buf = [0u32; 2];
                let response_ready = si_get_response(chan as i32, &mut buf) != 0;
                pad_trace(
                    PAD_TRACE_TRANSFER_RESULT,
                    chan as i32,
                    0,
                    response_ready as u32,
                    sistatus,
                    0,
                );
                if (state.waiting_bits & mask) == 0 {
                    *slot = PadStatus::default();
                    slot.err = PAD_ERR_NONE as i8;
                    if (state.checking_bits & mask) == 0 {
                        state.checking_bits |= mask;
                        let type_ = si::get_type_async(chan as i32, Some(pad_receivecheckcallback));
                        if type_ == si::SI_ERROR_UNKNOWN {
                            state.checking_bits &= !mask;
                            pad_disable(state, chan);
                            slot.err = PAD_ERR_NO_CONTROLLER as i8;
                        }
                    }
                } else {
                    pad_disable(state, chan);
                    *slot = PadStatus {
                        err: PAD_ERR_NO_CONTROLLER as i8,
                        ..PadStatus::default()
                    };
                }
            } else {
                let type_ = si::get_type(chan as i32);
                if (type_ & si::SI_WIRELESS_STATE) == 0 {
                    connected |= mask;
                }
                let mut buf = [0u32; 2];
                if si_get_response(chan as i32, &mut buf) == 0 || (buf[0] & 0x8000_0000) != 0 {
                    *slot = PadStatus {
                        err: PAD_ERR_TRANSFER as i8,
                        ..PadStatus::default()
                    };
                } else {
                    state.make_status.apply(chan, &buf, slot, &state.origin);
                    if (slot.button & 0x0000_2000) != 0 {
                        *slot = PadStatus {
                            err: PAD_ERR_TRANSFER as i8,
                            ..PadStatus::default()
                        };
                        // C keeps this channel connected when SI cannot queue
                        // the origin read and reports a transfer error here.
                        let _ = unsafe {
                            pad_transfer(
                                chan as i32,
                                (&state.cmd_read_origin as *const u32).cast(),
                                1,
                                state.origin[chan].as_mut_ptr().cast(),
                                10,
                                Some(pad_originupdatecallback),
                                0,
                            )
                        };
                    } else {
                        slot.err = PAD_ERR_NONE as i8;
                        slot.button &= !0x80;
                    }
                }
            }
        }
    }
    connected
}

pub fn pad_reset(mask: u32) -> u32 {
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    pad_trace(
        PAD_TRACE_REQUEST_RESET,
        -1,
        mask,
        mask & PAD_VALID_MASK,
        state.pending_bits,
        state.resetting_bits,
    );
    pad_reset_inner(state, mask);
    1
}

pub fn pad_recalibrate(mask: u32) -> u32 {
    // BUG FIX: The C PAD_Recalibrate body is empty; apply the requested
    // calibration through the same reset path used by initial setup.
    let _guard = PAD_LOCK.lock();
    let recalibration_enabled = !pad_recalibration_disabled();
    let state = pad_current_state();
    pad_trace(
        PAD_TRACE_REQUEST_RECALIBRATE,
        -1,
        mask,
        mask & PAD_VALID_MASK,
        state.pending_bits,
        recalibration_enabled as u32,
    );
    pad_start_reset_inner(state, mask, recalibration_enabled);
    1
}

pub fn pad_sync() -> u32 {
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    let si_busy = si::busy();
    let synced = if state.resetting_bits == 0 && state.resetting_chan == 32 && si_busy == 0 {
        1
    } else {
        0
    };
    pad_trace(
        PAD_TRACE_SYNC,
        -1,
        synced,
        state.resetting_bits,
        state.resetting_chan,
        si_busy,
    );
    synced
}

pub fn pad_set_spec(spec: u32) {
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    if state.initialized {
        return;
    }
    pad_set_spec_inner(state, spec);
}

pub fn pad_control_motor(chan: i32, cmd: u32) {
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    if !pad_chan_valid(chan) {
        pad_trace(PAD_TRACE_MOTOR, chan, cmd, 0, 0, 0);
        return;
    }
    let mask = pad_enabled_mask(chan as usize);
    if (state.enable_bits & mask) != 0 {
        let type_ = si::get_type(chan);
        if (type_ & si::SI_GC_NOMOTOR) == 0 {
            let mut cmd = cmd;
            if state.spec < 2 && cmd == PAD_MOTOR_STOP_HARD {
                cmd = 0;
            }
            si::set_command(chan, 0x0040_0000 | state.analog_mode | (cmd & 0x03));
            si::transfer_commands();
            pad_trace(PAD_TRACE_MOTOR, chan, cmd, 1, type_, state.enable_bits);
        }
    } else {
        pad_trace(PAD_TRACE_MOTOR, chan, cmd, 0, 0, state.enable_bits);
    }
}

pub fn pad_set_sampling_callback(cb: SamplingCallback) -> SamplingCallback {
    let _guard = PAD_LOCK.lock();
    unsafe {
        update_sampling_callback(
            &mut *core::ptr::addr_of_mut!(PAD_SAMPLING_CALLBACK),
            cb,
            || si::register_polling_handler(Some(pad_sampling_handler)),
            || si::unregister_polling_handler(Some(pad_sampling_handler)),
        )
    }
}

fn update_sampling_callback(
    current: &mut SamplingCallback,
    cb: SamplingCallback,
    register: impl FnOnce() -> u32,
    unregister: impl FnOnce() -> u32,
) -> SamplingCallback {
    let old = *current;
    if cb.is_some() {
        let _ = register();
    } else {
        let _ = unregister();
    }
    // C stores the new callback before attempting the SI operation and keeps
    // that state even when SI reports a registration or removal failure.
    *current = cb;
    old
}

/// # Safety
///
/// `status` must point to at least `PAD_CHANMAX` contiguous `PadStatus`
/// values.
pub unsafe fn pad_clamp(status: *mut PadStatus) {
    if status.is_null() {
        return;
    }
    let state = pad_current_state();
    for chan in 0..4 {
        let slot = unsafe { &mut *status.add(chan) };
        if slot.err == PAD_ERR_NONE as i8 {
            pad_clampstick(
                &mut slot.stick_x,
                &mut slot.stick_y,
                state.clamp_region[3] as i8,
                state.clamp_region[4] as i8,
                state.clamp_region[2] as i8,
            );
            pad_clampstick(
                &mut slot.substick_x,
                &mut slot.substick_y,
                state.clamp_region[6] as i8,
                state.clamp_region[7] as i8,
                state.clamp_region[5] as i8,
            );
            pad_clamptrigger(&mut slot.trigger_l);
            pad_clamptrigger(&mut slot.trigger_r);
        }
    }
}

pub fn pad_scan_pads() -> u32 {
    let mut padstatus = [PadStatus::default(); 4];
    pad_scan_pads_with_status(&mut padstatus)
}

/// Scans PAD once and returns the same decoded samples used to update keys.
/// This avoids consuming another SI response when diagnostics need raw status.
pub fn pad_scan_pads_with_status(padstatus: &mut [PadStatus; 4]) -> u32 {
    let read_connected = unsafe { pad_read(padstatus.as_mut_ptr()) };
    let _guard = PAD_LOCK.lock();
    let state = pad_current_state();
    let mut reset_bits = 0;
    let mut connected = 0;
    for (chan, pad) in padstatus.iter().enumerate() {
        let pad_bit = PAD_CHAN0_BIT >> chan;
        match pad.err as i32 {
            PAD_ERR_NONE => {
                let oldstate = state.keys[chan].state;
                let st = *pad;
                state.keys[chan].stick_x = st.stick_x;
                state.keys[chan].stick_y = st.stick_y;
                state.keys[chan].substick_x = st.substick_x;
                state.keys[chan].substick_y = st.substick_y;
                state.keys[chan].trigger_l = st.trigger_l;
                state.keys[chan].trigger_r = st.trigger_r;
                state.keys[chan].up = oldstate & !st.button;
                state.keys[chan].down = st.button & (st.button ^ oldstate);
                state.keys[chan].state = st.button;
                state.keys[chan].chan = chan as i32;
                connected |= 1 << chan;
            }
            PAD_ERR_NO_CONTROLLER => {
                if state.keys[chan].chan != -1 {
                    state.keys[chan] = KeyInput::new();
                }
                state.keys[chan].chan = -1;
                reset_bits |= pad_bit;
            }
            _ => {}
        }
    }
    if reset_bits != 0 {
        pad_reset_inner(state, reset_bits);
    }
    pad_trace(
        PAD_TRACE_FINAL_STATE,
        -1,
        read_connected,
        connected,
        reset_bits,
        state.enable_bits,
    );
    connected
}

pub fn pad_buttons_up(pad: i32) -> u16 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].up
}

pub fn pad_buttons_down(pad: i32) -> u16 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].down
}

pub fn pad_buttons_held(pad: i32) -> u16 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].state
}

pub fn pad_sub_stick_x(pad: i32) -> i8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].substick_x
}

pub fn pad_sub_stick_y(pad: i32) -> i8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].substick_y
}

pub fn pad_stick_x(pad: i32) -> i8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].stick_x
}

pub fn pad_stick_y(pad: i32) -> i8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].stick_y
}

pub fn pad_trigger_l(pad: i32) -> u8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].trigger_l
}

pub fn pad_trigger_r(pad: i32) -> u8 {
    let state = pad_current_state();
    if !pad_chan_valid(pad) || state.keys[pad as usize].chan == -1 {
        return 0;
    }
    state.keys[pad as usize].trigger_r
}

pub fn _pad_disable_recalibration(disable: i32) -> u32 {
    // BUG FIX: C protects this shared low-memory read/modify/write from IRQs.
    let _interrupt_guard = ogc_sys::InterruptGuard::new();
    unsafe {
        let reg = LOWMEM_RECALIBRATION_DISABLE as *mut u8;
        let (prev, value) =
            update_recalibration_disable(core::ptr::read_volatile(reg), disable != 0);
        core::ptr::write_volatile(reg, value);
        prev as u32
    }
}

pub fn pad_debug_origin_byte(chan: i32, idx: usize) -> u8 {
    if !pad_chan_valid(chan) || idx >= 10 {
        return 0;
    }

    unsafe { (*pad_state_ptr()).origin[chan as usize][idx] }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    unsafe extern "C" fn sampling_callback_a() {}
    unsafe extern "C" fn sampling_callback_b() {}

    #[test]
    fn pad_status_layout_matches_c() {
        assert_eq!(core::mem::size_of::<PadStatus>(), 12);
        assert_eq!(core::mem::align_of::<PadStatus>(), 2);
    }

    #[test]
    fn sampling_callback_updates_after_failed_si_registration() {
        let mut current: SamplingCallback = Some(sampling_callback_a);
        let old = update_sampling_callback(
            &mut current,
            Some(sampling_callback_b),
            || 0,
            || panic!("registration must be attempted"),
        );

        let callback_a: unsafe extern "C" fn() = sampling_callback_a;
        let callback_b: unsafe extern "C" fn() = sampling_callback_b;
        assert!(old.is_some_and(|cb| core::ptr::fn_addr_eq(cb, callback_a)));
        assert!(current.is_some_and(|cb| core::ptr::fn_addr_eq(cb, callback_b)));
    }

    #[test]
    fn sampling_callback_clears_after_failed_si_removal() {
        let mut current: SamplingCallback = Some(sampling_callback_a);
        let old = update_sampling_callback(
            &mut current,
            None,
            || panic!("unregistration must be attempted"),
            || 0,
        );

        let callback_a: unsafe extern "C" fn() = sampling_callback_a;
        assert!(old.is_some_and(|cb| core::ptr::fn_addr_eq(cb, callback_a)));
        assert!(current.is_none());
    }

    #[test]
    fn xpatch_uses_signed_origin_values() {
        assert!(origin_needs_xpatch(65));
        assert!(!origin_needs_xpatch(64));
        assert!(!origin_needs_xpatch((-1i8) as u8));
        assert!(!origin_needs_xpatch(i8::MIN as u8));
    }

    #[test]
    fn recalibration_disable_update_preserves_other_low_memory_bits() {
        assert_eq!(update_recalibration_disable(0xa5, true), (false, 0xe5));
        assert_eq!(update_recalibration_disable(0xe5, false), (true, 0xa5));
    }

    #[test]
    fn recalibration_prepares_only_eligible_channels() {
        let mut state = PadState::new();
        state.pending_bits = PAD_CHAN1_BIT;
        state.waiting_bits = PAD_CHAN2_BIT;
        state.checking_bits = PAD_CHAN3_BIT;
        state.enable_bits = PAD_CHAN0_BIT | PAD_CHAN1_BIT | PAD_CHAN2_BIT | PAD_CHAN3_BIT;

        let preparation = pad_prepare_reset_inner(
            &mut state,
            PAD_CHAN0_BIT | PAD_CHAN2_BIT | PAD_CHAN3_BIT,
            true,
        );

        let expected = PAD_CHAN0_BIT | PAD_CHAN1_BIT;
        assert_eq!(state.pending_bits, 0);
        assert_eq!(state.resetting_bits, expected);
        assert_eq!(state.recalibrate_bits, expected);
        assert_eq!(state.enable_bits, PAD_CHAN2_BIT | PAD_CHAN3_BIT);
        assert_eq!(preparation.disable_bits, expected);
        assert!(preparation.start_reset);
    }

    #[test]
    fn disabled_recalibration_still_prepares_a_reset() {
        let mut state = PadState::new();
        state.spec = 4;
        state.enable_bits = PAD_CHAN0_BIT;

        let preparation = pad_prepare_reset_inner(&mut state, PAD_CHAN0_BIT, false);

        assert_eq!(state.resetting_bits, PAD_CHAN0_BIT);
        assert_eq!(state.recalibrate_bits, 0);
        assert_eq!(state.enable_bits, 0);
        assert_eq!(preparation.disable_bits, PAD_CHAN0_BIT);
        assert!(preparation.start_reset);
    }

    #[test]
    fn recalibration_queues_behind_an_active_reset() {
        let mut state = PadState::new();
        state.resetting_chan = PAD_CHAN0;

        let preparation = pad_prepare_reset_inner(&mut state, PAD_CHAN1_BIT, true);

        assert_eq!(state.resetting_bits, PAD_CHAN1_BIT);
        assert_eq!(state.recalibrate_bits, PAD_CHAN1_BIT);
        assert!(!preparation.start_reset);
    }

    #[test]
    fn reset_preparation_filters_reserved_mask_bits() {
        let mut state = PadState::new();
        state.pending_bits = PAD_CHAN1_BIT | 0x0000_0040;
        state.enable_bits = PAD_VALID_MASK;

        let preparation = pad_prepare_reset_inner(&mut state, PAD_CHAN3_BIT | 0x0fff_ffff, true);

        assert_eq!(state.resetting_bits, PAD_CHAN1_BIT | PAD_CHAN3_BIT);
        assert_eq!(state.recalibrate_bits, PAD_CHAN1_BIT | PAD_CHAN3_BIT);
        assert_eq!(state.pending_bits, 0);
        assert_eq!(preparation.disable_bits, PAD_CHAN1_BIT | PAD_CHAN3_BIT);
    }

    #[test]
    fn zero_mask_still_services_valid_pending_channels() {
        let mut state = PadState::new();
        state.pending_bits = PAD_CHAN2_BIT;

        let preparation = pad_prepare_reset_inner(&mut state, 0, false);

        assert_eq!(state.resetting_bits, PAD_CHAN2_BIT);
        assert!(preparation.start_reset);
    }

    unsafe fn trace_test_hook(_event: PadDebugEvent) {}

    #[test]
    fn trace_ring_drains_in_sequence_without_drops() {
        let _ = set_trace_hook(Some(trace_test_hook));
        pad_trace(PAD_TRACE_REQUEST_RESET, -1, 1, 2, 3, 4);
        pad_trace(PAD_TRACE_SYNC, -1, 5, 6, 7, 8);
        let mut events = [PadDebugEvent::default(); 2];
        assert_eq!(pad_debug_drain(&mut events), 2);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[0].kind, PAD_TRACE_REQUEST_RESET);
        assert_eq!(events[1].sequence, 1);
        assert_eq!(events[1].kind, PAD_TRACE_SYNC);
        assert_eq!(pad_debug_counters().dropped, 0);
        let _ = set_trace_hook(None);
    }
}
