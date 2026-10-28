extern crate alloc;

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use ogc_bt_sync::{self as bt};
use ogc_conf as conf;
use ogc_ipc::poll as ipc_poll;
use ogc_lwp_driver as lwp;
use wiiuse_rs::{
    AckCompletion, BalanceBoardCalibration, CommandEnqueue, CommandTag, ExpansionSetup,
    ExpansionSetupAction, HandshakeAction, InitialHandshake, InitialHandshakeError,
    NunchukCalibration, RemoteStatusReport, WII_COMMAND_MAX_BYTES, WiiCommandQueue,
    encode_wii_read_command, encode_wii_write_command,
};

use crate::{
    Accel, Vec3w, WPAD_CHAN_ALL, WPAD_CONNECTED, WPAD_DATA_EXPANSION, WPAD_DATA_STATE,
    WPAD_ERR_BAD_CHANNEL, WPAD_ERR_BADCONF, WPAD_ERR_BADVALUE, WPAD_ERR_NO_CONTROLLER,
    WPAD_ERR_NONE, WPAD_ERR_NOT_READY, WPAD_ERR_TRANSFER, WPAD_EXP_NONE, WPAD_EXP_UNKNOWN,
    WPAD_FMT_BTNS, WPAD_FMT_BTNS_ACC_IR, WPAD_FORMAT, WPAD_GLOBAL_STATUS, WPAD_IDLE_ACCEL,
    WPAD_IDLE_BTNS, WPAD_IDLE_IR, WPAD_IDLE_JS, WPAD_IDLE_MP, WPAD_IDLE_WB, WPAD_MAX_DEVICES,
    WPAD_RUMBLE, WPAD_SPEAKER, WPAD_STATE_DISABLED, WPAD_STATE_ENABLED, WPAD_STREAM_LEN,
    WPAD_THRESH_ANY, WPAD_THRESH_IGNORE, WpadData, push_event, reset_data, set_connected,
    valid_chan,
    wii_remote::{
        ReadAssembly, ReadFragmentResult, apply_read_report_buttons, decode_ack_report,
        decode_expansion_calibration, ir_aspect_vres, ir_mode_for_expansion, ir_position_offset,
        ir_sensitivity_blocks, report_type_for_format, shape_wiimote_input_report,
        wiimote_calibration_or_default,
    },
};

static mut BT_HOST: MaybeUninit<bt::WiiHidHost> = MaybeUninit::uninit();
static DISCONNECT_REQUESTS: AtomicU32 = AtomicU32::new(0);
static mut HID_INPUT: [crate::BluetoothHidState; bt::HID_SLOTS] =
    [crate::BluetoothHidState::EMPTY; bt::HID_SLOTS];
struct HidInputQueueCell(UnsafeCell<bt::BluetoothInputQueue>);
// Access is serialized by ServiceGuard in with_bt_input_queue.
unsafe impl Sync for HidInputQueueCell {}
static BT_INPUT_QUEUE: HidInputQueueCell =
    HidInputQueueCell(UnsafeCell::new(bt::BluetoothInputQueue::new()));
static BT_HOST_READY: AtomicBool = AtomicBool::new(false);
static BT_WORKER_STARTED: AtomicBool = AtomicBool::new(false);
struct GuestConfigCell(UnsafeCell<MaybeUninit<conf::ConfPadGuests>>);
unsafe impl Sync for GuestConfigCell {}
static GUEST_CONFIG: GuestConfigCell = GuestConfigCell(UnsafeCell::new(MaybeUninit::uninit()));
static GUEST_CONFIG_READY: AtomicBool = AtomicBool::new(false);
const BT_WORKER_STACK_BYTES: usize = 32 * 1024;
const DISCONNECT_WAIT_POLLS: usize = 3_000;
const DISCONNECT_POLL_TICKS: u64 = lwp::TICKS_PER_SECOND / 20_000;
#[repr(align(32))]
struct BluetoothWorkerStack([u8; BT_WORKER_STACK_BYTES]);
static mut BT_WORKER_STACK: BluetoothWorkerStack = BluetoothWorkerStack([0; BT_WORKER_STACK_BYTES]);
static mut BT_WORKER_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;

// Keep the USB read endpoints active independently of rendering. A frame-based
// poll cannot consume the Remote's 200 Hz stream and can lose status replies
// behind queued input reports. All host access uses the same critical section;
// only the application thread consumes events and updates WPAD slot state.
unsafe extern "C" fn bluetooth_worker(_argument: *mut c_void) -> *mut c_void {
    loop {
        ipc_poll();
        if with_host(|host| host.poll(now_ms())).is_none() {
            WPAD_GLOBAL_STATUS.store(WPAD_STATE_DISABLED, Ordering::Release);
        }
        lwp::sleep_ticks((lwp::TICKS_PER_SECOND / 1000).max(1));
    }
}

fn start_bluetooth_worker() -> Result<(), i32> {
    let _guard = ServiceGuard(lwp::critical_enter());
    if BT_WORKER_STARTED.load(Ordering::Relaxed) {
        return Ok(());
    }
    // The thread and its stack live for the application's lifetime. After
    // adapter shutdown the worker services close completion, then remains idle
    // until init starts the same host again. Reinitialization creates no thread.
    let result = unsafe {
        lwp::create_thread(
            addr_of_mut!(BT_WORKER_THREAD),
            Some(bluetooth_worker),
            core::ptr::null_mut(),
            addr_of_mut!(BT_WORKER_STACK.0).cast(),
            BT_WORKER_STACK_BYTES as u32,
            80,
        )
    };
    if result != lwp::LWP_SUCCESSFUL {
        return Err(result);
    }
    BT_WORKER_STARTED.store(true, Ordering::Release);
    Ok(())
}
static BT_POLL_HOOK: AtomicUsize = AtomicUsize::new(0);
const BT_GUEST_SLOTS: usize = 4;

fn reverse_bytes(bytes: [u8; 6]) -> [u8; 6] {
    [bytes[5], bytes[4], bytes[3], bytes[2], bytes[1], bytes[0]]
}

fn reverse_key(key: [u8; 16]) -> [u8; 16] {
    let mut reversed = key;
    reversed.reverse();
    reversed
}

fn guest_at(guests: &conf::ConfPadGuests, index: usize) -> conf::ConfPadGuestDevice {
    if index < 4 {
        guests.guests[index]
    } else {
        guests.unknown[index - 4]
    }
}

fn guest_at_mut(guests: &mut conf::ConfPadGuests, index: usize) -> &mut conf::ConfPadGuestDevice {
    if index < 4 {
        &mut guests.guests[index]
    } else {
        &mut guests.unknown[index - 4]
    }
}

fn load_paired_keys() -> Result<conf::ConfPadGuests, i32> {
    let init = conf::init();
    if init < 0 && init != conf::CONF_ENOTINIT {
        return Err(init);
    }
    let mut guests = conf::ConfPadGuests::default();
    let result = conf::get_pad_guest_devices(&mut guests);
    if result < 0 {
        return Err(result);
    }
    Ok(guests)
}

fn store_volatile_guests(guests: conf::ConfPadGuests) {
    let _guard = ServiceGuard(lwp::critical_enter());
    unsafe {
        (*GUEST_CONFIG.0.get()).write(guests);
    }
    GUEST_CONFIG_READY.store(true, Ordering::Release);
}

fn with_volatile_guests<T>(f: impl FnOnce(&mut conf::ConfPadGuests) -> T) -> Result<T, i32> {
    let _guard = ServiceGuard(lwp::critical_enter());
    if !GUEST_CONFIG_READY.load(Ordering::Acquire) {
        return Err(WPAD_ERR_NOT_READY as i32);
    }
    // The state is initialized before adapter startup and is serialized by the
    // same critical section as WPAD host operations.
    let guests = unsafe { (*GUEST_CONFIG.0.get()).assume_init_mut() };
    Ok(f(guests))
}

fn record_guest_pairing(address: bt::BluetoothAddress, key: [u8; 16]) -> Result<(), i32> {
    let config_address = reverse_bytes(address.bytes);
    with_volatile_guests(|guests| {
        if let Some(index) =
            (0..BT_GUEST_SLOTS).find(|&index| guest_at(guests, index).bdaddr == config_address)
        {
            let mut guest = guest_at(guests, index);
            guest.link_key = reverse_key(key);
            *guest_at_mut(guests, index) = guest;
        } else {
            for index in (1..BT_GUEST_SLOTS).rev() {
                *guest_at_mut(guests, index) = guest_at(guests, index - 1);
            }
            *guest_at_mut(guests, 0) = conf::ConfPadGuestDevice {
                bdaddr: config_address,
                link_key: reverse_key(key),
                ..conf::ConfPadGuestDevice::default()
            };
            guests.num_guests = guests.num_guests.saturating_add(1).min(4);
        }
    })
}

pub fn set_bluetooth_poll_hook(hook: Option<fn(&'static str)>) {
    BT_POLL_HOOK.store(hook.map_or(0, |hook| hook as usize), Ordering::Release);
}
fn bluetooth_poll_mark(stage: &'static str) {
    let hook = BT_POLL_HOOK.load(Ordering::Acquire);
    if hook != 0 {
        // Only the setter above publishes a function pointer with this signature.
        let hook: fn(&'static str) = unsafe { core::mem::transmute(hook) };
        hook(stage);
    }
}
static mut SLOTS: [ControllerSlot; MAX_POWERPC_SLOTS] =
    [const { ControllerSlot::new() }; MAX_POWERPC_SLOTS];

#[derive(Copy, Clone)]
struct ActivitySnapshot {
    buttons: u32,
    ir_x: f32,
    ir_y: f32,
    accel: Vec3w,
    expansion: i32,
    joystick: [u8; 4],
    board: [f32; 2],
    motion_plus: [i16; 3],
}

impl ActivitySnapshot {
    const fn new() -> Self {
        Self {
            buttons: 0,
            ir_x: 0.0,
            ir_y: 0.0,
            accel: Vec3w { x: 0, y: 0, z: 0 },
            expansion: WPAD_EXP_NONE as i32,
            joystick: [0; 4],
            board: [0.0; 2],
            motion_plus: [0; 3],
        }
    }
}

static mut LAST_ACTIVITY: [ActivitySnapshot; MAX_POWERPC_SLOTS] =
    [const { ActivitySnapshot::new() }; MAX_POWERPC_SLOTS];

const WM_CMD_RUMBLE: u8 = 0x10;
const WM_CMD_LED: u8 = 0x11;
const WM_CMD_REPORT_TYPE: u8 = 0x12;
const WM_CMD_IR: u8 = 0x13;
const WM_CMD_SPEAKER_ENABLE: u8 = 0x14;
const WM_CMD_CTRL_STATUS: u8 = 0x15;
const WM_CMD_READ_DATA: u8 = 0x17;
const WM_CMD_STREAM_DATA: u8 = 0x18;
const WM_CMD_SPEAKER_MUTE: u8 = 0x19;
const WM_CMD_IR_2: u8 = 0x1a;
const WM_REG_IR: u32 = 0x04b0_0030;
const WM_REG_IR_BLOCK1: u32 = 0x04b0_0000;
const WM_REG_IR_BLOCK2: u32 = 0x04b0_001a;
const WM_REG_IR_MODENUM: u32 = 0x04b0_0033;
const WM_EXP_MOTION_PLUS_ENABLE: u32 = 0x04a6_00f0;
const WM_EXP_MOTION_PLUS_MODE: u32 = 0x04a6_00fe;
const WM_MEM_OFFSET_CALIBRATION: u32 = 0x0000_0016;
const WM_EXP_ID: u32 = 0x04a4_00fa;
const WM_EXP_MEM_ENABLE1: u32 = 0x04a4_00f0;
const WM_EXP_MEM_ENABLE2: u32 = 0x04a4_00fb;
const WM_EXP_MEM_CALIBR: u32 = 0x04a4_0020;
const WM_REG_SPEAKER_REG1: u32 = 0x04a2_0001;
const WM_REG_SPEAKER_REG2: u32 = 0x04a2_0008;
const WM_REG_SPEAKER_REG3: u32 = 0x04a2_0009;
const WM_REG_SPEAKER_BLOCK: u32 = 0x04a2_0001;
const IDLE_TIMER_KEY_BASE: usize = 0x5750_4144;
const STREAM_TIMER_KEY_BASE: usize = 0x5750_4150;
const STREAM_DATA_BYTES: usize = 20;
const STREAM_INTERVAL_NS: u64 = 6_666_667;
const STREAM_SLOT_BITS: usize = 3;
const STREAM_SLOT_MASK: usize = (1 << STREAM_SLOT_BITS) - 1;
const STREAM_GENERATION_MASK: u32 = (1 << (usize::BITS as usize - STREAM_SLOT_BITS)) - 1;

const MAX_POWERPC_SLOTS: usize = WPAD_MAX_DEVICES;
static IR_RESOLUTION_SET: [AtomicBool; MAX_POWERPC_SLOTS] =
    [const { AtomicBool::new(false) }; MAX_POWERPC_SLOTS];
static IR_RESOLUTION_X: [AtomicU32; MAX_POWERPC_SLOTS] =
    [const { AtomicU32::new(0) }; MAX_POWERPC_SLOTS];
static IR_RESOLUTION_Y: [AtomicU32; MAX_POWERPC_SLOTS] =
    [const { AtomicU32::new(0) }; MAX_POWERPC_SLOTS];
static IDLE_TIMEOUT_SECONDS: AtomicU32 = AtomicU32::new(0);

#[derive(Copy, Clone, Eq, PartialEq)]
enum ControllerKind {
    Unknown,
    Generic,
    Switch,
    WiiRemote,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum PendingReadKind {
    None,
    InitialExpansionId,
    WiiRemoteCalibration,
    ExpansionCalibration,
}

fn complete_initial_expansion_id(
    slot: &mut ControllerSlot,
    id: u32,
) -> Result<HandshakeAction, InitialHandshakeError> {
    let action = slot.initial_handshake.expansion_id(id)?;
    slot.wii_u_pro = action == HandshakeAction::RefreshStatus;
    if slot.wii_u_pro {
        // C zeroes the expansion-ID buffer and decodes that buffer as calibration.
        slot.accel_calib.cal_zero = Vec3w::default();
        slot.accel_calib.cal_g = Vec3w::default();
    }
    Ok(action)
}

fn request_initial_handshake_action(slot_idx: usize, action: HandshakeAction) -> bool {
    let sent = match action {
        HandshakeAction::ReadExpansionId => request_initial_expansion_id(slot_idx),
        HandshakeAction::ReadCalibration => request_wii_calibration(slot_idx),
        HandshakeAction::RefreshStatus => request_wii_status(slot_idx),
    };
    if sent {
        return true;
    }
    if action == HandshakeAction::RefreshStatus {
        return false;
    }
    let Some(()) = with_slot_mut(slot_idx, |slot| slot.initial_handshake.retry()) else {
        return false;
    };
    request_wii_status(slot_idx)
}

#[derive(Copy, Clone)]
struct ControllerSlot {
    active: bool,
    search_disconnect_pending: bool,
    kind: ControllerKind,
    accel_calib: Accel,
    read: ReadAssembly,
    pending_read: PendingReadKind,
    command_queue: WiiCommandQueue,
    expansion_setup: ExpansionSetup,
    wii_initialized: bool,
    initial_handshake: InitialHandshake,
    wii_u_pro: bool,
    expansion_handshake_active: bool,
    closed_hid_channels: u8,
    attachment_present: bool,
    ir_enabled: bool,
}

#[derive(Copy, Clone)]
struct StreamState {
    data: *const u8,
    len: usize,
    offset: usize,
    generation: u32,
}

impl StreamState {
    const EMPTY: Self = Self {
        data: core::ptr::null(),
        len: 0,
        offset: 0,
        generation: 0,
    };
}

static mut STREAMS: [StreamState; MAX_POWERPC_SLOTS] = [StreamState::EMPTY; MAX_POWERPC_SLOTS];

const fn encode_stream_timer_token(slot_idx: usize, generation: u32) -> usize {
    ((generation as usize) << STREAM_SLOT_BITS) | slot_idx
}

const fn decode_stream_timer_token(token: usize) -> (usize, u32) {
    (token & STREAM_SLOT_MASK, (token >> STREAM_SLOT_BITS) as u32)
}

impl ControllerSlot {
    const fn new() -> Self {
        Self {
            active: false,
            search_disconnect_pending: false,
            kind: ControllerKind::Unknown,
            accel_calib: Accel {
                cal_zero: Vec3w {
                    x: 499,
                    y: 509,
                    z: 507,
                },
                cal_g: Vec3w {
                    x: 703,
                    y: 709,
                    z: 709,
                },
                st_roll: 0.0,
                st_pitch: 0.0,
                st_alpha: 0.3,
            },
            read: ReadAssembly::new(0, 0),
            pending_read: PendingReadKind::None,
            command_queue: WiiCommandQueue::new(),
            expansion_setup: ExpansionSetup::new(),
            wii_initialized: false,
            initial_handshake: InitialHandshake::new(),
            wii_u_pro: false,
            expansion_handshake_active: false,
            closed_hid_channels: 0,
            attachment_present: false,
            ir_enabled: false,
        }
    }

    fn is_wii_remote(&self) -> bool {
        self.active && self.kind == ControllerKind::WiiRemote && !self.search_disconnect_pending
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct WencStatus {
    predictor: i32,
    step_index: i16,
    step: i32,
    prev_sample: i32,
    sample1: i16,
    sample2: i16,
    coeff1: i32,
    coeff2: i32,
    idelta: i32,
}

const WENC_DIFF_LOOKUP: [i32; 16] = [1, 3, 5, 7, 9, 11, 13, 15, -1, -3, -5, -7, -9, -11, -13, -15];

const WENC_INDEX_SCALE: [i32; 16] = [
    230, 230, 230, 230, 307, 409, 512, 614, 230, 230, 230, 230, 307, 409, 512, 614,
];

#[inline]
fn wenc_clip_short(value: i32) -> i16 {
    if (value + 32768) & !65535 != 0 {
        ((value >> 31) ^ 32767) as i16
    } else {
        value as i16
    }
}

#[inline]
fn wenc_clip(value: i32, min: i32, max: i32) -> i32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn wencdata(info: &mut WencStatus, sample: i16) -> u8 {
    if info.step == 0 {
        info.predictor = 0;
        info.step = 127;
    }

    let delta = sample as i32 - info.predictor;
    let magnitude = ((delta.abs() * 4) / info.step).min(7);
    let nibble = (magnitude + if delta < 0 { 8 } else { 0 }) as usize;

    info.predictor += (info.step * WENC_DIFF_LOOKUP[nibble]) / 8;
    info.predictor = i32::from(wenc_clip_short(info.predictor));
    info.step = (info.step * WENC_INDEX_SCALE[nibble]) >> 8;
    info.step = wenc_clip(info.step, 127, 24_576);
    nibble as u8
}

fn with_wenc_status(info: *mut crate::WpadEncStatus, f: impl FnOnce(&mut WencStatus)) {
    if info.is_null() {
        return;
    }
    let ptr = info.cast::<WencStatus>();
    let mut status = unsafe { core::ptr::read_unaligned(ptr) };
    f(&mut status);
    unsafe {
        core::ptr::write_unaligned(ptr, status);
    }
}

const SENSOR_BAR_CTRL_REG: *mut u32 = 0xcd00_00c0 as *mut u32;

unsafe fn sensorbar_enable(enable: bool) {
    unsafe {
        let mut value = read_volatile(SENSOR_BAR_CTRL_REG);
        value &= !0x100;
        if enable {
            value |= 0x100;
        }
        write_volatile(SENSOR_BAR_CTRL_REG, value);
    }
}

fn with_slot<T>(slot_idx: usize, f: impl FnOnce(&ControllerSlot) -> T) -> Option<T> {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return None;
    }
    let slots = core::ptr::addr_of!(SLOTS).cast::<ControllerSlot>();
    Some(unsafe { f(&*slots.add(slot_idx)) })
}

fn with_slot_mut<T>(slot_idx: usize, f: impl FnOnce(&mut ControllerSlot) -> T) -> Option<T> {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return None;
    }
    let slots = core::ptr::addr_of_mut!(SLOTS).cast::<ControllerSlot>();
    Some(unsafe { f(&mut *slots.add(slot_idx)) })
}

fn slot_is_wii_remote(slot_idx: usize) -> bool {
    with_slot(slot_idx, ControllerSlot::is_wii_remote).unwrap_or(false)
}

fn slot_is_search_disconnect_pending(slot_idx: usize) -> bool {
    with_slot(slot_idx, |slot| slot.search_disconnect_pending).unwrap_or(false)
}

fn should_accept_connected_event(slot: &ControllerSlot) -> bool {
    !slot.search_disconnect_pending
}

fn mark_search_disconnect(slot: &mut ControllerSlot) -> bool {
    if slot.active && slot.kind == ControllerKind::WiiRemote {
        slot.search_disconnect_pending = true;
        true
    } else {
        false
    }
}

fn close_hid_channel(closed: &mut u8, channel: bt::HidChannel) -> bool {
    let bit = match channel {
        bt::HidChannel::Control => 1,
        bt::HidChannel::Interrupt => 2,
    };
    *closed |= bit;
    *closed == 3
}

const fn probe_status(status: i32, connected: bool, handshake_complete: bool) -> i32 {
    if status == WPAD_STATE_DISABLED {
        WPAD_ERR_NOT_READY as i32
    } else if !connected {
        WPAD_ERR_NO_CONTROLLER as i32
    } else if !handshake_complete {
        WPAD_ERR_NOT_READY as i32
    } else {
        WPAD_ERR_NONE as i32
    }
}

fn send_wii_raw_command(slot_idx: usize, raw: &mut [u8]) -> bool {
    send_wii_raw_command_tagged(slot_idx, raw, CommandTag::General)
}

fn send_wii_raw_command_tagged(slot_idx: usize, raw: &mut [u8], tag: CommandTag) -> bool {
    if !slot_is_wii_remote(slot_idx) {
        return false;
    }
    let Some(enqueue) = with_slot_mut(slot_idx, |slot| slot.command_queue.enqueue_tagged(raw, tag))
    else {
        return false;
    };
    match enqueue {
        Ok(CommandEnqueue::Queued) => true,
        Ok(CommandEnqueue::Started) => send_active_wii_command(slot_idx),
        Err(_) => false,
    }
}

fn send_active_wii_command(slot_idx: usize) -> bool {
    let Some(command) = with_slot(slot_idx, |slot| slot.command_queue.active().copied()) else {
        return false;
    };
    let Some(command) = command else {
        return true;
    };
    let mut bytes = [0u8; WII_COMMAND_MAX_BYTES];
    let command_bytes = command.bytes();
    bytes[..command_bytes.len()].copy_from_slice(command_bytes);
    if command_bytes.len() > 1 && WPAD_RUMBLE[slot_idx].load(Ordering::Acquire) {
        bytes[1] |= 0x01;
    }
    if with_host(|host| host.send_report(slot_idx, &bytes[..command_bytes.len()])).unwrap_or(false)
    {
        return true;
    }
    if !abort_wii_command_queue(slot_idx) {
        return false;
    }
    false
}

fn abort_wii_command_queue(slot_idx: usize) -> bool {
    with_slot_mut(slot_idx, |slot| {
        let expansion_was_pending = slot.expansion_handshake_active;
        slot.command_queue.clear();
        slot.pending_read = PendingReadKind::None;
        slot.expansion_handshake_active = false;
        slot.expansion_setup.complete();
        if !slot.initial_handshake.is_complete() {
            slot.initial_handshake.retry();
        }
        if expansion_was_pending {
            unsafe {
                WPAD_DATA_STATE[slot_idx].exp.type_ = WPAD_EXP_NONE as i32;
                WPAD_DATA_STATE[slot_idx].data_present &= !WPAD_DATA_EXPANSION;
            }
        }
    })
    .is_some()
}

fn complete_wii_response(slot_idx: usize, command: u8) -> bool {
    let Some(completion) = with_slot_mut(slot_idx, |slot| {
        if slot.command_queue.active().map(|active| active.command()) != Some(command) {
            return Ok(None);
        }
        slot.command_queue.complete_response(command)
    }) else {
        return false;
    };
    match completion {
        Err(_) => false,
        Ok(None) => true,
        Ok(Some(_)) => send_active_wii_command(slot_idx),
    }
}

fn complete_wii_ack(slot_idx: usize, ack: wiiuse_rs::AckReport) -> bool {
    let status = [WM_CMD_CTRL_STATUS, 0];
    let Some(completion) = with_slot_mut(slot_idx, |slot| {
        slot.command_queue.complete_ack_report(ack, &status)
    }) else {
        return false;
    };
    match completion {
        Ok(AckCompletion::Ignored) => true,
        Err(_) => {
            if !abort_wii_command_queue(slot_idx) {
                return false;
            }
            false
        }
        Ok(AckCompletion::Completed {
            completed, next, ..
        }) => {
            let action = if matches!(
                completed.tag(),
                CommandTag::ExpansionEnableOne | CommandTag::ExpansionEnableTwo
            ) {
                with_slot_mut(slot_idx, |slot| slot.expansion_setup.acknowledge(ack)).flatten()
            } else {
                None
            };
            if let Some(action) = action {
                if !submit_expansion_action(slot_idx, action) {
                    if !abort_wii_command_queue(slot_idx) {
                        return false;
                    }
                    return false;
                }
            }
            if next.is_some() {
                send_active_wii_command(slot_idx)
            } else {
                true
            }
        }
    }
}

fn send_wii_simple_command(slot_idx: usize, cmd: u8, params: &[u8], set_report_flag: bool) -> bool {
    let mut raw = [0u8; 4];
    raw[0] = cmd;
    let len = 1 + params.len();
    if len > raw.len() {
        return false;
    }
    raw[1..len].copy_from_slice(params);
    if set_report_flag && len > 1 {
        raw[1] |= 0x02;
    }
    send_wii_raw_command(slot_idx, &mut raw[..len])
}

fn encode_wii_write(addr: u32, data: &[u8]) -> Option<[u8; 22]> {
    encode_wii_write_command(addr, data)
}

fn send_wii_write(slot_idx: usize, addr: u32, data: &[u8]) -> bool {
    send_wii_write_tagged(slot_idx, addr, data, CommandTag::General)
}

fn send_wii_write_tagged(slot_idx: usize, addr: u32, data: &[u8], tag: CommandTag) -> bool {
    let Some(mut raw) = encode_wii_write(addr, data) else {
        return false;
    };
    send_wii_raw_command_tagged(slot_idx, &mut raw, tag)
}

fn send_wii_read(slot_idx: usize, addr: u32, len: u16) -> bool {
    let Some(mut raw) = encode_wii_read_command(addr, len) else {
        return false;
    };
    send_wii_raw_command(slot_idx, &mut raw)
}

fn request_wii_status(slot_idx: usize) -> bool {
    send_wii_simple_command(slot_idx, WM_CMD_CTRL_STATUS, &[0x00], false)
}

fn idle_timer_key(slot_idx: usize) -> usize {
    IDLE_TIMER_KEY_BASE + slot_idx
}

fn cancel_idle_timer(slot_idx: usize) {
    lwp::timer_cancel(idle_timer_key(slot_idx));
}

unsafe fn idle_timeout_cb(arg: *mut c_void) {
    let slot_idx = arg as usize;
    if slot_idx < MAX_POWERPC_SLOTS {
        DISCONNECT_REQUESTS.fetch_or(1 << slot_idx, Ordering::Release);
    }
}

fn restart_idle_timer(slot_idx: usize) {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return;
    }
    let timeout = IDLE_TIMEOUT_SECONDS.load(Ordering::Acquire);
    if timeout == 0 || !slot_is_wii_remote(slot_idx) {
        cancel_idle_timer(slot_idx);
        return;
    }
    let delay = u64::from(timeout).saturating_mul(lwp::TICKS_PER_SECOND);
    // SAFETY: the callback interprets the argument only as the copied slot
    // index and never dereferences it.
    let _ = unsafe {
        lwp::timer_start(
            idle_timer_key(slot_idx),
            Some(idle_timeout_cb),
            slot_idx as *mut c_void,
            delay,
            0,
        )
    };
}

pub(crate) fn mark_activity(slot_idx: usize) {
    restart_idle_timer(slot_idx);
}

fn threshold_hit(delta: u32, threshold: i32) -> bool {
    threshold != WPAD_THRESH_IGNORE
        && (threshold == WPAD_THRESH_ANY || delta >= threshold.max(0) as u32)
}

fn threshold_hit_f32(delta: f32, threshold: i32) -> bool {
    threshold != WPAD_THRESH_IGNORE
        && (threshold == WPAD_THRESH_ANY || delta >= threshold.max(0) as f32)
}

fn expansion_activity(data: &WpadData) -> ([u8; 4], [f32; 2], [i16; 3]) {
    let mut joystick = [0; 4];
    let mut board = [0.0; 2];
    let mut motion_plus = [0; 3];
    unsafe {
        match data.exp.type_ {
            x if x == crate::WPAD_EXP_NUNCHUK as i32 => {
                let js = data.exp.u.nunchuk.js;
                joystick[..2].copy_from_slice(&[js.pos.x, js.pos.y]);
            }
            x if x == crate::WPAD_EXP_CLASSIC as i32 => {
                let classic = data.exp.u.classic;
                joystick.copy_from_slice(&[
                    classic.ljs.pos.x,
                    classic.ljs.pos.y,
                    classic.rjs.pos.x,
                    classic.rjs.pos.y,
                ]);
            }
            x if x == crate::WPAD_EXP_GUITARHERO3 as i32 => {
                let js = data.exp.u.gh3.js;
                joystick[..2].copy_from_slice(&[js.pos.x, js.pos.y]);
            }
            x if x == crate::WPAD_EXP_WIIBOARD as i32 => {
                let wb = data.exp.u.wb;
                board = [wb.x, wb.y];
            }
            x if x == crate::WPAD_EXP_UNKNOWN as i32 => {
                let mp = data.exp.u.mp;
                motion_plus = [mp.rx, mp.ry, mp.rz];
            }
            _ => {}
        }
    }
    (joystick, board, motion_plus)
}

pub(crate) fn mark_activity_for_data(slot_idx: usize, data: &WpadData) {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return;
    }

    let previous = unsafe { LAST_ACTIVITY[slot_idx] };
    let button_delta = (data.btns_h ^ previous.buttons).count_ones();
    let accel_delta = data
        .accel
        .x
        .abs_diff(previous.accel.x)
        .max(data.accel.y.abs_diff(previous.accel.y))
        .max(data.accel.z.abs_diff(previous.accel.z));
    let (joystick, board, motion_plus) = expansion_activity(data);
    let joystick_delta = joystick
        .iter()
        .zip(previous.joystick.iter())
        .map(|(current, old)| u32::from(current.abs_diff(*old)))
        .max()
        .unwrap_or(0);
    let board_delta = board
        .iter()
        .zip(previous.board.iter())
        .map(|(current, old)| (*current - *old).abs())
        .fold(0.0, f32::max);
    let motion_plus_delta = motion_plus
        .iter()
        .zip(previous.motion_plus.iter())
        .map(|(current, old)| u32::from(current.abs_diff(*old)))
        .max()
        .unwrap_or(0);
    let ir_delta = (data.ir.x - previous.ir_x)
        .abs()
        .max((data.ir.y - previous.ir_y).abs());
    let changed = threshold_hit(
        button_delta,
        WPAD_IDLE_BTNS[slot_idx].load(Ordering::Acquire),
    ) || threshold_hit_f32(ir_delta, WPAD_IDLE_IR[slot_idx].load(Ordering::Acquire))
        || threshold_hit(
            u32::from(accel_delta),
            WPAD_IDLE_ACCEL[slot_idx].load(Ordering::Acquire),
        )
        || threshold_hit(
            u32::from(data.exp.type_ != previous.expansion),
            WPAD_IDLE_MP[slot_idx].load(Ordering::Acquire),
        )
        || threshold_hit(
            joystick_delta,
            WPAD_IDLE_JS[slot_idx].load(Ordering::Acquire),
        )
        || threshold_hit_f32(board_delta, WPAD_IDLE_WB[slot_idx].load(Ordering::Acquire))
        || threshold_hit(
            motion_plus_delta,
            WPAD_IDLE_MP[slot_idx].load(Ordering::Acquire),
        )
        || data.data_present != 0 && previous.expansion == WPAD_EXP_NONE as i32;

    unsafe {
        LAST_ACTIVITY[slot_idx] = ActivitySnapshot {
            buttons: data.btns_h,
            ir_x: data.ir.x,
            ir_y: data.ir.y,
            accel: data.accel,
            expansion: data.exp.type_,
            joystick,
            board,
            motion_plus,
        };
    }

    if changed {
        mark_activity(slot_idx);
    }
}

pub(crate) fn reset_activity(slot_idx: usize) {
    if slot_idx < MAX_POWERPC_SLOTS {
        unsafe {
            LAST_ACTIVITY[slot_idx] = ActivitySnapshot::new();
        }
    }
}

fn speaker_volume() -> u8 {
    let vol = conf::get_pad_speaker_volume();
    if (0..=255).contains(&vol) {
        vol as u8
    } else {
        0x40
    }
}

fn speaker_config_bytes() -> [u8; 7] {
    [0x00, 0x00, 0xd0, 0x07, speaker_volume(), 0x0c, 0x0e]
}

fn send_wii_stream_data(slot_idx: usize, data: &[u8]) -> bool {
    let Some(mut raw) = encode_wii_stream_data(data) else {
        return false;
    };
    send_wii_raw_command(slot_idx, &mut raw)
}

fn encode_wii_stream_data(data: &[u8]) -> Option<[u8; 22]> {
    if data.is_empty() || data.len() > STREAM_DATA_BYTES {
        return None;
    }
    let mut raw = [0u8; 22];
    raw[0] = WM_CMD_STREAM_DATA;
    raw[1] = (data.len() as u8) << 3;
    raw[2..2 + data.len()].copy_from_slice(data);
    Some(raw)
}

fn copy_stream_chunk(data: &[u8], offset: usize) -> Option<[u8; STREAM_DATA_BYTES]> {
    let remaining = data.get(offset..)?;
    if remaining.is_empty() {
        return None;
    }
    let mut chunk = [0; STREAM_DATA_BYTES];
    let count = remaining.len().min(STREAM_DATA_BYTES);
    chunk[..count].copy_from_slice(&remaining[..count]);
    Some(chunk)
}

fn with_stream_state<T>(slot_idx: usize, f: impl FnOnce(&mut StreamState) -> T) -> Option<T> {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return None;
    }
    let _guard = ServiceGuard(lwp::critical_enter());
    let streams = core::ptr::addr_of_mut!(STREAMS).cast::<StreamState>();
    Some(unsafe { f(&mut *streams.add(slot_idx)) })
}

fn clear_stream(slot_idx: usize) {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return;
    }
    lwp::timer_cancel(STREAM_TIMER_KEY_BASE + slot_idx);
    let _ = with_stream_state(slot_idx, |stream| {
        let generation = stream.generation.wrapping_add(1) & STREAM_GENERATION_MASK;
        *stream = StreamState {
            generation,
            ..StreamState::EMPTY
        };
    });
    WPAD_STREAM_LEN[slot_idx].store(0, Ordering::Release);
}

unsafe fn stream_timer_callback(arg: *mut c_void) {
    let token = arg as usize;
    let (slot_idx, generation) = decode_stream_timer_token(token);
    let Some(result) = with_stream_state(slot_idx, |stream| {
        if stream.generation != generation {
            return None;
        }
        if stream.data.is_null() || stream.offset >= stream.len {
            return Some(None);
        }
        // SAFETY: the public unsafe WPAD stream API requires the caller to
        // keep the complete input buffer readable until all scheduled chunks
        // have been copied by this callback.
        let chunk = unsafe {
            let data = core::slice::from_raw_parts(stream.data, stream.len);
            copy_stream_chunk(data, stream.offset).unwrap_or([0; STREAM_DATA_BYTES])
        };
        stream.offset += STREAM_DATA_BYTES;
        Some(Some(chunk))
    }) else {
        return;
    };
    let Some(chunk) = result else {
        return;
    };
    let Some(chunk) = chunk else {
        clear_stream(slot_idx);
        return;
    };

    let _ = send_wii_stream_data(slot_idx, &chunk);
}

fn enable_speaker(slot_idx: usize) -> bool {
    let conf = speaker_config_bytes();
    send_wii_simple_command(slot_idx, WM_CMD_SPEAKER_MUTE, &[0x04], true)
        && send_wii_simple_command(slot_idx, WM_CMD_SPEAKER_ENABLE, &[0x04], true)
        && send_wii_write(slot_idx, WM_REG_SPEAKER_REG3, &[0x01])
        && send_wii_write(slot_idx, WM_REG_SPEAKER_REG1, &[0x08])
        && send_wii_write(slot_idx, WM_REG_SPEAKER_BLOCK, &conf)
        && send_wii_write(slot_idx, WM_REG_SPEAKER_REG2, &[0x01])
        && send_wii_simple_command(slot_idx, WM_CMD_SPEAKER_MUTE, &[0x00], true)
}

fn disable_speaker(slot_idx: usize) -> bool {
    send_wii_simple_command(slot_idx, WM_CMD_SPEAKER_MUTE, &[0x04], true)
        && send_wii_write(slot_idx, WM_REG_SPEAKER_REG1, &[0x01])
        && send_wii_write(slot_idx, WM_REG_SPEAKER_REG3, &[0x00])
        && send_wii_simple_command(slot_idx, WM_CMD_SPEAKER_ENABLE, &[0x00], true)
}

pub fn control_speaker(slot_idx: usize, enable: bool) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NO_CONTROLLER as i32;
    }
    let current = WPAD_SPEAKER[slot_idx].load(Ordering::Acquire);
    if enable == current {
        if !request_wii_status(slot_idx) {
            return WPAD_ERR_TRANSFER as i32;
        }
        mark_activity(slot_idx);
        return WPAD_ERR_NONE as i32;
    }
    let ok = if enable {
        enable_speaker(slot_idx)
    } else {
        disable_speaker(slot_idx)
    };
    if !ok {
        return WPAD_ERR_TRANSFER as i32;
    }
    WPAD_SPEAKER[slot_idx].store(enable, Ordering::Release);
    if !request_wii_status(slot_idx) {
        return WPAD_ERR_TRANSFER as i32;
    }
    mark_activity(slot_idx);
    WPAD_ERR_NONE as i32
}

pub fn control_speaker_if_present(slot_idx: usize, enable: bool) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NONE as i32;
    }
    control_speaker(slot_idx, enable)
}

pub fn rumble_if_present(slot_idx: usize, enabled: bool) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NONE as i32;
    }
    rumble(slot_idx, enabled)
}

pub fn rumble(slot_idx: usize, enabled: bool) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NO_CONTROLLER as i32;
    }

    let previous = WPAD_RUMBLE[slot_idx].swap(enabled, Ordering::AcqRel);
    let ok = send_wii_simple_command(slot_idx, WM_CMD_RUMBLE, &[u8::from(enabled)], false);
    if !ok {
        WPAD_RUMBLE[slot_idx].store(previous, Ordering::Release);
        return WPAD_ERR_TRANSFER as i32;
    }
    mark_activity(slot_idx);
    WPAD_ERR_NONE as i32
}

pub fn set_motion_plus(slot_idx: usize, enable: bool) -> i32 {
    let slot_state = with_slot(slot_idx, |slot| {
        (slot.active, slot.kind == ControllerKind::WiiRemote)
    })
    .unwrap_or((false, false));
    if let Some(result) = motion_plus_preflight(
        WPAD_GLOBAL_STATUS.load(Ordering::Acquire),
        slot_state.0,
        slot_state.1,
    ) {
        return result;
    }
    let ok = if enable {
        send_wii_write(slot_idx, WM_EXP_MOTION_PLUS_MODE, &[0x04])
            && send_wii_write(slot_idx, WM_EXP_MOTION_PLUS_ENABLE, &[0x55])
    } else {
        send_wii_write(slot_idx, WM_EXP_MEM_ENABLE1, &[0x55])
            && send_wii_write(slot_idx, WM_EXP_MEM_ENABLE1, &[0x00])
    };
    if !ok {
        return WPAD_ERR_TRANSFER as i32;
    }
    if with_slot_mut(slot_idx, |slot| {
        if enable {
            slot.expansion_handshake_active = false;
            slot.attachment_present = false;
            unsafe {
                WPAD_DATA_STATE[slot_idx].exp.type_ = WPAD_EXP_UNKNOWN as i32;
                WPAD_DATA_STATE[slot_idx].exp.u.mp = crate::MotionPlus {
                    rx: 0,
                    ry: 0,
                    rz: 0,
                    status: 1,
                    ext: 1,
                };
                WPAD_DATA_STATE[slot_idx].data_present |= WPAD_DATA_EXPANSION;
            }
        } else {
            slot.expansion_handshake_active = false;
            unsafe {
                WPAD_DATA_STATE[slot_idx].exp.type_ = WPAD_EXP_NONE as i32;
                WPAD_DATA_STATE[slot_idx].exp.u.mp = crate::MotionPlus::default();
                WPAD_DATA_STATE[slot_idx].data_present &= !WPAD_DATA_EXPANSION;
            }
        }
    })
    .is_none()
    {
        return WPAD_ERR_NOT_READY as i32;
    }
    if !request_wii_status(slot_idx) || !refresh_wii_remote_reporting(slot_idx) {
        return WPAD_ERR_TRANSFER as i32;
    }
    mark_activity(slot_idx);
    WPAD_ERR_NONE as i32
}

const fn motion_plus_preflight(
    status: i32,
    slot_active: bool,
    slot_is_wii_remote: bool,
) -> Option<i32> {
    if status == WPAD_STATE_DISABLED {
        Some(WPAD_ERR_NOT_READY as i32)
    } else if !slot_active || !slot_is_wii_remote {
        // C calls the void controller routine for any allocated slot, then returns success.
        Some(WPAD_ERR_NONE as i32)
    } else {
        None
    }
}

pub fn set_motion_plus_if_present(slot_idx: usize, enable: bool) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NONE as i32;
    }
    set_motion_plus(slot_idx, enable)
}

pub fn send_stream_data(slot_idx: usize, data: &[u8]) -> i32 {
    if !slot_is_wii_remote(slot_idx) {
        return WPAD_ERR_NO_CONTROLLER as i32;
    }
    if !WPAD_SPEAKER[slot_idx].load(Ordering::Acquire) {
        return WPAD_ERR_NOT_READY as i32;
    }
    if data.is_empty() {
        return WPAD_ERR_BADVALUE as i32;
    }

    lwp::timer_cancel(STREAM_TIMER_KEY_BASE + slot_idx);
    let generation = with_stream_state(slot_idx, |stream| {
        let generation = stream.generation.wrapping_add(1) & STREAM_GENERATION_MASK;
        *stream = StreamState {
            data: data.as_ptr(),
            len: data.len(),
            offset: 0,
            generation,
        };
        generation
    });
    let Some(generation) = generation else {
        return WPAD_ERR_NOT_READY as i32;
    };
    WPAD_STREAM_LEN[slot_idx].store(data.len() as u32, Ordering::Release);

    let period = lwp::TICKS_PER_SECOND.saturating_mul(STREAM_INTERVAL_NS) / 1_000_000_000;
    // SAFETY: the public unsafe WPAD stream API requires `data` to stay valid
    // until each scheduled callback has copied its next chunk.
    let rc = unsafe {
        lwp::timer_start(
            STREAM_TIMER_KEY_BASE + slot_idx,
            Some(stream_timer_callback),
            encode_stream_timer_token(slot_idx, generation) as *mut c_void,
            period,
            period,
        )
    };
    if rc != lwp::LWP_SUCCESSFUL {
        clear_stream(slot_idx);
        return WPAD_ERR_NOT_READY as i32;
    }
    mark_activity(slot_idx);
    WPAD_ERR_NONE as i32
}

pub fn set_idle_timeout(seconds: u32) {
    IDLE_TIMEOUT_SECONDS.store(seconds, Ordering::Release);
    for slot_idx in 0..MAX_POWERPC_SLOTS {
        if seconds == 0 {
            cancel_idle_timer(slot_idx);
        } else {
            restart_idle_timer(slot_idx);
        }
    }
}

pub fn pad_status(chan: i32) {
    if chan == WPAD_CHAN_ALL {
        for slot_idx in 0..MAX_POWERPC_SLOTS {
            if slot_is_wii_remote(slot_idx) && request_wii_status(slot_idx) {
                mark_activity(slot_idx);
            }
        }
        return;
    }
    let Some(slot_idx) = valid_chan(chan) else {
        return;
    };
    if slot_is_wii_remote(slot_idx) && request_wii_status(slot_idx) {
        mark_activity(slot_idx);
    }
}

pub fn wipe_saved_controllers() -> i32 {
    if conf::set_pad_devices(&conf::ConfPads::default()) < 0 {
        return WPAD_ERR_BADCONF as i32;
    }
    if conf::set_pad_guest_devices(Some(&conf::ConfPadGuests::default())) < 0 {
        return WPAD_ERR_BADCONF as i32;
    }
    if conf::save_changes() < 0 {
        return WPAD_ERR_BADCONF as i32;
    }
    WPAD_ERR_NONE as i32
}

pub fn encode_data(
    info: *mut crate::WpadEncStatus,
    flag: u32,
    pcm_samples: *const i16,
    num_samples: i32,
    enc_data: *mut u8,
) {
    if info.is_null() || enc_data.is_null() || pcm_samples.is_null() || num_samples <= 0 {
        return;
    }
    let samples = unsafe { core::slice::from_raw_parts(pcm_samples, num_samples as usize) };
    let pair_count = samples.len().div_ceil(2);
    with_wenc_status(info, |status| {
        if flag & crate::WPAD_ENC_CONT == 0 {
            status.step = 0;
        }
        for index in 0..pair_count {
            let left = samples.get(index * 2).copied().unwrap_or(0);
            let right = samples.get(index * 2 + 1).copied().unwrap_or(0);
            let hi = wencdata(status, left);
            let lo = wencdata(status, right);
            unsafe {
                *enc_data.add(index) = (hi << 4) | lo;
            }
        }
    });
}

fn disconnect_slot(slot_idx: usize) -> i32 {
    if slot_idx >= MAX_POWERPC_SLOTS {
        return WPAD_ERR_BAD_CHANNEL as i32;
    }
    if WPAD_GLOBAL_STATUS.load(Ordering::Acquire) != WPAD_STATE_ENABLED {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(active) = with_slot(slot_idx, |slot| slot.active) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if !active {
        return WPAD_ERR_NONE as i32;
    }
    let Some(connected) = with_host(|host| host.has_link(slot_idx)) else {
        return WPAD_ERR_NOT_READY as i32;
    };
    if !connected {
        return WPAD_ERR_NONE as i32;
    }
    let Some(result) = with_host(|host| host.request_disconnect(slot_idx)) else {
        return WPAD_ERR_NOT_READY as i32;
    };
    if result.is_err() {
        return WPAD_ERR_TRANSFER as i32;
    }
    for _ in 0..DISCONNECT_WAIT_POLLS {
        match with_host(|host| host.has_link(slot_idx)) {
            Some(false) => return WPAD_ERR_NONE as i32,
            Some(true) => lwp::sleep_ticks(DISCONNECT_POLL_TICKS),
            None => return WPAD_ERR_NOT_READY as i32,
        }
    }
    match with_host(|host| host.has_link(slot_idx)) {
        Some(false) => WPAD_ERR_NONE as i32,
        Some(true) => WPAD_ERR_TRANSFER as i32,
        None => WPAD_ERR_NOT_READY as i32,
    }
}

fn current_ir_aspect() -> i32 {
    match conf::get_aspect_ratio() {
        x if x == conf::CONF_ASPECT_16_9 as i32 => 1,
        _ => 0,
    }
}

fn current_sensorbar_bottom() -> bool {
    match conf::get_sensor_bar_position() {
        x if x == conf::CONF_SENSORBAR_BOTTOM as i32 => true,
        x if x == conf::CONF_SENSORBAR_TOP as i32 => false,
        _ => false,
    }
}

fn current_ir_sensitivity() -> i32 {
    let sensitivity = conf::get_ir_sensitivity();
    if (1..=5).contains(&sensitivity) {
        sensitivity
    } else {
        3
    }
}

pub(super) fn set_ir_resolution(slot_idx: usize, width: u32, height: u32) {
    IR_RESOLUTION_X[slot_idx].store(width, Ordering::Relaxed);
    IR_RESOLUTION_Y[slot_idx].store(height, Ordering::Relaxed);
    IR_RESOLUTION_SET[slot_idx].store(true, Ordering::Release);
    unsafe {
        WPAD_DATA_STATE[slot_idx].ir.vres = [width, height];
        WPAD_DATA_STATE[slot_idx].data_present |= crate::WPAD_DATA_IR;
    }
    mark_activity(slot_idx);
}

fn sync_ir_config(slot_idx: usize) {
    let aspect = current_ir_aspect();
    let (aspect, default_vres) = ir_aspect_vres(aspect);
    // WPAD_SetVRes configures the caller's coordinate system before a Remote
    // connects. Camera/connection setup must not replace it with aspect defaults.
    let vres = if IR_RESOLUTION_SET[slot_idx].load(Ordering::Acquire) {
        [
            IR_RESOLUTION_X[slot_idx].load(Ordering::Relaxed),
            IR_RESOLUTION_Y[slot_idx].load(Ordering::Relaxed),
        ]
    } else {
        default_vres
    };
    let offset = ir_position_offset(aspect, current_sensorbar_bottom());
    unsafe {
        WPAD_DATA_STATE[slot_idx].ir.aspect = aspect;
        WPAD_DATA_STATE[slot_idx].ir.vres = vres;
        WPAD_DATA_STATE[slot_idx].ir.offset = offset;
    }
}

fn refresh_wii_remote_reporting(slot_idx: usize) -> bool {
    let Some(fmt) = WPAD_FORMAT
        .get(slot_idx)
        .map(|fmt| fmt.load(Ordering::Acquire))
    else {
        return false;
    };
    let has_expansion = slot_is_wii_remote(slot_idx)
        && unsafe { WPAD_DATA_STATE[slot_idx].exp.type_ != WPAD_EXP_NONE as i32 };
    let Some((continuous, report_type)) = report_type_for_format(fmt, has_expansion) else {
        return false;
    };
    let mut report_cmd = [WM_CMD_REPORT_TYPE, 0x00, report_type];
    report_cmd[1] = if continuous { 0x04 } else { 0x00 };
    if report_cmd.len() > 1 {
        report_cmd[1] |= 0x02;
    }
    if !send_wii_raw_command(slot_idx, &mut report_cmd) {
        return false;
    }

    let wants_ir = fmt == WPAD_FMT_BTNS_ACC_IR;
    if wants_ir {
        if !enable_wii_ir(slot_idx, has_expansion) {
            return false;
        }
    } else if !disable_wii_ir(slot_idx) {
        return false;
    }
    true
}

fn enable_wii_ir(slot_idx: usize, has_expansion: bool) -> bool {
    let wii_u_pro = with_slot(slot_idx, |slot| slot.wii_u_pro).unwrap_or(false);
    if wii_u_pro {
        let _ = request_wii_status(slot_idx);
        return true;
    }
    sync_ir_config(slot_idx);
    let sensitivity = ir_sensitivity_blocks(current_ir_sensitivity());
    if !send_wii_simple_command(slot_idx, WM_CMD_IR, &[0x04], true) {
        return false;
    }
    if !send_wii_simple_command(slot_idx, WM_CMD_IR_2, &[0x04], true) {
        return false;
    }
    if !send_wii_write(slot_idx, WM_REG_IR, &[0x08]) {
        return false;
    }
    if !send_wii_write(slot_idx, WM_REG_IR_BLOCK1, &sensitivity.block1) {
        return false;
    }
    if !send_wii_write(slot_idx, WM_REG_IR_BLOCK2, &sensitivity.block2) {
        return false;
    }
    if !send_wii_write(
        slot_idx,
        WM_REG_IR_MODENUM,
        &[ir_mode_for_expansion(has_expansion)],
    ) {
        return false;
    }
    let _ = request_wii_status(slot_idx);
    with_slot_mut(slot_idx, |slot| {
        slot.ir_enabled = true;
    });
    true
}

fn disable_wii_ir(slot_idx: usize) -> bool {
    let was_enabled = with_slot(slot_idx, |slot| slot.ir_enabled).unwrap_or(false);
    if should_send_ir_disable_commands(was_enabled) {
        if !send_wii_simple_command(slot_idx, WM_CMD_IR, &[0x00], true) {
            return false;
        }
        if !send_wii_simple_command(slot_idx, WM_CMD_IR_2, &[0x00], true) {
            return false;
        }
    }
    let _ = request_wii_status(slot_idx);
    with_slot_mut(slot_idx, |slot| {
        slot.ir_enabled = false;
    });
    true
}

const fn should_send_ir_disable_commands(ir_enabled: bool) -> bool {
    ir_enabled
}

fn request_initial_expansion_id(slot_idx: usize) -> bool {
    let Some(()) = with_slot_mut(slot_idx, |slot| {
        slot.pending_read = PendingReadKind::InitialExpansionId;
        slot.read = ReadAssembly::new(WM_EXP_ID, 6);
    }) else {
        return false;
    };
    if send_wii_read(slot_idx, WM_EXP_ID, 6) {
        true
    } else {
        let _ = with_slot_mut(slot_idx, |slot| {
            slot.pending_read = PendingReadKind::None;
        });
        false
    }
}

fn request_wii_calibration(slot_idx: usize) -> bool {
    let Some(()) = with_slot_mut(slot_idx, |slot| {
        slot.pending_read = PendingReadKind::WiiRemoteCalibration;
        slot.read = ReadAssembly::new(WM_MEM_OFFSET_CALIBRATION, 8);
    }) else {
        return false;
    };
    if send_wii_read(slot_idx, WM_MEM_OFFSET_CALIBRATION, 8) {
        true
    } else {
        let _ = with_slot_mut(slot_idx, |slot| {
            slot.pending_read = PendingReadKind::None;
        });
        false
    }
}

fn begin_expansion_handshake(slot_idx: usize) -> bool {
    let Some(action) = with_slot_mut(slot_idx, |slot| {
        slot.expansion_handshake_active = true;
        slot.expansion_setup.start()
    }) else {
        return false;
    };
    let Some(action) = action else {
        return false;
    };
    if !submit_expansion_action(slot_idx, action) {
        if !abort_wii_command_queue(slot_idx) {
            return false;
        }
        return false;
    }
    true
}

fn submit_expansion_action(slot_idx: usize, action: ExpansionSetupAction) -> bool {
    match action {
        ExpansionSetupAction::WriteEnableOne => send_wii_write_tagged(
            slot_idx,
            WM_EXP_MEM_ENABLE1,
            &[0x55],
            CommandTag::ExpansionEnableOne,
        ),
        ExpansionSetupAction::WriteEnableTwo => send_wii_write_tagged(
            slot_idx,
            WM_EXP_MEM_ENABLE2,
            &[0x00],
            CommandTag::ExpansionEnableTwo,
        ),
        ExpansionSetupAction::ReadCalibration => {
            let Some(()) = with_slot_mut(slot_idx, |slot| {
                slot.pending_read = PendingReadKind::ExpansionCalibration;
                slot.read = ReadAssembly::new(WM_EXP_MEM_CALIBR, 224);
            }) else {
                return false;
            };
            send_wii_read(slot_idx, WM_EXP_MEM_CALIBR, 224)
        }
    }
}

fn initialize_wii_remote(slot_idx: usize) -> bool {
    if !with_slot_mut(slot_idx, |slot| {
        slot.kind = ControllerKind::WiiRemote;
        slot.wii_initialized = true;
        slot.pending_read = PendingReadKind::None;
        slot.expansion_handshake_active = false;
        slot.ir_enabled = false;
    })
    .is_some()
    {
        return false;
    }

    sync_ir_config(slot_idx);

    let mut led_command = wii_remote_led_command(slot_idx);
    if !send_wii_raw_command(slot_idx, &mut led_command) {
        return false;
    }

    // libogc serializes the initial handshake: status, optional expansion ID,
    // accelerometer calibration, then a final status/report refresh.
    request_wii_status(slot_idx)
}

const fn wii_remote_led_command(channel: usize) -> [u8; 2] {
    [WM_CMD_LED, (0x10 << (channel % 4)) | 0x02]
}

fn handle_wii_status_report(slot_idx: usize, report: &[u8]) -> bool {
    let Some(status) = RemoteStatusReport::decode(report) else {
        return false;
    };
    let Some(chan) = valid_chan(slot_idx as i32) else {
        return false;
    };

    let mut needs_init = false;
    let mut handshake_complete = false;
    let mut pending_read = PendingReadKind::None;
    let Some(()) = with_slot_mut(slot_idx, |slot| unsafe {
        slot.kind = ControllerKind::WiiRemote;
        slot.attachment_present = status.attachment;
        slot.ir_enabled = status.ir;
        needs_init = !slot.wii_initialized;
        handshake_complete = slot.initial_handshake.is_complete();
        pending_read = slot.pending_read;
        if !status.attachment && handshake_complete {
            slot.expansion_handshake_active = false;
            slot.expansion_setup.complete();
            slot.pending_read = PendingReadKind::None;
            WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NONE as i32;
            WPAD_DATA_STATE[chan].data_present &= !WPAD_DATA_EXPANSION;
        }
        WPAD_DATA_STATE[chan].battery_level = status.battery_level;
    }) else {
        return false;
    };

    if needs_init {
        return initialize_wii_remote(slot_idx);
    }

    if !handshake_complete {
        if !matches!(pending_read, PendingReadKind::None) {
            return true;
        }
        let Some(action) = with_slot_mut(slot_idx, |slot| {
            slot.initial_handshake.status_report(status.attachment)
        })
        .and_then(Result::ok) else {
            return false;
        };
        return request_initial_handshake_action(slot_idx, action);
    }

    let Some(expansion_handshake_available) = with_slot(slot_idx, |slot| {
        !slot.expansion_handshake_active && matches!(slot.pending_read, PendingReadKind::None)
    }) else {
        return false;
    };
    let needs_expansion = status.attachment
        && expansion_handshake_available
        && unsafe { WPAD_DATA_STATE[chan].exp.type_ == WPAD_EXP_NONE as i32 };
    if needs_expansion {
        return begin_expansion_handshake(slot_idx);
    }
    refresh_wii_remote_reporting(slot_idx)
}

fn handle_wii_read_report(slot_idx: usize, report: &[u8]) -> bool {
    let Some(chan) = valid_chan(slot_idx as i32) else {
        return false;
    };
    if !apply_read_report_buttons(unsafe { &mut WPAD_DATA_STATE[chan] }, report) {
        return false;
    }
    let Some(pending) = with_slot(slot_idx, |slot| slot.pending_read) else {
        return true;
    };
    if matches!(pending, PendingReadKind::None) {
        return true;
    }

    let outcome = with_slot_mut(slot_idx, |slot| slot.read.apply_report(report));
    let Some(outcome) = outcome else {
        return false;
    };

    match outcome {
        ReadFragmentResult::Incomplete => true,
        ReadFragmentResult::Error(_) => {
            let retry_status = matches!(
                pending,
                PendingReadKind::InitialExpansionId | PendingReadKind::WiiRemoteCalibration
            );
            let reset = with_slot_mut(slot_idx, |slot| {
                slot.pending_read = PendingReadKind::None;
                slot.expansion_handshake_active = false;
                slot.expansion_setup.complete();
                if retry_status {
                    slot.initial_handshake.retry();
                }
            });
            if reset.is_none() {
                return false;
            }
            let requested = if retry_status {
                request_wii_status(slot_idx)
            } else {
                true
            };
            let completed = complete_wii_response(slot_idx, WM_CMD_READ_DATA);
            requested && completed
        }
        ReadFragmentResult::Complete(_) => {
            let mut need_refresh = false;
            let mut initial_followup = None;
            let mut invalid_handshake_transition = false;
            let mut retry_expansion_calibration = false;
            let Some(()) = with_slot_mut(slot_idx, |slot| {
                match pending {
                    PendingReadKind::InitialExpansionId => {
                        if slot.read.requested() < 6 {
                            slot.initial_handshake.retry();
                            invalid_handshake_transition = true;
                        } else {
                            let buffer = slot.read.buffer();
                            let id =
                                u32::from_be_bytes([buffer[2], buffer[3], buffer[4], buffer[5]]);
                            match complete_initial_expansion_id(slot, id) {
                                Ok(action) => initial_followup = Some(action),
                                Err(_) => {
                                    slot.initial_handshake.retry();
                                    invalid_handshake_transition = true;
                                }
                            }
                        }
                    }
                    PendingReadKind::WiiRemoteCalibration => {
                        slot.accel_calib = wiimote_calibration_or_default(
                            &slot.read.buffer()[..slot.read.requested()],
                        );
                        match slot.initial_handshake.calibration_complete() {
                            Ok(action) => initial_followup = Some(action),
                            Err(_) => {
                                slot.initial_handshake.retry();
                                invalid_handshake_transition = true;
                            }
                        }
                    }
                    PendingReadKind::ExpansionCalibration => {
                        let calibration_data = slot.read.buffer();
                        let is_wiiboard = calibration_data
                            .get(220..224)
                            .is_some_and(|id| id == 0xa4200402u32.to_be_bytes());
                        let is_nunchuk = calibration_data
                            .get(220..224)
                            .is_some_and(|id| id == 0xa4200000u32.to_be_bytes());
                        let retry_board = is_wiiboard
                            && BalanceBoardCalibration::requires_retry(calibration_data);
                        let retry_nunchuk =
                            is_nunchuk && NunchukCalibration::requires_retry(calibration_data);
                        if retry_board || retry_nunchuk {
                            slot.read = ReadAssembly::new(WM_EXP_MEM_CALIBR, 224);
                            retry_expansion_calibration = true;
                        } else {
                            unsafe {
                                if decode_expansion_calibration(
                                    calibration_data,
                                    &mut WPAD_DATA_STATE[chan].exp,
                                ) {
                                    WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
                                    need_refresh = true;
                                } else {
                                    WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NONE as i32;
                                    WPAD_DATA_STATE[chan].data_present &= !WPAD_DATA_EXPANSION;
                                }
                            }
                            slot.expansion_handshake_active = false;
                            slot.expansion_setup.complete();
                        }
                    }
                    PendingReadKind::None => {}
                }
                if pending != PendingReadKind::ExpansionCalibration || !retry_expansion_calibration
                {
                    slot.pending_read = PendingReadKind::None;
                }
            }) else {
                return false;
            };
            if invalid_handshake_transition {
                if !complete_wii_response(slot_idx, WM_CMD_READ_DATA) {
                    return false;
                }
                return false;
            }
            let followup = match pending {
                PendingReadKind::InitialExpansionId | PendingReadKind::WiiRemoteCalibration => {
                    initial_followup
                        .is_some_and(|action| request_initial_handshake_action(slot_idx, action))
                }
                PendingReadKind::ExpansionCalibration => {
                    if retry_expansion_calibration {
                        if send_wii_read(slot_idx, WM_EXP_MEM_CALIBR, 224) {
                            true
                        } else {
                            let Some(()) = with_slot_mut(slot_idx, |slot| {
                                slot.pending_read = PendingReadKind::None;
                                slot.expansion_handshake_active = false;
                                unsafe {
                                    WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NONE as i32;
                                    WPAD_DATA_STATE[chan].data_present &= !WPAD_DATA_EXPANSION;
                                }
                            }) else {
                                return false;
                            };
                            false
                        }
                    } else if need_refresh {
                        refresh_wii_remote_reporting(slot_idx)
                    } else {
                        true
                    }
                }
                PendingReadKind::None => true,
            };
            let completed = complete_wii_response(slot_idx, WM_CMD_READ_DATA);
            followup && completed
        }
    }
}

fn handle_wii_remote_report(slot_idx: usize, report: &[u8]) -> bool {
    let Some(report_type) = report.first().copied() else {
        return false;
    };
    if !(0x20..=0x37).contains(&report_type) {
        return false;
    }

    if !slot_is_wii_remote(slot_idx) {
        return false;
    }

    match report_type {
        0x20 => {
            handle_wii_status_report(slot_idx, report)
                && complete_wii_response(slot_idx, WM_CMD_CTRL_STATUS)
        }
        0x21 => handle_wii_read_report(slot_idx, report),
        0x22 => {
            let Some(ack) = decode_ack_report(report) else {
                return false;
            };
            let Some(chan) = valid_chan(slot_idx as i32) else {
                return false;
            };
            if !apply_read_report_buttons(unsafe { &mut WPAD_DATA_STATE[chan] }, report) {
                return false;
            }
            push_event(chan);
            complete_wii_ack(slot_idx, ack)
        }
        0x30..=0x37 => {
            let Some(chan) = valid_chan(slot_idx as i32) else {
                return false;
            };
            let prev = unsafe { WPAD_DATA_STATE[chan] };
            let mut current = prev;
            let Some(accel_calib) = with_slot(slot_idx, |slot| slot.accel_calib) else {
                return false;
            };
            if shape_wiimote_input_report(&prev, &mut current, &accel_calib, report) {
                unsafe {
                    WPAD_DATA_STATE[chan] = current;
                }
                push_event(chan);
            }
            true
        }
        _ => false,
    }
}

fn clear_slot(slot_idx: usize) {
    cancel_idle_timer(slot_idx);
    clear_stream(slot_idx);
    reset_activity(slot_idx);
    let _ = with_slot_mut(slot_idx, |slot| *slot = ControllerSlot::new());
    if let Some(chan) = valid_chan(slot_idx as i32) {
        WPAD_SPEAKER[chan].store(false, Ordering::Release);
        WPAD_RUMBLE[chan].store(false, Ordering::Release);
        WPAD_STREAM_LEN[chan].store(0, Ordering::Release);
        set_connected(chan, false);
        reset_data(chan);
    }
}

fn hide_searching_wii_remote(slot_idx: usize) {
    let hidden = with_slot_mut(slot_idx, mark_search_disconnect).unwrap_or(false);
    if !hidden {
        return;
    }
    cancel_idle_timer(slot_idx);
    clear_stream(slot_idx);
    if let Some(chan) = valid_chan(slot_idx as i32) {
        WPAD_SPEAKER[chan].store(false, Ordering::Release);
        WPAD_RUMBLE[chan].store(false, Ordering::Release);
        WPAD_STREAM_LEN[chan].store(0, Ordering::Release);
        set_connected(chan, false);
        reset_data(chan);
    }
}

fn clear_all_slots() {
    for idx in 0..MAX_POWERPC_SLOTS {
        clear_slot(idx);
    }
}

struct ServiceGuard(u32);
impl Drop for ServiceGuard {
    fn drop(&mut self) {
        lwp::critical_exit(self.0);
    }
}
fn with_bt_input_queue<T>(f: impl FnOnce(&mut bt::BluetoothInputQueue) -> T) -> T {
    let _guard = ServiceGuard(lwp::critical_enter());
    // SAFETY: this is the only access to BT_INPUT_QUEUE, and the guard
    // serializes the producer, consumer, and compatibility-state updates.
    unsafe { f(&mut *BT_INPUT_QUEUE.0.get()) }
}
fn with_host<T>(f: impl FnOnce(&mut bt::WiiHidHost) -> T) -> Option<T> {
    let _guard = ServiceGuard(lwp::critical_enter());
    if !BT_HOST_READY.load(Ordering::Acquire) {
        return None;
    }
    // All service access is serialized. IOS callbacks only own request state.
    Some(unsafe { f(&mut *addr_of_mut!(BT_HOST).cast::<bt::WiiHidHost>()) })
}
fn now_ms() -> u64 {
    lwp::now_ticks() / (lwp::TICKS_PER_SECOND / 1000)
}

pub fn bluetooth_diagnostics() -> bt::HostDiagnostics {
    with_host(|host| host.diagnostics()).unwrap_or_else(|| bt::HidHost::new().diagnostics())
}
pub fn bluetooth_switch_calibration(slot: usize) -> Option<bt::switch::SwitchCalibration> {
    with_host(|host| host.switch_calibration(slot)).flatten()
}
pub fn bluetooth_hid_state(slot: usize) -> crate::BluetoothHidState {
    let _guard = ServiceGuard(lwp::critical_enter());
    if slot >= bt::HID_SLOTS {
        return crate::BluetoothHidState::EMPTY;
    }
    unsafe { HID_INPUT[slot] }
}
pub fn bluetooth_hid_next_event() -> Option<bt::BluetoothInputEvent> {
    with_bt_input_queue(|queue| queue.pop())
}

fn hid_queue_error_code(error: bt::BluetoothInputQueueError) -> i32 {
    match error {
        bt::BluetoothInputQueueError::Full => crate::WPAD_ERR_HID_QUEUE_FULL,
        bt::BluetoothInputQueueError::InvalidSlot
        | bt::BluetoothInputQueueError::AlreadyConnected
        | bt::BluetoothInputQueueError::NotConnected
        | bt::BluetoothInputQueueError::StaleGeneration
        | bt::BluetoothInputQueueError::ReportTooLarge
        | bt::BluetoothInputQueueError::GenerationExhausted => WPAD_ERR_TRANSFER as i32,
    }
}

/// Queue one disconnection before clearing the compatibility snapshot. A
/// repeated host disconnection after a local disconnect is an expected
/// duplicate and does not create a second input event.
fn enqueue_hid_disconnect(slot: usize, reason: u8) -> Result<bool, i32> {
    if slot >= bt::HID_SLOTS {
        return Err(WPAD_ERR_TRANSFER as i32);
    }
    with_bt_input_queue(|queue| {
        let state = unsafe {
            core::ptr::addr_of!(HID_INPUT)
                .cast::<crate::BluetoothHidState>()
                .add(slot)
                .read()
        };
        if !state.connected || state.family == Some(bt::ControllerFamily::WiiRemote) {
            return Ok(false);
        }
        let Some(generation) = queue.active_generation(slot) else {
            return Err(WPAD_ERR_TRANSFER as i32);
        };
        queue
            .push_disconnected(slot, generation, reason)
            .map_err(hid_queue_error_code)?;
        unsafe {
            addr_of_mut!(HID_INPUT)
                .cast::<crate::BluetoothHidState>()
                .add(slot)
                .write(crate::BluetoothHidState::EMPTY);
        }
        Ok(true)
    })
}

fn clear_hid_snapshot(slot: usize) {
    if slot < bt::HID_SLOTS {
        let _guard = ServiceGuard(lwp::critical_enter());
        unsafe {
            addr_of_mut!(HID_INPUT)
                .cast::<crate::BluetoothHidState>()
                .add(slot)
                .write(crate::BluetoothHidState::EMPTY);
        }
    }
}

fn process_hid_event(event: bt::HidEvent) -> Result<(), i32> {
    // Shutdown has already published disconnections and cleared input slots.
    // Reports accepted before shutdown must not recreate a connection or enter
    // the generic HID queue without a live generation. Consume only those
    // obsolete input events; terminal events and stored keys still run below.
    if matches!(
        &event,
        bt::HidEvent::Connected { .. } | bt::HidEvent::Report { .. }
    ) && bluetooth_diagnostics().stage == bt::HostStage::Closing
    {
        return Ok(());
    }
    match event {
        bt::HidEvent::Connected {
            slot,
            address,
            handle,
            family,
            calibration,
            guest_pairing,
            guest_key,
        } => {
            if slot >= bt::HID_SLOTS {
                return Err(WPAD_ERR_TRANSFER as i32);
            }
            // WPAD_Search may race with a Connected event that the adapter
            // worker queued before the disconnect request. Do not republish a
            // controller that the search has already hidden.
            if !with_slot(slot, should_accept_connected_event).unwrap_or(false) {
                return Ok(());
            }
            let kind = if family == bt::ControllerFamily::WiiRemote {
                ControllerKind::WiiRemote
            } else if family.is_switch() {
                ControllerKind::Switch
            } else {
                ControllerKind::Generic
            };
            if family.is_switch() && calibration.is_none() {
                return Err(WPAD_ERR_TRANSFER as i32);
            }
            if guest_pairing
                && family == bt::ControllerFamily::WiiRemote
                && let Some(key) = guest_key
            {
                record_guest_pairing(address, key)?;
            }
            with_bt_input_queue(|queue| {
                let generation = if family == bt::ControllerFamily::WiiRemote {
                    0
                } else {
                    queue
                        .push_connected(slot, address, handle, family, calibration)
                        .map_err(hid_queue_error_code)?
                };
                unsafe {
                    HID_INPUT[slot] = crate::BluetoothHidState {
                        connected: true,
                        address,
                        handle,
                        family: Some(family),
                        generation,
                        ..crate::BluetoothHidState::EMPTY
                    };
                }
                Ok::<(), i32>(())
            })?;
            let _ = with_slot_mut(slot, |state| {
                *state = ControllerSlot {
                    active: true,
                    kind,
                    ..ControllerSlot::new()
                };
            });
            if kind == ControllerKind::WiiRemote {
                set_connected(slot, true);
                if !initialize_wii_remote(slot) {
                    return Err(WPAD_ERR_TRANSFER as i32);
                }
            }
        }
        bt::HidEvent::Report { slot, data } => {
            if slot >= bt::HID_SLOTS {
                return Err(WPAD_ERR_TRANSFER as i32);
            }
            if slot_is_search_disconnect_pending(slot) {
                return Ok(());
            }
            if slot_is_wii_remote(slot) {
                if !handle_wii_remote_report(slot, &data) {
                    return Err(WPAD_ERR_TRANSFER as i32);
                }
            } else {
                with_bt_input_queue(|queue| unsafe {
                    let generation = queue
                        .active_generation(slot)
                        .ok_or(WPAD_ERR_TRANSFER as i32)?;
                    queue
                        .push_report(slot, generation, &data)
                        .map_err(hid_queue_error_code)?;
                    let state = &mut HID_INPUT[slot];
                    state.report[..data.len()].copy_from_slice(&data);
                    state.report_len = data.len() as u16;
                    state.report_count = state.report_count.wrapping_add(1);
                    Ok::<(), i32>(())
                })?;
            }
            mark_activity(slot);
        }
        bt::HidEvent::Disconnected { slot, reason } => {
            if slot >= bt::HID_SLOTS {
                return Err(WPAD_ERR_TRANSFER as i32);
            }
            enqueue_hid_disconnect(slot, reason)?;
            clear_slot(slot);
            clear_hid_snapshot(slot);
        }
        bt::HidEvent::ChannelDisconnected { slot, channel } => {
            if slot >= bt::HID_SLOTS {
                return Err(WPAD_ERR_TRANSFER as i32);
            }
            let bit = match channel {
                bt::HidChannel::Control => 1,
                bt::HidChannel::Interrupt => 2,
            };
            let will_close_last_channel =
                with_slot(slot, |state| (state.closed_hid_channels | bit) == 3).unwrap_or(false);
            if will_close_last_channel {
                enqueue_hid_disconnect(slot, 0)?;
            }
            let all_closed = with_slot_mut(slot, |state| {
                close_hid_channel(&mut state.closed_hid_channels, channel)
            })
            .unwrap_or(false);
            if all_closed {
                clear_slot(slot);
                clear_hid_snapshot(slot);
            }
        }
        bt::HidEvent::Failed { .. } => {}
    }
    Ok(())
}

pub fn set_data_format(chan: i32, fmt: i32) -> i32 {
    let all_channels = chan == WPAD_CHAN_ALL;
    if all_channels && !(WPAD_FMT_BTNS..=WPAD_FMT_BTNS_ACC_IR).contains(&fmt) {
        return WPAD_ERR_BADVALUE as i32;
    }
    if all_channels {
        for idx in 0..WPAD_MAX_DEVICES {
            let ret = set_data_format(idx as i32, fmt);
            if ret < WPAD_ERR_NONE as i32 {
                return ret;
            }
        }
        return WPAD_ERR_NONE as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if !(WPAD_FMT_BTNS..=WPAD_FMT_BTNS_ACC_IR).contains(&fmt) {
        return WPAD_ERR_BADVALUE as i32;
    }
    WPAD_FORMAT[chan].store(fmt, Ordering::Release);
    if slot_is_wii_remote(chan) && !refresh_wii_remote_reporting(chan) {
        return WPAD_ERR_TRANSFER as i32;
    }
    WPAD_ERR_NONE as i32
}

pub fn probe(chan: i32, type_: *mut u32) -> i32 {
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    let status = probe_status(
        WPAD_GLOBAL_STATUS.load(Ordering::Acquire),
        WPAD_CONNECTED[chan].load(Ordering::Acquire),
        with_slot(chan, |slot| slot.initial_handshake.is_complete()).unwrap_or(false),
    );
    if status != WPAD_ERR_NONE as i32 {
        return probe_result(status, type_, 0);
    }
    let expansion_type =
        unsafe { crate::probe_expansion_type_for_wii(WPAD_DATA_STATE[chan].exp.type_) };
    probe_result(status, type_, expansion_type)
}

fn probe_result(status: i32, type_: *mut u32, expansion_type: u32) -> i32 {
    if status == WPAD_ERR_NONE as i32 && !type_.is_null() {
        unsafe { *type_ = expansion_type };
    }
    status
}

pub fn init() -> i32 {
    {
        let _guard = ServiceGuard(lwp::critical_enter());
        if !BT_HOST_READY.load(Ordering::Acquire) {
            unsafe {
                addr_of_mut!(BT_HOST).write(MaybeUninit::new(bt::WiiHidHost::new()));
            }
            BT_HOST_READY.store(true, Ordering::Release);
        }
    }
    let paired_keys = match load_paired_keys() {
        Ok(guests) => guests,
        Err(_) => return WPAD_ERR_BADCONF as i32,
    };
    if paired_keys.num_guests > BT_GUEST_SLOTS as u8 {
        return WPAD_ERR_BADCONF as i32;
    }
    store_volatile_guests(paired_keys);
    if conf::set_pad_guest_devices(None) < 0 {
        return WPAD_ERR_BADCONF as i32;
    }
    match with_host(|host| {
        host.start(now_ms())?;
        for index in 0..usize::from(paired_keys.num_guests.min(BT_GUEST_SLOTS as u8)) {
            let guest = guest_at(&paired_keys, index);
            if guest.bdaddr != [0; 6] {
                host.load_guest_link_key(
                    bt::BluetoothAddress::new(reverse_bytes(guest.bdaddr)),
                    reverse_key(guest.link_key),
                );
            }
        }
        Ok::<(), bt::Error>(())
    }) {
        Some(Ok(())) => {
            unsafe {
                sensorbar_enable(true);
            }
            WPAD_ERR_NONE as i32
        }
        _ => WPAD_ERR_NOT_READY as i32,
    }
}

/// Starts inquiry. Completion and connections are serviced by scan_pads.
pub fn search() -> i32 {
    begin_search(true)
}
pub fn search_bluetooth_hid() -> i32 {
    begin_search(false)
}
fn begin_search(guest: bool) -> i32 {
    let search = with_host(|host| {
        let disconnects = if guest {
            host.wii_remote_link_mask()
        } else {
            0
        };
        let started = if guest {
            host.search_wii_guest(now_ms())
        } else {
            host.search(now_ms())
        };
        if guest && started {
            with_volatile_guests(|guests| *guests = conf::ConfPadGuests::default())?;
        }
        Ok::<_, i32>((started, disconnects))
    });
    if let Some(Ok((true, disconnects))) = search {
        for slot in 0..bt::HID_SLOTS {
            if disconnects & (1 << slot) != 0 {
                hide_searching_wii_remote(slot);
            }
        }
        crate::powerpc_runtime_begin_search();
        WPAD_ERR_NONE as i32
    } else {
        WPAD_ERR_NOT_READY as i32
    }
}
pub fn stop_search() -> i32 {
    if with_host(|host| host.stop_search()).is_none() {
        return WPAD_ERR_NOT_READY as i32;
    }
    crate::powerpc_runtime_end_search();
    WPAD_ERR_NONE as i32
}
/// Begins shutdown without waiting for IOS. Call scan_pads until completion.
pub fn shutdown() -> i32 {
    if !BT_HOST_READY.load(Ordering::Acquire) {
        return WPAD_ERR_NOT_READY as i32;
    }
    let result = with_bt_input_queue(|queue| {
        if !BT_HOST_READY.load(Ordering::Acquire) {
            return Err(WPAD_ERR_NOT_READY as i32);
        }
        let mut disconnects = [(0usize, 0u32); bt::HID_SLOTS];
        let mut disconnect_count = 0;
        for slot in 0..bt::HID_SLOTS {
            let state = unsafe {
                core::ptr::addr_of!(HID_INPUT)
                    .cast::<crate::BluetoothHidState>()
                    .add(slot)
                    .read()
            };
            if !state.connected || state.family == Some(bt::ControllerFamily::WiiRemote) {
                continue;
            }
            let Some(generation) = queue.active_generation(slot) else {
                return Err(WPAD_ERR_TRANSFER as i32);
            };
            disconnects[disconnect_count] = (slot, generation);
            disconnect_count += 1;
        }
        if queue.len() + disconnect_count > bt::BLUETOOTH_INPUT_QUEUE_CAPACITY {
            return Err(crate::WPAD_ERR_HID_QUEUE_FULL);
        }
        for &(slot, generation) in &disconnects[..disconnect_count] {
            queue
                .push_disconnected(slot, generation, 0)
                .map_err(hid_queue_error_code)?;
        }
        for slot in 0..bt::HID_SLOTS {
            unsafe {
                addr_of_mut!(HID_INPUT)
                    .cast::<crate::BluetoothHidState>()
                    .add(slot)
                    .write(crate::BluetoothHidState::EMPTY);
            }
        }
        let host = unsafe { &mut *addr_of_mut!(BT_HOST).cast::<bt::WiiHidHost>() };
        host.shutdown(now_ms());
        Ok(())
    });
    if let Err(error) = result {
        return error;
    }
    // BTE_Close is synchronous in libogc. Rust starts its asynchronous adapter
    // close above, then commits the same guest list before publishing disabled
    // WPAD state. Keep a save error visible, but finish the shutdown cleanup.
    let save_result = with_volatile_guests(|guests| {
        let set_result = conf::set_pad_guest_devices(Some(guests));
        if set_result < 0 {
            return set_result;
        }
        conf::save_changes()
    })
    .unwrap_or_else(|error| error);
    unsafe {
        sensorbar_enable(false);
    }
    clear_all_slots();
    crate::powerpc_runtime_reset();
    if save_result < 0 {
        WPAD_ERR_BADCONF as i32
    } else {
        WPAD_ERR_NONE as i32
    }
}
pub fn bluetooth_shutdown_complete() -> bool {
    with_host(|host| host.shutdown_complete()).unwrap_or(true)
}

pub fn disconnect(chan: i32) -> i32 {
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    disconnect_slot(chan)
}

pub fn scan_pads() -> i32 {
    bluetooth_poll_mark("WPAD adapter service state check starting");
    let host_available = BT_HOST_READY.load(Ordering::Acquire);
    bluetooth_poll_mark("WPAD adapter service state check completed");
    if !host_available {
        return WPAD_ERR_NOT_READY as i32;
    }
    // Start service when the caller starts consuming events. Starting it during
    // init can fill the event queue while the application still prepares its UI.
    if let Err(error) = start_bluetooth_worker() {
        return error;
    }
    bluetooth_poll_mark("WPAD IPC completion poll starting");
    // `poll` reports whether it serviced an IPC status bit; it is not an
    // operation status, so continue scanning when there was no work to service.
    ipc_poll();
    bluetooth_poll_mark("WPAD IPC completion poll completed");
    let requested = DISCONNECT_REQUESTS.swap(0, Ordering::AcqRel);
    let mut pending_disconnects = requested;
    for slot in 0..MAX_POWERPC_SLOTS {
        if requested & (1 << slot) != 0 {
            let result = disconnect_slot(slot);
            if result < WPAD_ERR_NONE as i32 {
                DISCONNECT_REQUESTS.fetch_or(pending_disconnects, Ordering::AcqRel);
                return result;
            }
            pending_disconnects &= !(1 << slot);
        }
    }
    while let Some(event) = with_host(|host| host.peek_event()).flatten() {
        if let Err(error) = process_hid_event(event) {
            if error == WPAD_ERR_BADCONF as i32 {
                WPAD_GLOBAL_STATUS.store(WPAD_STATE_DISABLED, Ordering::Release);
            }
            return error;
        }
        if with_host(|host| host.next_event()).flatten().is_none() {
            WPAD_GLOBAL_STATUS.store(WPAD_STATE_DISABLED, Ordering::Release);
            return WPAD_ERR_NOT_READY as i32;
        }
    }
    let status = bluetooth_diagnostics();
    if status.stage == bt::HostStage::Ready {
        WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLED, Ordering::Release);
        if !status.searching {
            crate::powerpc_runtime_end_search();
        }
    } else if status.stage == bt::HostStage::Failed {
        crate::powerpc_runtime_end_search();
        WPAD_GLOBAL_STATUS.store(crate::WPAD_STATE_DISABLED, Ordering::Release);
    }
    crate::read_pending(crate::WPAD_CHAN_ALL, None)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::{
        ControllerSlot, MAX_POWERPC_SLOTS, STREAM_DATA_BYTES, close_hid_channel,
        complete_initial_expansion_id, copy_stream_chunk, decode_stream_timer_token,
        encode_stream_timer_token, encode_wii_stream_data, encode_wii_write,
        mark_search_disconnect, motion_plus_preflight, probe_result, probe_status, reverse_bytes,
        reverse_key, should_accept_connected_event, should_send_ir_disable_commands,
        wii_remote_led_command,
    };
    use crate::{
        WPAD_CHAN_UNKNOWN, WPAD_ERR_NONE, WPAD_ERR_NOT_READY, WPAD_MAX_DEVICES,
        WPAD_STATE_DISABLED, WPAD_STATE_ENABLED,
    };
    use wiiuse_rs::{HandshakeAction, WII_U_PRO_EXPANSION_ID};

    #[test]
    fn backend_allocates_six_channel_slots() {
        assert_eq!(MAX_POWERPC_SLOTS, WPAD_MAX_DEVICES);
        assert_eq!(MAX_POWERPC_SLOTS, 6);
        assert_eq!(WPAD_CHAN_UNKNOWN as usize, MAX_POWERPC_SLOTS - 1);
    }

    #[test]
    fn guest_search_hides_wii_remote_until_disconnect_completion() {
        let mut slot = ControllerSlot {
            active: true,
            kind: super::ControllerKind::WiiRemote,
            ..ControllerSlot::new()
        };
        assert!(slot.is_wii_remote());
        assert!(should_accept_connected_event(&slot));
        assert!(mark_search_disconnect(&mut slot));
        assert!(slot.active);
        assert!(slot.search_disconnect_pending);
        assert!(!slot.is_wii_remote());
        assert!(!should_accept_connected_event(&slot));
        assert!(!mark_search_disconnect(&mut ControllerSlot {
            active: true,
            kind: super::ControllerKind::Generic,
            ..ControllerSlot::new()
        }));
    }

    #[test]
    fn syconfig_guest_addresses_and_keys_use_reverse_byte_order() {
        assert_eq!(reverse_bytes([1, 2, 3, 4, 5, 6]), [6, 5, 4, 3, 2, 1]);
        assert_eq!(
            reverse_key(core::array::from_fn(|index| index as u8)),
            [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
        );
    }

    #[test]
    fn stream_timer_token_roundtrips_sixth_slot() {
        let generation = 0x0123_4567;
        let token = encode_stream_timer_token(WPAD_CHAN_UNKNOWN as usize, generation);
        assert_eq!(
            decode_stream_timer_token(token),
            (WPAD_CHAN_UNKNOWN as usize, generation)
        );
    }

    #[test]
    fn probe_waits_for_initialization_and_handshake() {
        assert_eq!(
            probe_status(WPAD_STATE_DISABLED, true, true),
            WPAD_ERR_NOT_READY as i32
        );
        assert_eq!(
            probe_status(WPAD_STATE_ENABLED, true, false),
            WPAD_ERR_NOT_READY as i32
        );
        assert_eq!(
            probe_status(WPAD_STATE_ENABLED, true, true),
            WPAD_ERR_NONE as i32
        );
    }

    #[test]
    fn probe_preserves_output_on_errors_and_writes_on_success() {
        let mut expansion = 0xfeed_beef;
        assert_eq!(
            probe_result(crate::WPAD_ERR_BAD_CHANNEL as i32, &mut expansion, 0),
            crate::WPAD_ERR_BAD_CHANNEL as i32
        );
        assert_eq!(expansion, 0xfeed_beef);

        assert_eq!(
            probe_result(crate::WPAD_ERR_NO_CONTROLLER as i32, &mut expansion, 0),
            crate::WPAD_ERR_NO_CONTROLLER as i32
        );
        assert_eq!(expansion, 0xfeed_beef);

        assert_eq!(probe_result(WPAD_ERR_NONE as i32, &mut expansion, 3), 0);
        assert_eq!(expansion, 3);
    }

    #[test]
    fn motion_plus_returns_not_ready_while_disabled() {
        assert_eq!(
            motion_plus_preflight(WPAD_STATE_DISABLED, false, false),
            Some(WPAD_ERR_NOT_READY as i32)
        );
    }

    #[test]
    fn motion_plus_is_success_for_allocated_inactive_slot() {
        assert_eq!(
            motion_plus_preflight(WPAD_STATE_ENABLED, false, false),
            Some(WPAD_ERR_NONE as i32)
        );
        assert_eq!(motion_plus_preflight(WPAD_STATE_ENABLED, true, true), None);
    }

    #[test]
    fn motion_plus_is_success_for_allocated_non_wii_remote_slot() {
        assert_eq!(
            motion_plus_preflight(WPAD_STATE_ENABLED, true, false),
            Some(WPAD_ERR_NONE as i32)
        );
    }

    #[test]
    fn wii_u_pro_id_skips_calibration_read_and_completes_handshake() {
        let mut slot = ControllerSlot::new();
        let initial_calibration = slot.accel_calib;

        assert_eq!(
            slot.initial_handshake.status_report(true),
            Ok(HandshakeAction::ReadExpansionId)
        );
        let followup = complete_initial_expansion_id(&mut slot, WII_U_PRO_EXPANSION_ID).unwrap();

        assert_eq!(followup, HandshakeAction::RefreshStatus);
        assert!(slot.wii_u_pro);
        assert!(slot.initial_handshake.is_complete());
        assert_eq!(slot.accel_calib.cal_zero.x, 0);
        assert_eq!(slot.accel_calib.cal_zero.y, 0);
        assert_eq!(slot.accel_calib.cal_zero.z, 0);
        assert_eq!(slot.accel_calib.cal_g.x, 0);
        assert_eq!(slot.accel_calib.cal_g.y, 0);
        assert_eq!(slot.accel_calib.cal_g.z, 0);
        assert_eq!(slot.accel_calib.st_alpha, initial_calibration.st_alpha);
    }

    #[test]
    fn standard_wiimote_id_keeps_calibration_read_in_handshake() {
        let mut slot = ControllerSlot::new();

        assert_eq!(
            slot.initial_handshake.status_report(true),
            Ok(HandshakeAction::ReadExpansionId)
        );
        let followup = complete_initial_expansion_id(&mut slot, 0x0000_0000).unwrap();

        assert_eq!(followup, HandshakeAction::ReadCalibration);
        assert!(!slot.wii_u_pro);
        assert!(!slot.initial_handshake.is_complete());
        assert_eq!(slot.accel_calib.cal_zero.x, 499);
        assert_eq!(slot.accel_calib.cal_g.x, 703);
    }

    #[test]
    fn ir_disable_skips_register_commands_when_ir_is_already_off() {
        assert!(!should_send_ir_disable_commands(false));
        assert!(should_send_ir_disable_commands(true));
    }

    #[test]
    fn controller_disconnect_waits_for_both_hid_channels_to_close() {
        let mut closed = 0;
        assert!(!close_hid_channel(
            &mut closed,
            ogc_bt_sync::HidChannel::Control
        ));
        assert_eq!(closed, 1);
        assert!(close_hid_channel(
            &mut closed,
            ogc_bt_sync::HidChannel::Interrupt
        ));
        assert_eq!(closed, 3);
    }

    #[test]
    fn write_command_encodes_sixteen_byte_length_as_zero() {
        let data = [0x5a; 16];
        let command = encode_wii_write(0x04a2_0001, &data).unwrap();

        assert_eq!(command[5], 0);
        assert_eq!(&command[6..22], &data);
    }

    #[test]
    fn connection_led_mask_matches_wpad_channel_assignment() {
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(wii_remote_led_command),
            [
                [0x11, 0x12],
                [0x11, 0x22],
                [0x11, 0x42],
                [0x11, 0x82],
                [0x11, 0x12],
                [0x11, 0x22],
            ]
        );
    }

    #[test]
    fn write_command_rejects_zero_and_oversized_payloads() {
        assert!(encode_wii_write(0, &[]).is_none());
        assert!(encode_wii_write(0, &[0; 17]).is_none());
    }

    #[test]
    fn stream_encoding_uses_full_wii_report_and_length_header() {
        let data = [0x5a; STREAM_DATA_BYTES];
        let command = encode_wii_stream_data(&data).unwrap();

        assert_eq!(command[0], 0x18);
        assert_eq!(command[1], (STREAM_DATA_BYTES as u8) << 3);
        assert_eq!(&command[2..], &data);
    }

    #[test]
    fn final_stream_chunk_is_zero_padded() {
        let data = [1, 2, 3, 4, 5];
        let chunk = copy_stream_chunk(&data, 0).unwrap();

        assert_eq!(&chunk[..data.len()], &data);
        assert!(chunk[data.len()..].iter().all(|byte| *byte == 0));
        assert!(copy_stream_chunk(&data, data.len()).is_none());
    }
}
