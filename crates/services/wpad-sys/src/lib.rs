#![no_std]
#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

pub use ogc_sys::{Bool, F32};

mod wii_remote;

#[cfg(target_arch = "powerpc")]
mod powerpc_backend;

/// Raw Bluetooth HID state from the shared adapter service. Wii Remote input
/// continues to use WPAD data; other HID controllers use the common decoder.
pub const BLUETOOTH_REPORT_BYTES: usize = 512;
/// `scan_pads` could not consume a Bluetooth HID event because the consumer
/// queue is full. Drain `bluetooth_hid_next_event` and scan again.
pub const WPAD_ERR_HID_QUEUE_FULL: i32 = -1000;
#[derive(Copy, Clone)]
pub struct BluetoothHidState {
    pub connected: bool,
    pub address: ogc_bt_sync::BluetoothAddress,
    pub handle: u16,
    pub family: Option<ogc_bt_sync::ControllerFamily>,
    pub generation: u32,
    pub report_count: u32,
    pub report_len: u16,
    pub report: [u8; BLUETOOTH_REPORT_BYTES],
}
impl BluetoothHidState {
    pub const EMPTY: Self = Self {
        connected: false,
        address: ogc_bt_sync::BluetoothAddress::new([0; 6]),
        handle: 0,
        family: None,
        generation: 0,
        report_count: 0,
        report_len: 0,
        report: [0; BLUETOOTH_REPORT_BYTES],
    };
}
pub fn bluetooth_hid_state(slot: usize) -> BluetoothHidState {
    #[cfg(target_arch = "powerpc")]
    {
        powerpc_backend::bluetooth_hid_state(slot)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = slot;
        BluetoothHidState::EMPTY
    }
}
/// Remove the oldest ordered non-Wii Bluetooth HID event, if one is pending.
/// Drain this queue after every `scan_pads` result, including backpressure and
/// other errors.
pub fn bluetooth_hid_next_event() -> Option<ogc_bt_sync::BluetoothInputEvent> {
    #[cfg(target_arch = "powerpc")]
    {
        powerpc_backend::bluetooth_hid_next_event()
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        None
    }
}
pub fn bluetooth_diagnostics() -> ogc_bt_sync::HostDiagnostics {
    #[cfg(target_arch = "powerpc")]
    {
        powerpc_backend::bluetooth_diagnostics()
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ogc_bt_sync::HidHost::new().diagnostics()
    }
}
/// Read validated calibration from the active Switch HID connection.
/// Returns no calibration until setup completes, or after the link closes.
#[cfg(target_arch = "powerpc")]
pub fn bluetooth_switch_calibration(slot: usize) -> Option<ogc_bt_sync::switch::SwitchCalibration> {
    powerpc_backend::bluetooth_switch_calibration(slot)
}
pub fn bluetooth_shutdown_complete() -> bool {
    #[cfg(target_arch = "powerpc")]
    {
        powerpc_backend::bluetooth_shutdown_complete()
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        true
    }
}

/// Observe Bluetooth polling boundaries from application context. The observer
/// must not call WPAD or wait for IOS. Use it for bounded boot diagnostics.
pub fn set_bluetooth_poll_hook(hook: Option<fn(&'static str)>) {
    #[cfg(target_arch = "powerpc")]
    powerpc_backend::set_bluetooth_poll_hook(hook);
    #[cfg(not(target_arch = "powerpc"))]
    let _ = hook;
}

pub const WPAD_MAX_IR_DOTS: usize = 4;
pub const WPAD_CHAN_ALL: i32 = -1;
pub const WPAD_CHAN_0: i32 = 0;
pub const WPAD_CHAN_1: i32 = 1;
pub const WPAD_CHAN_2: i32 = 2;
pub const WPAD_CHAN_3: i32 = 3;
pub const WPAD_BALANCE_BOARD: i32 = 4;
pub const WPAD_CHAN_UNKNOWN: i32 = 5;
pub const WPAD_MAX_DEVICES: usize = 6;
pub const WPAD_MAX_WIIMOTES: usize = WPAD_MAX_DEVICES;

pub const WPAD_BUTTON_2: u32 = 0x0001;
pub const WPAD_BUTTON_1: u32 = 0x0002;
pub const WPAD_BUTTON_B: u32 = 0x0004;
pub const WPAD_BUTTON_A: u32 = 0x0008;
pub const WPAD_BUTTON_MINUS: u32 = 0x0010;
pub const WPAD_BUTTON_HOME: u32 = 0x0080;
pub const WPAD_BUTTON_LEFT: u32 = 0x0100;
pub const WPAD_BUTTON_RIGHT: u32 = 0x0200;
pub const WPAD_BUTTON_DOWN: u32 = 0x0400;
pub const WPAD_BUTTON_UP: u32 = 0x0800;
pub const WPAD_BUTTON_PLUS: u32 = 0x1000;
pub const WPAD_NUNCHUK_BUTTON_Z: u32 = 0x0001 << 16;
pub const WPAD_NUNCHUK_BUTTON_C: u32 = 0x0002 << 16;
pub const WPAD_CLASSIC_BUTTON_UP: u32 = 0x0001u32 << 16;
pub const WPAD_CLASSIC_BUTTON_LEFT: u32 = 0x0002u32 << 16;
pub const WPAD_CLASSIC_BUTTON_ZR: u32 = 0x0004u32 << 16;
pub const WPAD_CLASSIC_BUTTON_X: u32 = 0x0008u32 << 16;
pub const WPAD_CLASSIC_BUTTON_A: u32 = 0x0010u32 << 16;
pub const WPAD_CLASSIC_BUTTON_Y: u32 = 0x0020u32 << 16;
pub const WPAD_CLASSIC_BUTTON_B: u32 = 0x0040u32 << 16;
pub const WPAD_CLASSIC_BUTTON_ZL: u32 = 0x0080u32 << 16;
pub const WPAD_CLASSIC_BUTTON_FULL_R: u32 = 0x0200u32 << 16;
pub const WPAD_CLASSIC_BUTTON_PLUS: u32 = 0x0400u32 << 16;
pub const WPAD_CLASSIC_BUTTON_HOME: u32 = 0x0800u32 << 16;
pub const WPAD_CLASSIC_BUTTON_MINUS: u32 = 0x1000u32 << 16;
pub const WPAD_CLASSIC_BUTTON_FULL_L: u32 = 0x2000u32 << 16;
pub const WPAD_CLASSIC_BUTTON_DOWN: u32 = 0x4000u32 << 16;
pub const WPAD_CLASSIC_BUTTON_RIGHT: u32 = 0x8000u32 << 16;
pub const WPAD_WII_U_PRO_BUTTON_RSTICK: u32 = 0x0001;
pub const WPAD_WII_U_PRO_BUTTON_LSTICK: u32 = 0x0002;
pub const WPAD_GUITAR_HERO_3_BUTTON_STRUM_UP: u32 = 0x0001 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_YELLOW: u32 = 0x0008 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_GREEN: u32 = 0x0010 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_BLUE: u32 = 0x0020 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_RED: u32 = 0x0040 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_ORANGE: u32 = 0x0080 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_PLUS: u32 = 0x0400 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_MINUS: u32 = 0x1000 << 16;
pub const WPAD_GUITAR_HERO_3_BUTTON_STRUM_DOWN: u32 = 0x4000 << 16;

pub const WPAD_EXP_NONE: u32 = 0;
pub const WPAD_EXP_NUNCHUK: u32 = 1;
pub const WPAD_EXP_CLASSIC: u32 = 2;
pub const WPAD_EXP_GUITARHERO3: u32 = 3;
pub const WPAD_EXP_WIIBOARD: u32 = 4;
pub const WPAD_EXP_UNKNOWN: u32 = 255;

pub const WPAD_FMT_BTNS: i32 = 0;
pub const WPAD_FMT_BTNS_ACC: i32 = 1;
pub const WPAD_FMT_BTNS_ACC_IR: i32 = 2;

pub const WPAD_STATE_DISABLED: i32 = 0;
pub const WPAD_STATE_ENABLING: i32 = 1;
pub const WPAD_STATE_ENABLED: i32 = 2;

pub const WPAD_ERR_NONE: i16 = 0;
pub const WPAD_ERR_NO_CONTROLLER: i16 = -1;
pub const WPAD_ERR_NOT_READY: i16 = -2;
pub const WPAD_ERR_TRANSFER: i16 = -3;
pub const WPAD_ERR_NONEREGISTERED: i16 = -4;
pub const WPAD_ERR_UNKNOWN: i16 = -5;
pub const WPAD_ERR_BAD_CHANNEL: i16 = -6;
pub const WPAD_ERR_QUEUE_EMPTY: i16 = -7;
pub const WPAD_ERR_BADVALUE: i16 = -8;
pub const WPAD_ERR_BADCONF: i16 = -9;

pub const WPAD_DATA_BUTTONS: u32 = 0x01;
pub const WPAD_DATA_ACCEL: u32 = 0x02;
pub const WPAD_DATA_EXPANSION: u32 = 0x04;
pub const WPAD_DATA_IR: u32 = 0x08;
pub const WPAD_ENC_FIRST: u32 = 0x00;
pub const WPAD_ENC_CONT: u32 = 0x01;
pub const WPAD_THRESH_IGNORE: i32 = -1;
pub const WPAD_THRESH_ANY: i32 = 0;
pub const WPAD_THRESH_DEFAULT_BUTTONS: i32 = 0;
pub const WPAD_THRESH_DEFAULT_IR: i32 = WPAD_THRESH_IGNORE;
pub const WPAD_THRESH_DEFAULT_ACCEL: i32 = 20;
pub const WPAD_THRESH_DEFAULT_JOYSTICK: i32 = 2;
pub const WPAD_THRESH_DEFAULT_BALANCEBOARD: i32 = 60;
pub const WPAD_THRESH_DEFAULT_MOTION_PLUS: i32 = 100;

pub const WPAD_DISCON_AUTH_FAILURE: u8 = 0x05;
pub const WPAD_DISCON_TIMEOUT: u8 = 0x08;
pub const WPAD_DISCON_SYNC_PRESSED: u8 = 0x13;
pub const WPAD_DISCON_BATTERY_DIED: u8 = 0x14;
pub const WPAD_DISCON_POWER_OFF: u8 = 0x15;
pub const WPAD_DISCON_IDLE_TIMEOUT: u8 = 0x16;
pub const WPAD_DISCON_REPEATED_ATTEMPTS: u8 = 0x17;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Vec2b {
    pub x: u8,
    pub y: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Vec3w {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Orient {
    pub roll: F32,
    pub pitch: F32,
    pub yaw: F32,
    pub a_roll: F32,
    pub a_pitch: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GForce {
    pub x: F32,
    pub y: F32,
    pub z: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Accel {
    pub cal_zero: Vec3w,
    pub cal_g: Vec3w,
    pub st_roll: F32,
    pub st_pitch: F32,
    pub st_alpha: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct IrDot {
    pub visible: u8,
    pub rx: i16,
    pub ry: i16,
    pub size: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct FDot {
    pub x: F32,
    pub y: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SensorBar {
    pub dots: [FDot; 2],
    pub acc_dots: [FDot; 2],
    pub rot_dots: [FDot; 2],
    pub angle: F32,
    pub off_angle: F32,
    pub score: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Ir {
    pub dot: [IrDot; 4],
    pub num_dots: u8,
    pub state: i32,
    pub raw_valid: i32,
    pub sensorbar: SensorBar,
    pub ax: F32,
    pub ay: F32,
    pub distance: F32,
    pub z: F32,
    pub angle: F32,
    pub smooth_valid: i32,
    pub sx: F32,
    pub sy: F32,
    pub error_cnt: F32,
    pub glitch_cnt: F32,
    pub valid: i32,
    pub x: F32,
    pub y: F32,
    pub aspect: i32,
    pub pos: i32,
    pub vres: [u32; 2],
    pub offset: [i32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Joystick {
    pub max: Vec2b,
    pub min: Vec2b,
    pub center: Vec2b,
    pub pos: Vec2b,
    pub ang: F32,
    pub mag: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Nunchuk {
    pub accel_calib: Accel,
    pub js: Joystick,
    pub flags: *mut i32,
    pub btns: u8,
    pub btns_last: u8,
    pub btns_held: u8,
    pub btns_released: u8,
    pub accel: Vec3w,
    pub orient: Orient,
    pub gforce: GForce,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct ClassicCtrl {
    pub btns: u32,
    pub btns_last: u32,
    pub btns_held: u32,
    pub btns_released: u32,
    pub rs_raw: u8,
    pub ls_raw: u8,
    pub r_shoulder: F32,
    pub l_shoulder: F32,
    pub ljs: Joystick,
    pub rjs: Joystick,
    pub type_: u8,
    pub charging: u8,
    pub wired: u8,
    pub battery: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct GuitarHero3 {
    pub btns: i16,
    pub btns_last: i16,
    pub btns_held: i16,
    pub btns_released: i16,
    pub wb_raw: u8,
    pub whammy_bar: F32,
    pub tb_raw: u8,
    pub touch_bar: i32,
    pub js: Joystick,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct WiiBoard {
    pub tl: F32,
    pub tr: F32,
    pub bl: F32,
    pub br: F32,
    pub rtl: i16,
    pub rtr: i16,
    pub rbl: i16,
    pub rbr: i16,
    pub ctl: [i16; 3],
    pub ctr: [i16; 3],
    pub cbl: [i16; 3],
    pub cbr: [i16; 3],
    pub x: F32,
    pub y: F32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MotionPlus {
    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
    pub status: u8,
    pub ext: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ExpansionUnion {
    pub nunchuk: Nunchuk,
    pub classic: ClassicCtrl,
    pub gh3: GuitarHero3,
    pub wb: WiiBoard,
    pub mp: MotionPlus,
}

impl Default for ExpansionUnion {
    fn default() -> Self {
        Self {
            mp: MotionPlus::default(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Expansion {
    pub type_: i32,
    pub u: ExpansionUnion,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct WpadData {
    pub err: i16,
    pub data_present: u32,
    pub battery_level: u8,
    pub btns_h: u32,
    pub btns_l: u32,
    pub btns_d: u32,
    pub btns_u: u32,
    pub ir: Ir,
    pub accel: Vec3w,
    pub orient: Orient,
    pub gforce: GForce,
    pub exp: Expansion,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct WpadEncStatus {
    pub data: [u8; 32],
}

pub type WpadDataCallback = Option<unsafe extern "C" fn(chan: i32, data: *const WpadData)>;
pub type WpadShutdownCallback = Option<unsafe extern "C" fn(chan: i32)>;
pub type WpadDisconnectCallback = Option<unsafe extern "C" fn(chan: i32, reason: u8)>;
pub type WpadStatusCallback = Option<unsafe extern "C" fn(chan: i32)>;
pub type WpadHostSyncBtnCallback = Option<unsafe extern "C" fn(held: u32)>;

static WPAD_INITED: AtomicBool = AtomicBool::new(false);
static WPAD_SEARCHING: AtomicBool = AtomicBool::new(false);
static WPAD_GLOBAL_STATUS: AtomicI32 = AtomicI32::new(WPAD_STATE_DISABLED);
static WPAD_IDLE_TIMEOUT: AtomicU32 = AtomicU32::new(0);
static WPAD_CONNECTED: [AtomicBool; WPAD_MAX_DEVICES] =
    [const { AtomicBool::new(false) }; WPAD_MAX_DEVICES];
// Keep the consumer's previous scan separate from incoming report state.
// A producer can update WPAD_DATA_STATE several times before read_pending.
static WPAD_CONSUMED_BUTTONS: [AtomicU32; WPAD_MAX_DEVICES] =
    [const { AtomicU32::new(0) }; WPAD_MAX_DEVICES];
static WPAD_FORMAT: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_FMT_BTNS) }; WPAD_MAX_DEVICES];
static WPAD_SPEAKER: [AtomicBool; WPAD_MAX_DEVICES] =
    [const { AtomicBool::new(false) }; WPAD_MAX_DEVICES];
static WPAD_RUMBLE: [AtomicBool; WPAD_MAX_DEVICES] =
    [const { AtomicBool::new(false) }; WPAD_MAX_DEVICES];
static WPAD_DROPPED: [AtomicU32; WPAD_MAX_DEVICES] =
    [const { AtomicU32::new(0) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_BTNS: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_BUTTONS) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_IR: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_IR) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_ACCEL: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_ACCEL) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_JS: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_JOYSTICK) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_WB: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_BALANCEBOARD) }; WPAD_MAX_DEVICES];
static WPAD_IDLE_MP: [AtomicI32; WPAD_MAX_DEVICES] =
    [const { AtomicI32::new(WPAD_THRESH_DEFAULT_MOTION_PLUS) }; WPAD_MAX_DEVICES];
static WPAD_STREAM_LEN: [AtomicU32; WPAD_MAX_DEVICES] =
    [const { AtomicU32::new(0) }; WPAD_MAX_DEVICES];
const WPAD_REPORT_RING_LEN: usize = 8;

static mut WPAD_DATA_STATE: [WpadData; WPAD_MAX_DEVICES] = [WpadData {
    err: WPAD_ERR_NO_CONTROLLER,
    data_present: 0,
    battery_level: 0,
    btns_h: 0,
    btns_l: 0,
    btns_d: 0,
    btns_u: 0,
    ir: Ir {
        dot: [IrDot {
            visible: 0,
            rx: 0,
            ry: 0,
            size: 0,
        }; 4],
        num_dots: 0,
        state: 0,
        raw_valid: 0,
        sensorbar: SensorBar {
            dots: [FDot { x: 0.0, y: 0.0 }; 2],
            acc_dots: [FDot { x: 0.0, y: 0.0 }; 2],
            rot_dots: [FDot { x: 0.0, y: 0.0 }; 2],
            angle: 0.0,
            off_angle: 0.0,
            score: 0.0,
        },
        ax: 0.0,
        ay: 0.0,
        distance: 0.0,
        z: 0.0,
        angle: 0.0,
        smooth_valid: 0,
        sx: 0.0,
        sy: 0.0,
        error_cnt: 0.0,
        glitch_cnt: 0.0,
        valid: 0,
        x: 0.0,
        y: 0.0,
        aspect: 0,
        pos: 0,
        vres: [0; 2],
        offset: [0; 2],
    },
    accel: Vec3w { x: 0, y: 0, z: 0 },
    orient: Orient {
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
        a_roll: 0.0,
        a_pitch: 0.0,
    },
    gforce: GForce {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    exp: Expansion {
        type_: WPAD_EXP_NONE as i32,
        u: ExpansionUnion {
            mp: MotionPlus {
                rx: 0,
                ry: 0,
                rz: 0,
                status: 0,
                ext: 0,
            },
        },
    },
}; WPAD_MAX_DEVICES];

static mut POWER_CB: WpadShutdownCallback = None;
static mut BATTERY_CB: WpadShutdownCallback = None;
static mut DISCONNECT_CB: WpadDisconnectCallback = None;
static mut HOSTSYNC_CB: WpadHostSyncBtnCallback = None;
static mut STATUS_CB: WpadStatusCallback = None;
static mut READPENDING_CB: WpadDataCallback = None;
static mut EVENT_BUFS: [*mut WpadData; WPAD_MAX_DEVICES] =
    [core::ptr::null_mut(); WPAD_MAX_DEVICES];
static mut EVENT_BUF_COUNTS: [u32; WPAD_MAX_DEVICES] = [0; WPAD_MAX_DEVICES];
static mut EVENT_BUF_WRITE: [u32; WPAD_MAX_DEVICES] = [0; WPAD_MAX_DEVICES];
static mut REPORT_RING: [[WpadData; WPAD_REPORT_RING_LEN]; WPAD_MAX_DEVICES] =
    [EMPTY_REPORT_RING; WPAD_MAX_DEVICES];
static mut REPORT_RING_HEAD: [usize; WPAD_MAX_DEVICES] = [0; WPAD_MAX_DEVICES];
static mut REPORT_RING_TAIL: [usize; WPAD_MAX_DEVICES] = [0; WPAD_MAX_DEVICES];
static mut REPORT_RING_COUNT: [usize; WPAD_MAX_DEVICES] = [0; WPAD_MAX_DEVICES];

#[inline]
const fn wpad_host_compatibility_enabled() -> bool {
    !cfg!(target_arch = "powerpc")
}

const EMPTY_WPAD_DATA: WpadData = WpadData {
    err: WPAD_ERR_NO_CONTROLLER,
    data_present: 0,
    battery_level: 0,
    btns_h: 0,
    btns_l: 0,
    btns_d: 0,
    btns_u: 0,
    ir: Ir {
        dot: [IrDot {
            visible: 0,
            rx: 0,
            ry: 0,
            size: 0,
        }; 4],
        num_dots: 0,
        state: 0,
        raw_valid: 0,
        sensorbar: SensorBar {
            dots: [FDot { x: 0.0, y: 0.0 }; 2],
            acc_dots: [FDot { x: 0.0, y: 0.0 }; 2],
            rot_dots: [FDot { x: 0.0, y: 0.0 }; 2],
            angle: 0.0,
            off_angle: 0.0,
            score: 0.0,
        },
        ax: 0.0,
        ay: 0.0,
        distance: 0.0,
        z: 0.0,
        angle: 0.0,
        smooth_valid: 0,
        sx: 0.0,
        sy: 0.0,
        error_cnt: 0.0,
        glitch_cnt: 0.0,
        valid: 0,
        x: 0.0,
        y: 0.0,
        aspect: 0,
        pos: 0,
        vres: [0; 2],
        offset: [0; 2],
    },
    accel: Vec3w { x: 0, y: 0, z: 0 },
    orient: Orient {
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
        a_roll: 0.0,
        a_pitch: 0.0,
    },
    gforce: GForce {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    },
    exp: Expansion {
        type_: WPAD_EXP_NONE as i32,
        u: ExpansionUnion {
            mp: MotionPlus {
                rx: 0,
                ry: 0,
                rz: 0,
                status: 0,
                ext: 0,
            },
        },
    },
};

const EMPTY_REPORT_RING: [WpadData; WPAD_REPORT_RING_LEN] = [EMPTY_WPAD_DATA; WPAD_REPORT_RING_LEN];

fn valid_chan(chan: i32) -> Option<usize> {
    if (WPAD_CHAN_0..WPAD_MAX_DEVICES as i32).contains(&chan) {
        Some(chan as usize)
    } else {
        None
    }
}

fn reset_data(chan: usize) {
    unsafe {
        WPAD_DATA_STATE[chan].err = if WPAD_CONNECTED[chan].load(Ordering::Acquire) {
            WPAD_ERR_NONE
        } else {
            WPAD_ERR_NO_CONTROLLER
        };
        WPAD_DATA_STATE[chan].data_present = 0;
        WPAD_DATA_STATE[chan].battery_level = 0;
        WPAD_DATA_STATE[chan].btns_h = 0;
        WPAD_DATA_STATE[chan].btns_l = 0;
        WPAD_DATA_STATE[chan].btns_d = 0;
        WPAD_DATA_STATE[chan].btns_u = 0;
        WPAD_DATA_STATE[chan].ir = Ir::default();
        WPAD_DATA_STATE[chan].accel = Vec3w::default();
        WPAD_DATA_STATE[chan].orient = Orient::default();
        WPAD_DATA_STATE[chan].gforce = GForce::default();
        WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NONE as i32;
        WPAD_DATA_STATE[chan].exp.u.mp = MotionPlus::default();
        REPORT_RING_HEAD[chan] = 0;
        REPORT_RING_TAIL[chan] = 0;
        REPORT_RING_COUNT[chan] = 0;
    }
    WPAD_DROPPED[chan].store(0, Ordering::Release);
    WPAD_CONSUMED_BUTTONS[chan].store(0, Ordering::Release);
}

fn notify_status(chan: usize) {
    unsafe {
        if let Some(cb) = STATUS_CB {
            cb(chan as i32);
        }
    }
}

fn set_connected(chan: usize, connected: bool) {
    WPAD_CONNECTED[chan].store(connected, Ordering::Release);
    unsafe {
        WPAD_DATA_STATE[chan].err = if connected {
            WPAD_ERR_NONE
        } else {
            WPAD_ERR_NO_CONTROLLER
        };
        if !connected {
            WPAD_DATA_STATE[chan].data_present = 0;
            WPAD_DATA_STATE[chan].btns_h = 0;
            WPAD_DATA_STATE[chan].btns_l = 0;
            WPAD_DATA_STATE[chan].btns_d = 0;
            WPAD_DATA_STATE[chan].btns_u = 0;
        }
    }
    notify_status(chan);
}

fn push_event(chan: usize) {
    unsafe {
        if REPORT_RING_COUNT[chan] == WPAD_REPORT_RING_LEN {
            REPORT_RING_HEAD[chan] = (REPORT_RING_HEAD[chan] + 1) % WPAD_REPORT_RING_LEN;
            WPAD_DROPPED[chan].fetch_add(1, Ordering::AcqRel);
        } else {
            REPORT_RING_COUNT[chan] += 1;
        }
        REPORT_RING[chan][REPORT_RING_TAIL[chan]] = WPAD_DATA_STATE[chan];
        REPORT_RING_TAIL[chan] = (REPORT_RING_TAIL[chan] + 1) % WPAD_REPORT_RING_LEN;
        if WPAD_CONNECTED[chan].load(Ordering::Acquire) {
            if let Some(cb) = READPENDING_CB {
                cb(chan as i32, core::ptr::addr_of!(WPAD_DATA_STATE[chan]));
            }
            if WPAD_DATA_STATE[chan].battery_level <= 5
                && let Some(cb) = BATTERY_CB
            {
                cb(chan as i32);
            }
        }
        let bufs = EVENT_BUFS[chan];
        let count = EVENT_BUF_COUNTS[chan];
        if !bufs.is_null() && count != 0 {
            let idx = (EVENT_BUF_WRITE[chan] % count) as usize;
            *bufs.add(idx) = WPAD_DATA_STATE[chan];
            EVENT_BUF_WRITE[chan] = EVENT_BUF_WRITE[chan].wrapping_add(1);
            if EVENT_BUF_WRITE[chan] > count {
                WPAD_DROPPED[chan].fetch_add(1, Ordering::AcqRel);
            }
        }
    }
    if cfg!(target_arch = "powerpc") {
        let current = unsafe { WPAD_DATA_STATE[chan] };
        powerpc_backend::mark_activity_for_data(chan, &current);
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
fn apply_buttons(chan: usize, buttons: u32) {
    unsafe {
        let last = WPAD_DATA_STATE[chan].btns_h;
        WPAD_DATA_STATE[chan].btns_l = last;
        WPAD_DATA_STATE[chan].btns_h = buttons;
        WPAD_DATA_STATE[chan].btns_d = buttons & !last;
        WPAD_DATA_STATE[chan].btns_u = last & !buttons;
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_BUTTONS;
    }
    push_event(chan);
}

fn connect_with_defaults(chan: usize) {
    set_connected(chan, true);
    unsafe {
        WPAD_DATA_STATE[chan].battery_level = 0x80;
        WPAD_DATA_STATE[chan].data_present = WPAD_DATA_BUTTONS;
    }
    push_event(chan);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn set_buttons(chan: usize, buttons: u32) {
    apply_buttons(chan, buttons);
}
pub(crate) fn powerpc_runtime_reset() {
    WPAD_INITED.store(false, Ordering::Release);
    WPAD_SEARCHING.store(false, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_DISABLED, Ordering::Release);
}
pub(crate) fn powerpc_runtime_mark_initialized() {
    WPAD_INITED.store(true, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLING, Ordering::Release);
}
pub(crate) fn powerpc_runtime_begin_search() {
    WPAD_SEARCHING.store(true, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLING, Ordering::Release);
}
pub(crate) fn powerpc_runtime_end_search() {
    WPAD_SEARCHING.store(false, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLED, Ordering::Release);
}
pub fn init() -> i32 {
    if WPAD_INITED.load(Ordering::Acquire) {
        return WPAD_ERR_NONE as i32;
    }
    if cfg!(target_arch = "powerpc") {
        powerpc_runtime_reset();
        for chan in 0..WPAD_MAX_DEVICES {
            WPAD_CONNECTED[chan].store(false, Ordering::Release);
            WPAD_FORMAT[chan].store(WPAD_FMT_BTNS, Ordering::Release);
            WPAD_SPEAKER[chan].store(false, Ordering::Release);
            WPAD_RUMBLE[chan].store(false, Ordering::Release);
            WPAD_STREAM_LEN[chan].store(0, Ordering::Release);
            WPAD_IDLE_BTNS[chan].store(WPAD_THRESH_DEFAULT_BUTTONS, Ordering::Release);
            WPAD_IDLE_IR[chan].store(WPAD_THRESH_DEFAULT_IR, Ordering::Release);
            WPAD_IDLE_ACCEL[chan].store(WPAD_THRESH_DEFAULT_ACCEL, Ordering::Release);
            WPAD_IDLE_JS[chan].store(WPAD_THRESH_DEFAULT_JOYSTICK, Ordering::Release);
            WPAD_IDLE_WB[chan].store(WPAD_THRESH_DEFAULT_BALANCEBOARD, Ordering::Release);
            WPAD_IDLE_MP[chan].store(WPAD_THRESH_DEFAULT_MOTION_PLUS, Ordering::Release);
            unsafe {
                EVENT_BUFS[chan] = core::ptr::null_mut();
                EVENT_BUF_COUNTS[chan] = 0;
                EVENT_BUF_WRITE[chan] = 0;
            }
            reset_data(chan);
        }
        let rc = powerpc_backend::init();
        if rc == WPAD_ERR_NONE as i32 {
            powerpc_runtime_mark_initialized();
        }
        return rc;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    WPAD_INITED.store(true, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLED, Ordering::Release);
    for chan in 0..WPAD_MAX_DEVICES {
        WPAD_CONNECTED[chan].store(false, Ordering::Release);
        WPAD_FORMAT[chan].store(WPAD_FMT_BTNS, Ordering::Release);
        WPAD_SPEAKER[chan].store(false, Ordering::Release);
        WPAD_RUMBLE[chan].store(false, Ordering::Release);
        WPAD_STREAM_LEN[chan].store(0, Ordering::Release);
        WPAD_IDLE_BTNS[chan].store(WPAD_THRESH_DEFAULT_BUTTONS, Ordering::Release);
        WPAD_IDLE_IR[chan].store(WPAD_THRESH_DEFAULT_IR, Ordering::Release);
        WPAD_IDLE_ACCEL[chan].store(WPAD_THRESH_DEFAULT_ACCEL, Ordering::Release);
        WPAD_IDLE_JS[chan].store(WPAD_THRESH_DEFAULT_JOYSTICK, Ordering::Release);
        WPAD_IDLE_WB[chan].store(WPAD_THRESH_DEFAULT_BALANCEBOARD, Ordering::Release);
        WPAD_IDLE_MP[chan].store(WPAD_THRESH_DEFAULT_MOTION_PLUS, Ordering::Release);
        unsafe {
            EVENT_BUFS[chan] = core::ptr::null_mut();
            EVENT_BUF_COUNTS[chan] = 0;
            EVENT_BUF_WRITE[chan] = 0;
        }
        reset_data(chan);
    }
    WPAD_ERR_NONE as i32
}
pub fn control_speaker(chan: i32, enable: i32) -> i32 {
    if chan == WPAD_CHAN_ALL {
        if cfg!(target_arch = "powerpc") {
            if WPAD_GLOBAL_STATUS.load(Ordering::Acquire) == WPAD_STATE_DISABLED {
                return WPAD_ERR_NOT_READY as i32;
            }
            return apply_all_channels(|chan| {
                powerpc_backend::control_speaker_if_present(chan, enable != 0)
            });
        }
        return apply_all_channels(|chan| control_speaker(chan as i32, enable));
    }
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return powerpc_backend::control_speaker(chan, enable != 0);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    WPAD_SPEAKER[chan].store(enable != 0, Ordering::Release);
    WPAD_ERR_NONE as i32
}

/// Reads the host-compat event state for a controller channel into a caller buffer.
///
/// # Safety
/// `data` must be either null or a valid, writable pointer to one `WpadData`
/// value for the duration of this call.
pub unsafe fn read_event(chan: i32, data: *mut WpadData) -> i32 {
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        if !WPAD_INITED.load(Ordering::Acquire) {
            return WPAD_ERR_NOT_READY as i32;
        }
        if !WPAD_CONNECTED[chan].load(Ordering::Acquire) {
            return WPAD_ERR_NO_CONTROLLER as i32;
        }
        if unsafe { REPORT_RING_COUNT[chan] } == 0 {
            return WPAD_ERR_QUEUE_EMPTY as i32;
        }
        unsafe {
            let entry = REPORT_RING[chan][REPORT_RING_HEAD[chan]];
            REPORT_RING_HEAD[chan] = (REPORT_RING_HEAD[chan] + 1) % WPAD_REPORT_RING_LEN;
            REPORT_RING_COUNT[chan] -= 1;
            WPAD_DATA_STATE[chan] = entry;
            if !data.is_null() {
                *data = entry;
            }
        }
        return WPAD_ERR_NONE as i32;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if !WPAD_INITED.load(Ordering::Acquire) {
        return WPAD_ERR_NOT_READY as i32;
    }
    if !WPAD_CONNECTED[chan].load(Ordering::Acquire) {
        return WPAD_ERR_NO_CONTROLLER as i32;
    }
    if unsafe { REPORT_RING_COUNT[chan] } == 0 {
        return WPAD_ERR_QUEUE_EMPTY as i32;
    }
    unsafe {
        let entry = REPORT_RING[chan][REPORT_RING_HEAD[chan]];
        REPORT_RING_HEAD[chan] = (REPORT_RING_HEAD[chan] + 1) % WPAD_REPORT_RING_LEN;
        REPORT_RING_COUNT[chan] -= 1;
        WPAD_DATA_STATE[chan] = entry;
        if !data.is_null() {
            *data = entry;
        }
    }
    if data.is_null() {
        return WPAD_ERR_NONE as i32;
    }
    WPAD_ERR_NONE as i32
}
pub fn dropped_events(chan: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if chan == WPAD_CHAN_ALL {
            let mut total = 0;
            for dropped in WPAD_DROPPED.iter().take(WPAD_MAX_DEVICES) {
                total += dropped.swap(0, Ordering::AcqRel) as i32;
            }
            return total;
        }
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return WPAD_DROPPED[chan].swap(0, Ordering::AcqRel) as i32;
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    if chan == WPAD_CHAN_ALL {
        let mut total = 0;
        for dropped in WPAD_DROPPED.iter().take(WPAD_MAX_DEVICES) {
            total += dropped.swap(0, Ordering::AcqRel) as i32;
        }
        return total;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    WPAD_DROPPED[chan].swap(0, Ordering::AcqRel) as i32
}
pub fn flush(chan: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if chan == WPAD_CHAN_ALL {
            let mut total = 0;
            for idx in 0..WPAD_MAX_DEVICES {
                let count = unsafe { REPORT_RING_COUNT[idx] as i32 };
                unsafe {
                    REPORT_RING_HEAD[idx] = 0;
                    REPORT_RING_TAIL[idx] = 0;
                    REPORT_RING_COUNT[idx] = 0;
                }
                total += count;
            }
            return total;
        }
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        let count = unsafe { REPORT_RING_COUNT[chan] as i32 };
        unsafe {
            REPORT_RING_HEAD[chan] = 0;
            REPORT_RING_TAIL[chan] = 0;
            REPORT_RING_COUNT[chan] = 0;
        }
        return count;
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    if chan == WPAD_CHAN_ALL {
        let mut total = 0;
        for idx in 0..WPAD_MAX_DEVICES {
            let count = unsafe { REPORT_RING_COUNT[idx] as i32 };
            unsafe {
                REPORT_RING_HEAD[idx] = 0;
                REPORT_RING_TAIL[idx] = 0;
                REPORT_RING_COUNT[idx] = 0;
            }
            total += count;
        }
        return total;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    let count = unsafe { REPORT_RING_COUNT[chan] as i32 };
    unsafe {
        REPORT_RING_HEAD[chan] = 0;
        REPORT_RING_TAIL[chan] = 0;
        REPORT_RING_COUNT[chan] = 0;
    }
    count
}
pub fn read_pending(chan: i32, datacb: WpadDataCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        unsafe {
            READPENDING_CB = datacb;
        }
        if chan == WPAD_CHAN_ALL {
            let mut count = 0;
            for idx in 0..WPAD_MAX_DEVICES {
                let ret = read_pending(idx as i32, datacb);
                if ret >= 0 {
                    count += ret;
                }
            }
            return count;
        }
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return read_pending_channel(chan, datacb);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    unsafe {
        READPENDING_CB = datacb;
    }
    if chan == WPAD_CHAN_ALL {
        let mut count = 0;
        for idx in 0..WPAD_MAX_DEVICES {
            let ret = read_pending(idx as i32, datacb);
            if ret >= 0 {
                count += ret;
            }
        }
        return count;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    read_pending_channel(chan, datacb)
}

fn read_pending_channel(chan: usize, datacb: WpadDataCallback) -> i32 {
    let mut data = EMPTY_WPAD_DATA;
    let mut held = WPAD_CONSUMED_BUTTONS[chan].load(Ordering::Acquire);
    let last = held;
    let mut previous = held;
    let mut changed = 0;
    let mut count = 0;
    loop {
        let ret = unsafe { read_event(chan as i32, &mut data) };
        if ret < 0 {
            if ret != WPAD_ERR_QUEUE_EMPTY as i32 {
                return ret;
            }
            break;
        }
        unsafe {
            if let Some(cb) = datacb {
                cb(chan as i32, core::ptr::addr_of!(data));
            }
        }
        let buttons = data.btns_h;
        let transitions = (buttons ^ previous) & !changed;
        previous = buttons;
        held = (held & !transitions) | (buttons & transitions);
        changed |= transitions;
        count += 1;
    }
    unsafe {
        let data = &mut WPAD_DATA_STATE[chan];
        data.btns_h = held;
        data.btns_l = last;
        data.btns_d = held & !last;
        data.btns_u = !held & last;
    }
    WPAD_CONSUMED_BUTTONS[chan].store(held, Ordering::Release);
    count
}
pub fn set_data_format(chan: i32, fmt: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::set_data_format(chan, fmt);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    if !(WPAD_FMT_BTNS..=WPAD_FMT_BTNS_ACC_IR).contains(&fmt) {
        return WPAD_ERR_BADVALUE as i32;
    }
    if chan == WPAD_CHAN_ALL {
        for format in &WPAD_FORMAT {
            format.store(fmt, Ordering::Release);
        }
        return WPAD_ERR_NONE as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    WPAD_FORMAT[chan].store(fmt, Ordering::Release);
    WPAD_ERR_NONE as i32
}
pub fn set_motion_plus(chan: i32, _enable: u8) -> i32 {
    if chan == WPAD_CHAN_ALL {
        let status = WPAD_GLOBAL_STATUS.load(Ordering::Acquire);
        if status == WPAD_STATE_DISABLED {
            return WPAD_ERR_NOT_READY as i32;
        }
        return apply_motion_plus_all(_enable != 0, |chan, enable| {
            if cfg!(target_arch = "powerpc") {
                powerpc_backend::set_motion_plus_if_present(chan, enable)
            } else {
                set_motion_plus_host_state(chan, enable)
            }
        });
    }
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return powerpc_backend::set_motion_plus(chan, _enable != 0);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    set_motion_plus_host_state(chan, _enable != 0)
}

fn set_motion_plus_host_state(chan: usize, enable: bool) -> i32 {
    unsafe {
        if !enable {
            WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NONE as i32;
            WPAD_DATA_STATE[chan].exp.u.mp = MotionPlus::default();
            WPAD_DATA_STATE[chan].data_present &= !WPAD_DATA_EXPANSION;
        } else {
            WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_UNKNOWN as i32;
            WPAD_DATA_STATE[chan].exp.u.mp = MotionPlus {
                status: 1,
                ext: 1,
                ..MotionPlus::default()
            };
            WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
        }
    }
    WPAD_ERR_NONE as i32
}

fn apply_all_channels(mut apply: impl FnMut(usize) -> i32) -> i32 {
    for chan in 0..WPAD_MAX_DEVICES {
        let status = apply(chan);
        if status < WPAD_ERR_NONE as i32 {
            return status;
        }
    }
    WPAD_ERR_NONE as i32
}

fn apply_motion_plus_all(enable: bool, mut apply: impl FnMut(usize, bool) -> i32) -> i32 {
    apply_all_channels(|chan| apply(chan, enable))
}
pub fn set_v_res(chan: i32, xres: u32, yres: u32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        powerpc_backend::set_ir_resolution(chan, xres, yres);
        return WPAD_ERR_NONE as i32;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    unsafe {
        WPAD_DATA_STATE[chan].ir.vres = [xres, yres];
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_IR;
    }
    WPAD_ERR_NONE as i32
}
pub fn get_status() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return WPAD_GLOBAL_STATUS.load(Ordering::Acquire);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_STATE_DISABLED;
    }
    WPAD_GLOBAL_STATUS.load(Ordering::Acquire)
}

/// Reports the host-compat expansion type for a controller channel.
///
/// # Safety
/// `type_` may be null. When non-null it must be a valid, writable pointer to
/// one `u32` value for the duration of this call.
pub unsafe fn probe(chan: i32, type_: *mut u32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::probe(chan, type_);
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let connected = WPAD_CONNECTED[chan].load(Ordering::Acquire);
    if connected && !type_.is_null() {
        unsafe {
            *type_ = probe_expansion_type_for_wii(WPAD_DATA_STATE[chan].exp.type_);
        }
    }
    if connected {
        WPAD_ERR_NONE as i32
    } else {
        WPAD_ERR_NO_CONTROLLER as i32
    }
}
pub(crate) fn probe_expansion_type_for_wii(exp_type: i32) -> u32 {
    match exp_type as u32 {
        WPAD_EXP_NONE | WPAD_EXP_NUNCHUK | WPAD_EXP_CLASSIC | WPAD_EXP_GUITARHERO3
        | WPAD_EXP_WIIBOARD => exp_type as u32,
        _ => WPAD_EXP_NONE,
    }
}

/// Installs caller-owned event buffers for a channel in the host-compat state.
///
/// # Safety
/// `bufs` must point to at least `cnt` writable `WpadData` entries when
/// non-null. The caller is responsible for alignment and lifetime exactly as in
/// the C ABI.
pub unsafe fn set_event_bufs(chan: i32, bufs: *mut WpadData, cnt: u32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        if bufs.is_null() || cnt == 0 {
            return WPAD_ERR_BADVALUE as i32;
        }
        unsafe {
            EVENT_BUFS[chan] = bufs;
            EVENT_BUF_COUNTS[chan] = cnt;
            EVENT_BUF_WRITE[chan] = 0;
            *bufs = WPAD_DATA_STATE[chan];
        }
        return WPAD_ERR_NONE as i32;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if bufs.is_null() || cnt == 0 {
        return WPAD_ERR_BADVALUE as i32;
    }
    unsafe {
        EVENT_BUFS[chan] = bufs;
        EVENT_BUF_COUNTS[chan] = cnt;
        EVENT_BUF_WRITE[chan] = 0;
        *bufs = WPAD_DATA_STATE[chan];
    }
    WPAD_ERR_NONE as i32
}

pub fn disconnect(chan: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::disconnect(chan);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    set_connected(chan, false);
    unsafe {
        if let Some(cb) = DISCONNECT_CB {
            cb(chan as i32, WPAD_DISCON_POWER_OFF);
        }
    }
    WPAD_ERR_NONE as i32
}
pub fn is_speaker_enabled(chan: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return if WPAD_SPEAKER[chan].load(Ordering::Acquire) {
            1
        } else {
            0
        };
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if WPAD_SPEAKER[chan].load(Ordering::Acquire) {
        1
    } else {
        0
    }
}

/// Queues speaker stream data for transmission in 20-byte reports.
///
/// # Safety
/// When running on Wii, `buf` must point to `len` readable bytes and remain
/// readable until the stream completes, another stream replaces it, or WPAD
/// shuts down. The Wii backend reads each 20-byte chunk from this buffer on a
/// timer.
pub unsafe fn send_stream_data(chan: i32, buf: *mut c_void, len: u32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        if buf.is_null() || len == 0 {
            return WPAD_ERR_BADVALUE as i32;
        }
        let data = unsafe { core::slice::from_raw_parts(buf.cast::<u8>(), len as usize) };
        return powerpc_backend::send_stream_data(chan, data);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if buf.is_null() || len == 0 {
        return WPAD_ERR_BADVALUE as i32;
    }
    if !WPAD_SPEAKER[chan].load(Ordering::Acquire) {
        return WPAD_ERR_NOT_READY as i32;
    }
    WPAD_STREAM_LEN[chan].store(len, Ordering::Release);
    WPAD_ERR_NONE as i32
}
pub fn search() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::search();
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    if !WPAD_INITED.load(Ordering::Acquire) {
        return WPAD_ERR_NOT_READY as i32;
    }
    WPAD_SEARCHING.store(true, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLING, Ordering::Release);
    if !WPAD_CONNECTED[WPAD_CHAN_0 as usize].load(Ordering::Acquire) {
        connect_with_defaults(WPAD_CHAN_0 as usize);
    }
    WPAD_ERR_NONE as i32
}
/// Starts general Bluetooth HID discovery without selecting Wii guest pairing.
/// This extension uses the same adapter owner and event path as WPAD_Search.
pub fn search_bluetooth_hid() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::search_bluetooth_hid();
    }
    WPAD_ERR_NOT_READY as i32
}
pub fn stop_search() -> i32 {
    if cfg!(target_arch = "powerpc") {
        let status = powerpc_backend::stop_search();
        if status == WPAD_ERR_NONE as i32 {
            WPAD_SEARCHING.store(false, Ordering::Release);
            WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLED, Ordering::Release);
        }
        return status;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    WPAD_SEARCHING.store(false, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_ENABLED, Ordering::Release);
    WPAD_ERR_NONE as i32
}
pub fn start_pairing() -> i32 {
    if cfg!(target_arch = "powerpc") {
        unsafe {
            if let Some(cb) = HOSTSYNC_CB {
                cb(1);
            }
        }
        return search();
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    unsafe {
        if let Some(cb) = HOSTSYNC_CB {
            cb(1);
        }
    }
    search()
}

pub fn wipe_saved_controllers() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::wipe_saved_controllers();
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    for chan in 0..WPAD_MAX_DEVICES {
        set_connected(chan, false);
    }
    WPAD_ERR_NONE as i32
}
pub fn shutdown() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::shutdown();
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    unsafe {
        for (chan, connected) in WPAD_CONNECTED.iter().enumerate().take(WPAD_MAX_DEVICES) {
            if connected.load(Ordering::Acquire)
                && let Some(cb) = POWER_CB
            {
                cb(chan as i32);
            }
        }
    }
    WPAD_INITED.store(false, Ordering::Release);
    WPAD_SEARCHING.store(false, Ordering::Release);
    WPAD_GLOBAL_STATUS.store(WPAD_STATE_DISABLED, Ordering::Release);
    for chan in 0..WPAD_MAX_DEVICES {
        set_connected(chan, false);
    }
    WPAD_ERR_NONE as i32
}

pub fn set_idle_timeout(seconds: u32) {
    if cfg!(target_arch = "powerpc") {
        WPAD_IDLE_TIMEOUT.store(seconds, Ordering::Release);
        powerpc_backend::set_idle_timeout(seconds);
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    WPAD_IDLE_TIMEOUT.store(seconds, Ordering::Release);
}

pub fn set_power_button_callback(cb: WpadShutdownCallback) {
    unsafe { POWER_CB = cb };
}

pub fn set_battery_dead_callback(cb: WpadShutdownCallback) {
    unsafe { BATTERY_CB = cb };
}

pub fn set_disconnect_callback(cb: WpadDisconnectCallback) {
    unsafe { DISCONNECT_CB = cb };
}

pub fn set_host_sync_button_callback(cb: WpadHostSyncBtnCallback) {
    unsafe { HOSTSYNC_CB = cb };
}

pub fn set_status_callback(cb: WpadStatusCallback) {
    unsafe { STATUS_CB = cb };
}
pub fn scan_pads() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::scan_pads();
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    read_pending(WPAD_CHAN_ALL, None)
}
pub fn rumble(chan: i32, status: i32) -> i32 {
    if chan == WPAD_CHAN_ALL {
        if cfg!(target_arch = "powerpc") {
            if WPAD_GLOBAL_STATUS.load(Ordering::Acquire) == WPAD_STATE_DISABLED {
                return WPAD_ERR_NOT_READY as i32;
            }
            return apply_all_channels(|chan| {
                powerpc_backend::rumble_if_present(chan, status != 0)
            });
        }
        return apply_all_channels(|chan| rumble(chan as i32, status));
    }
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        return powerpc_backend::rumble(chan, status != 0);
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    WPAD_RUMBLE[chan].store(status != 0, Ordering::Release);
    WPAD_ERR_NONE as i32
}
pub fn set_idle_thresholds(
    chan: i32,
    _btns: i32,
    _ir: i32,
    _accel: i32,
    _js: i32,
    _wb: i32,
    _mp: i32,
) -> i32 {
    if chan == WPAD_CHAN_ALL {
        return apply_all_channels(|chan| {
            set_idle_thresholds(chan as i32, _btns, _ir, _accel, _js, _wb, _mp)
        });
    }
    if cfg!(target_arch = "powerpc") {
        let Some(chan) = valid_chan(chan) else {
            return WPAD_ERR_BAD_CHANNEL as i32;
        };
        if WPAD_GLOBAL_STATUS.load(Ordering::Acquire) == WPAD_STATE_DISABLED {
            return WPAD_ERR_NOT_READY as i32;
        }
        WPAD_IDLE_BTNS[chan].store(
            if _btns < 0 { WPAD_THRESH_IGNORE } else { 0 },
            Ordering::Release,
        );
        WPAD_IDLE_IR[chan].store(_ir, Ordering::Release);
        WPAD_IDLE_ACCEL[chan].store(_accel, Ordering::Release);
        WPAD_IDLE_JS[chan].store(_js, Ordering::Release);
        WPAD_IDLE_WB[chan].store(_wb, Ordering::Release);
        WPAD_IDLE_MP[chan].store(_mp, Ordering::Release);
        return WPAD_ERR_NONE as i32;
    }
    if !wpad_host_compatibility_enabled() {
        return WPAD_ERR_NOT_READY as i32;
    }
    let Some(chan) = valid_chan(chan) else {
        return WPAD_ERR_BAD_CHANNEL as i32;
    };
    if WPAD_GLOBAL_STATUS.load(Ordering::Acquire) == WPAD_STATE_DISABLED {
        return WPAD_ERR_NOT_READY as i32;
    }
    WPAD_IDLE_BTNS[chan].store(
        if _btns < 0 { WPAD_THRESH_IGNORE } else { 0 },
        Ordering::Release,
    );
    WPAD_IDLE_IR[chan].store(_ir, Ordering::Release);
    WPAD_IDLE_ACCEL[chan].store(_accel, Ordering::Release);
    WPAD_IDLE_JS[chan].store(_js, Ordering::Release);
    WPAD_IDLE_WB[chan].store(_wb, Ordering::Release);
    WPAD_IDLE_MP[chan].store(_mp, Ordering::Release);
    WPAD_ERR_NONE as i32
}

/// Encodes host-compat audio data into a small status structure and output buffer.
///
/// # Safety
/// `info` and `enc_data` may be null. Any non-null pointer must be valid for
/// writes of the corresponding output object during this call. `_pcm_samples`
/// follows the original C ABI and may reference caller-owned sample data.
pub unsafe fn encode_data(
    info: *mut WpadEncStatus,
    flag: u32,
    _pcm_samples: *const i16,
    num_samples: i32,
    enc_data: *mut u8,
) {
    if cfg!(target_arch = "powerpc") {
        powerpc_backend::encode_data(info, flag, _pcm_samples, num_samples, enc_data);
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if !info.is_null() {
        unsafe {
            (*info).data[0] = flag as u8;
            (*info).data[1] = num_samples.max(0) as u8;
        }
    }
    if !enc_data.is_null() {
        unsafe {
            *enc_data = flag as u8;
        }
    }
}
pub fn data(chan: i32) -> *mut WpadData {
    if cfg!(target_arch = "powerpc") {
        return valid_chan(chan)
            .map(|chan| unsafe { core::ptr::addr_of_mut!(WPAD_DATA_STATE[chan]) })
            .unwrap_or_default();
    }
    if !wpad_host_compatibility_enabled() {
        return core::ptr::null_mut();
    }
    valid_chan(chan)
        .map(|chan| unsafe { core::ptr::addr_of_mut!(WPAD_DATA_STATE[chan]) })
        .unwrap_or_default()
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_data(chan: usize, buttons: u32, battery_level: u8, data_present: u32) {
    unsafe {
        WPAD_DATA_STATE[chan].battery_level = battery_level;
        WPAD_DATA_STATE[chan].data_present = data_present;
    }
    set_connected(chan, true);
    set_buttons(chan, buttons);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_extension(chan: usize, exp_type: i32) {
    unsafe {
        WPAD_DATA_STATE[chan].exp.type_ = exp_type;
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
    }
    push_event(chan);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_nunchuk(chan: usize, buttons: u8, joy: (u8, u8), accel: (u16, u16, u16)) {
    unsafe {
        WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_NUNCHUK as i32;
        WPAD_DATA_STATE[chan].exp.u.nunchuk = Nunchuk {
            btns: buttons,
            btns_last: 0,
            btns_held: buttons,
            btns_released: 0,
            js: Joystick {
                pos: Vec2b { x: joy.0, y: joy.1 },
                ..Joystick::default()
            },
            accel: Vec3w {
                x: accel.0,
                y: accel.1,
                z: accel.2,
            },
            ..Nunchuk::default()
        };
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
    }
    push_event(chan);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_classic(chan: usize, buttons: u32, left: (u8, u8), right: (u8, u8)) {
    unsafe {
        WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_CLASSIC as i32;
        WPAD_DATA_STATE[chan].exp.u.classic = ClassicCtrl {
            btns: buttons,
            btns_last: 0,
            btns_held: buttons,
            btns_released: 0,
            ljs: Joystick {
                pos: Vec2b {
                    x: left.0,
                    y: left.1,
                },
                ..Joystick::default()
            },
            rjs: Joystick {
                pos: Vec2b {
                    x: right.0,
                    y: right.1,
                },
                ..Joystick::default()
            },
            ..ClassicCtrl::default()
        };
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
    }
    push_event(chan);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_guitar(chan: usize, buttons: i16, whammy_bar: F32, touch_bar: i32) {
    unsafe {
        WPAD_DATA_STATE[chan].exp.type_ = WPAD_EXP_GUITARHERO3 as i32;
        WPAD_DATA_STATE[chan].exp.u.gh3 = GuitarHero3 {
            btns: buttons,
            btns_last: 0,
            btns_held: buttons,
            btns_released: 0,
            whammy_bar,
            touch_bar,
            ..GuitarHero3::default()
        };
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_EXPANSION;
    }
    push_event(chan);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn test_set_ir(chan: usize, x: F32, y: F32, dots: u8) {
    unsafe {
        WPAD_DATA_STATE[chan].ir.x = x;
        WPAD_DATA_STATE[chan].ir.y = y;
        WPAD_DATA_STATE[chan].ir.num_dots = dots;
        WPAD_DATA_STATE[chan].ir.valid = 1;
        WPAD_DATA_STATE[chan].data_present |= WPAD_DATA_IR;
    }
    push_event(chan);
}
pub fn battery_level(chan: i32) -> u8 {
    if cfg!(target_arch = "powerpc") {
        return valid_chan(chan)
            .map(|chan| unsafe { WPAD_DATA_STATE[chan].battery_level })
            .unwrap_or(0);
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    valid_chan(chan)
        .map(|chan| unsafe { WPAD_DATA_STATE[chan].battery_level })
        .unwrap_or(0)
}
pub fn buttons_up(chan: i32) -> u32 {
    if cfg!(target_arch = "powerpc") {
        return valid_chan(chan)
            .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_u })
            .unwrap_or(0);
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    valid_chan(chan)
        .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_u })
        .unwrap_or(0)
}
pub fn buttons_down(chan: i32) -> u32 {
    if cfg!(target_arch = "powerpc") {
        return valid_chan(chan)
            .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_d })
            .unwrap_or(0);
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    valid_chan(chan)
        .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_d })
        .unwrap_or(0)
}
pub fn buttons_held(chan: i32) -> u32 {
    if cfg!(target_arch = "powerpc") {
        return valid_chan(chan)
            .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_h })
            .unwrap_or(0);
    }
    if !wpad_host_compatibility_enabled() {
        return 0;
    }
    valid_chan(chan)
        .map(|chan| unsafe { WPAD_DATA_STATE[chan].btns_h })
        .unwrap_or(0)
}

/// Copies the host-compat IR state for a controller channel into a caller buffer.
///
/// # Safety
/// `ir` may be null. When non-null it must be a valid, writable pointer to one
/// `Ir` value for the duration of this call.
pub unsafe fn ir(chan: i32, ir: *mut Ir) {
    if cfg!(target_arch = "powerpc") {
        if let Some(chan) = valid_chan(chan)
            && !ir.is_null()
        {
            unsafe { *ir = WPAD_DATA_STATE[chan].ir };
        }
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if let Some(chan) = valid_chan(chan)
        && !ir.is_null()
    {
        unsafe { *ir = WPAD_DATA_STATE[chan].ir };
    }
}

/// Copies the host-compat orientation state for a controller channel.
///
/// # Safety
/// `orient` may be null. When non-null it must be a valid, writable pointer to
/// one `Orient` value for the duration of this call.
pub unsafe fn orientation(chan: i32, orient: *mut Orient) {
    if cfg!(target_arch = "powerpc") {
        if let Some(chan) = valid_chan(chan)
            && !orient.is_null()
        {
            unsafe { *orient = WPAD_DATA_STATE[chan].orient };
        }
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if let Some(chan) = valid_chan(chan)
        && !orient.is_null()
    {
        unsafe { *orient = WPAD_DATA_STATE[chan].orient };
    }
}

/// Copies the host-compat g-force state for a controller channel.
///
/// # Safety
/// `gforce` may be null. When non-null it must be a valid, writable pointer to
/// one `GForce` value for the duration of this call.
pub unsafe fn g_force(chan: i32, gforce: *mut GForce) {
    if cfg!(target_arch = "powerpc") {
        if let Some(chan) = valid_chan(chan)
            && !gforce.is_null()
        {
            unsafe { *gforce = WPAD_DATA_STATE[chan].gforce };
        }
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if let Some(chan) = valid_chan(chan)
        && !gforce.is_null()
    {
        unsafe { *gforce = WPAD_DATA_STATE[chan].gforce };
    }
}

/// Copies the host-compat acceleration state for a controller channel.
///
/// # Safety
/// `accel` may be null. When non-null it must be a valid, writable pointer to
/// one `Vec3w` value for the duration of this call.
pub unsafe fn accel(chan: i32, accel: *mut Vec3w) {
    if cfg!(target_arch = "powerpc") {
        if let Some(chan) = valid_chan(chan)
            && !accel.is_null()
        {
            unsafe { *accel = WPAD_DATA_STATE[chan].accel };
        }
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if let Some(chan) = valid_chan(chan)
        && !accel.is_null()
    {
        unsafe { *accel = WPAD_DATA_STATE[chan].accel };
    }
}

/// Copies the host-compat expansion state for a controller channel.
///
/// # Safety
/// `exp` may be null. When non-null it must be a valid, writable pointer to
/// one `Expansion` value for the duration of this call.
pub unsafe fn expansion(chan: i32, exp: *mut Expansion) {
    if cfg!(target_arch = "powerpc") {
        if let Some(chan) = valid_chan(chan)
            && !exp.is_null()
        {
            unsafe { *exp = WPAD_DATA_STATE[chan].exp };
        }
        return;
    }
    if !wpad_host_compatibility_enabled() {
        return;
    }
    if let Some(chan) = valid_chan(chan)
        && !exp.is_null()
    {
        unsafe { *exp = WPAD_DATA_STATE[chan].exp };
    }
}

pub fn pad_status(chan: i32) {
    if cfg!(target_arch = "powerpc") {
        powerpc_backend::pad_status(chan);
    }
}

pub fn is_battery_critical(chan: i32) -> bool {
    if cfg!(target_arch = "powerpc") {
        return battery_level(chan) <= 5;
    }
    if !wpad_host_compatibility_enabled() {
        return false;
    }
    battery_level(chan) <= 5
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::size_of;
    use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
    use std::sync::Mutex;

    static STATUS_SEEN: AtomicI32 = AtomicI32::new(-1);
    static POWER_SEEN: AtomicI32 = AtomicI32::new(-1);
    static BATTERY_SEEN: AtomicI32 = AtomicI32::new(-1);
    static DISCONNECT_SEEN: AtomicI32 = AtomicI32::new(-1);
    static DISCONNECT_REASON: AtomicU32 = AtomicU32::new(0);
    static READPENDING_SEEN: AtomicU32 = AtomicU32::new(0);
    static HOSTSYNC_SEEN: AtomicU32 = AtomicU32::new(0);
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn status_cb(chan: i32) {
        STATUS_SEEN.store(chan, Ordering::Release);
    }

    unsafe extern "C" fn power_cb(chan: i32) {
        POWER_SEEN.store(chan, Ordering::Release);
    }

    unsafe extern "C" fn battery_cb(chan: i32) {
        BATTERY_SEEN.store(chan, Ordering::Release);
    }

    unsafe extern "C" fn disconnect_cb(chan: i32, reason: u8) {
        DISCONNECT_SEEN.store(chan, Ordering::Release);
        DISCONNECT_REASON.store(reason as u32, Ordering::Release);
    }

    unsafe extern "C" fn hostsync_cb(held: u32) {
        HOSTSYNC_SEEN.store(held, Ordering::Release);
    }

    unsafe extern "C" fn readpending_cb(_chan: i32, data: *const WpadData) {
        if !data.is_null() {
            READPENDING_SEEN.store(unsafe { (*data).btns_h }, Ordering::Release);
        }
    }

    #[test]
    fn constants_and_shapes_exist() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(WPAD_MAX_IR_DOTS, 4);
        assert_eq!(WPAD_BUTTON_A, 0x0008);
        assert_eq!(WPAD_ERR_BAD_CHANNEL, -6);
        assert_eq!(size_of::<WpadEncStatus>(), 32);
        assert!(size_of::<WpadData>() > 0);
        assert!(size_of::<Expansion>() > 0);
    }

    #[test]
    fn channel_validation_matches_c_six_channel_range() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(valid_chan(WPAD_CHAN_0), Some(0));
        assert_eq!(
            valid_chan(WPAD_CHAN_UNKNOWN),
            Some(WPAD_CHAN_UNKNOWN as usize)
        );
        assert_eq!(valid_chan(WPAD_MAX_DEVICES as i32), None);
        assert_eq!(valid_chan(WPAD_CHAN_ALL), None);
    }

    #[test]
    fn all_channel_dispatch_visits_all_six_slots_in_order() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut visited = [usize::MAX; WPAD_MAX_DEVICES];
        let mut count = 0;
        assert_eq!(
            apply_all_channels(|chan| {
                visited[count] = chan;
                count += 1;
                WPAD_ERR_NONE as i32
            }),
            WPAD_ERR_NONE as i32
        );
        assert_eq!(count, WPAD_MAX_DEVICES);
        assert_eq!(visited, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn motion_plus_all_dispatch_matches_c_channel_order_and_value() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut calls = [(usize::MAX, false); WPAD_MAX_DEVICES];
        let mut count = 0;
        assert_eq!(
            apply_motion_plus_all(true, |chan, enable| {
                calls[count] = (chan, enable);
                count += 1;
                WPAD_ERR_NONE as i32
            }),
            WPAD_ERR_NONE as i32
        );
        assert_eq!(count, WPAD_MAX_DEVICES);
        assert_eq!(
            calls,
            [
                (0, true),
                (1, true),
                (2, true),
                (3, true),
                (4, true),
                (5, true)
            ]
        );
    }

    #[test]
    fn exports_exist() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _ = init as fn() -> i32;
        let _ = read_event as unsafe fn(i32, *mut WpadData) -> i32;
        let _ = data as fn(i32) -> *mut WpadData;
    }

    #[test]
    fn init_status_and_shutdown_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        assert_eq!(get_status(), WPAD_STATE_ENABLED);
        assert_eq!(shutdown(), 0);
        assert_eq!(get_status(), WPAD_STATE_DISABLED);
    }

    #[test]
    fn powerpc_runtime_lifecycle_helpers_clear_and_restore_state() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        powerpc_runtime_reset();
        assert!(!WPAD_INITED.load(Ordering::Acquire));
        assert!(!WPAD_SEARCHING.load(Ordering::Acquire));
        assert_eq!(get_status(), WPAD_STATE_DISABLED);

        powerpc_runtime_mark_initialized();
        assert!(WPAD_INITED.load(Ordering::Acquire));
        assert_eq!(get_status(), WPAD_STATE_ENABLING);

        powerpc_runtime_begin_search();
        assert!(WPAD_SEARCHING.load(Ordering::Acquire));
        assert_eq!(get_status(), WPAD_STATE_ENABLING);

        powerpc_runtime_end_search();
        assert!(!WPAD_SEARCHING.load(Ordering::Acquire));
        assert_eq!(get_status(), WPAD_STATE_ENABLED);

        powerpc_runtime_reset();
        assert!(!WPAD_INITED.load(Ordering::Acquire));
        assert!(!WPAD_SEARCHING.load(Ordering::Acquire));
        assert_eq!(get_status(), WPAD_STATE_DISABLED);
    }

    #[test]
    fn bad_channel_and_empty_queue_are_stable() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut data = WpadData::default();
        assert_eq!(
            unsafe { read_event(99, &mut data) },
            WPAD_ERR_BAD_CHANNEL as i32
        );
        assert_eq!(init(), 0);
        assert_eq!(
            unsafe { read_event(WPAD_CHAN_0, &mut data) },
            WPAD_ERR_NO_CONTROLLER as i32
        );
        let mut exp_type = 0xfeed_beef;
        assert_eq!(
            unsafe { probe(99, &mut exp_type) },
            WPAD_ERR_BAD_CHANNEL as i32
        );
        assert_eq!(exp_type, 0xfeed_beef);
        assert_eq!(
            unsafe { probe(WPAD_CHAN_0, &mut exp_type) },
            WPAD_ERR_NO_CONTROLLER as i32
        );
        assert_eq!(exp_type, 0xfeed_beef);
        assert_eq!(shutdown(), 0);
        assert_eq!(
            unsafe { probe(WPAD_CHAN_0, &mut exp_type) },
            WPAD_ERR_NOT_READY as i32
        );
        assert_eq!(exp_type, 0xfeed_beef);
    }

    #[test]
    fn wii_probe_expansion_mapping_hides_motion_plus() {
        assert_eq!(
            probe_expansion_type_for_wii(WPAD_EXP_NUNCHUK as i32),
            WPAD_EXP_NUNCHUK
        );
        assert_eq!(
            probe_expansion_type_for_wii(WPAD_EXP_CLASSIC as i32),
            WPAD_EXP_CLASSIC
        );
        assert_eq!(
            probe_expansion_type_for_wii(WPAD_EXP_UNKNOWN as i32),
            WPAD_EXP_NONE
        );
    }

    #[test]
    fn format_search_and_rumble_state_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        assert_eq!(set_data_format(WPAD_CHAN_0, WPAD_FMT_BTNS_ACC_IR), 0);
        assert_eq!(search(), 0);
        assert_eq!(get_status(), WPAD_STATE_ENABLING);
        assert_eq!(scan_pads(), 1);
        assert_eq!(rumble(WPAD_CHAN_0, 1), 0);
        assert_eq!(stop_search(), 0);
        assert_eq!(get_status(), WPAD_STATE_ENABLED);
        assert_eq!(is_speaker_enabled(WPAD_CHAN_0), 0);
        assert_eq!(set_v_res(WPAD_CHAN_0, 640, 480), 0);
        let ir = unsafe { &*data(WPAD_CHAN_0) }.ir;
        assert_eq!(ir.vres, [640, 480]);
    }

    #[test]
    fn event_buffers_and_callbacks_follow_host_connection_state() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        STATUS_SEEN.store(-1, Ordering::Release);
        POWER_SEEN.store(-1, Ordering::Release);
        BATTERY_SEEN.store(-1, Ordering::Release);
        DISCONNECT_SEEN.store(-1, Ordering::Release);
        DISCONNECT_REASON.store(0, Ordering::Release);
        READPENDING_SEEN.store(0, Ordering::Release);
        HOSTSYNC_SEEN.store(0, Ordering::Release);

        assert_eq!(init(), 0);
        set_status_callback(Some(status_cb));
        set_power_button_callback(Some(power_cb));
        set_battery_dead_callback(Some(battery_cb));
        set_disconnect_callback(Some(disconnect_cb));
        set_host_sync_button_callback(Some(hostsync_cb));

        let mut ring = [WpadData::default(); 2];
        assert_eq!(
            unsafe { set_event_bufs(WPAD_CHAN_0, ring.as_mut_ptr(), 2) },
            0
        );

        test_set_data(
            WPAD_CHAN_0 as usize,
            WPAD_BUTTON_A | WPAD_BUTTON_HOME,
            3,
            WPAD_DATA_BUTTONS,
        );

        assert_eq!(STATUS_SEEN.load(Ordering::Acquire), WPAD_CHAN_0);
        assert_eq!(
            unsafe { (*data(WPAD_CHAN_0)).btns_h },
            WPAD_BUTTON_A | WPAD_BUTTON_HOME
        );
        assert_eq!(buttons_down(WPAD_CHAN_0), WPAD_BUTTON_A | WPAD_BUTTON_HOME);
        assert_eq!(buttons_up(WPAD_CHAN_0), 0);
        assert_eq!(ring[0].btns_h, WPAD_BUTTON_A | WPAD_BUTTON_HOME);
        assert!(is_battery_critical(WPAD_CHAN_0));
        assert_eq!(BATTERY_SEEN.load(Ordering::Acquire), WPAD_CHAN_0);

        assert_eq!(read_pending(WPAD_CHAN_0, Some(readpending_cb)), 1);
        assert_eq!(
            READPENDING_SEEN.load(Ordering::Acquire),
            WPAD_BUTTON_A | WPAD_BUTTON_HOME
        );
        assert_eq!(start_pairing(), 0);
        assert_eq!(HOSTSYNC_SEEN.load(Ordering::Acquire), 1);

        assert_eq!(disconnect(WPAD_CHAN_0), 0);
        assert_eq!(DISCONNECT_SEEN.load(Ordering::Acquire), WPAD_CHAN_0);
        assert_eq!(
            DISCONNECT_REASON.load(Ordering::Acquire),
            WPAD_DISCON_POWER_OFF as u32
        );
        assert_eq!(unsafe { (*data(WPAD_CHAN_0)).err }, WPAD_ERR_NO_CONTROLLER);
        assert_eq!(shutdown(), 0);
        assert_eq!(POWER_SEEN.load(Ordering::Acquire), -1);
    }

    #[test]
    fn dropped_events_and_buffer_wrap_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        let mut ring = [WpadData::default(); 1];
        assert_eq!(
            unsafe { set_event_bufs(WPAD_CHAN_0, ring.as_mut_ptr(), 1) },
            0
        );

        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_A, 0x20, WPAD_DATA_BUTTONS);
        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_B, 0x20, WPAD_DATA_BUTTONS);

        assert_eq!(ring[0].btns_h, WPAD_BUTTON_B);
        assert_eq!(dropped_events(WPAD_CHAN_0), 1);
        assert_eq!(dropped_events(WPAD_CHAN_0), 0);
        assert_eq!(buttons_down(WPAD_CHAN_0), WPAD_BUTTON_B);
        assert_eq!(buttons_up(WPAD_CHAN_0), WPAD_BUTTON_A);
    }

    #[test]
    fn read_event_and_scanpads_drain_queue_in_order() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);

        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_A, 0x20, WPAD_DATA_BUTTONS);
        test_set_data(
            WPAD_CHAN_0 as usize,
            WPAD_BUTTON_A | WPAD_BUTTON_B,
            0x20,
            WPAD_DATA_BUTTONS,
        );

        let mut first = WpadData::default();
        let mut second = WpadData::default();
        assert_eq!(unsafe { read_event(WPAD_CHAN_0, &mut first) }, 0);
        assert_eq!(first.btns_h, WPAD_BUTTON_A);
        assert_eq!(unsafe { read_event(WPAD_CHAN_0, &mut second) }, 0);
        assert_eq!(second.btns_h, WPAD_BUTTON_A | WPAD_BUTTON_B);

        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_B, 0x20, WPAD_DATA_BUTTONS);
        test_set_data(
            WPAD_CHAN_0 as usize,
            WPAD_BUTTON_B | WPAD_BUTTON_A,
            0x20,
            WPAD_DATA_BUTTONS,
        );
        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_B, 0x20, WPAD_DATA_BUTTONS);
        assert_eq!(scan_pads(), 3);
        assert_eq!(buttons_held(WPAD_CHAN_0), WPAD_BUTTON_A | WPAD_BUTTON_B);
        assert_eq!(buttons_down(WPAD_CHAN_0), WPAD_BUTTON_A);
        assert_eq!(buttons_up(WPAD_CHAN_0), 0);
        assert_eq!(
            unsafe { read_event(WPAD_CHAN_0, &mut second) },
            WPAD_ERR_QUEUE_EMPTY as i32
        );
    }

    #[test]
    fn flush_returns_drained_event_count() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_A, 0x20, WPAD_DATA_BUTTONS);
        test_set_data(WPAD_CHAN_0 as usize, WPAD_BUTTON_B, 0x20, WPAD_DATA_BUTTONS);
        assert_eq!(flush(WPAD_CHAN_0), 2);
        let mut data = WpadData::default();
        assert_eq!(
            unsafe { read_event(WPAD_CHAN_0, &mut data) },
            WPAD_ERR_QUEUE_EMPTY as i32
        );
    }

    #[test]
    fn motion_plus_probe_and_shutdown_callback_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        POWER_SEEN.store(-1, Ordering::Release);
        assert_eq!(init(), 0);
        set_power_button_callback(Some(power_cb));
        assert_eq!(search(), 0);

        let mut exp_type = 0;
        assert_eq!(unsafe { probe(WPAD_CHAN_0, &mut exp_type) }, 0);
        assert_eq!(exp_type, WPAD_EXP_NONE);

        assert_eq!(set_motion_plus(WPAD_CHAN_0, 1), 0);
        assert_eq!(unsafe { probe(WPAD_CHAN_0, &mut exp_type) }, 0);
        assert_eq!(exp_type, WPAD_EXP_NONE);

        assert_eq!(set_motion_plus(WPAD_CHAN_0, 0), 0);
        assert_eq!(unsafe { probe(WPAD_CHAN_0, &mut exp_type) }, 0);
        assert_eq!(exp_type, WPAD_EXP_NONE);

        test_set_extension(WPAD_CHAN_0 as usize, WPAD_EXP_CLASSIC as i32);
        let mut exp = Expansion::default();
        unsafe { expansion(WPAD_CHAN_0, &mut exp) };
        assert_eq!(exp.type_, WPAD_EXP_CLASSIC as i32);

        assert_eq!(shutdown(), 0);
        assert_eq!(POWER_SEEN.load(Ordering::Acquire), WPAD_CHAN_0);
    }

    #[test]
    fn all_channel_rumble_speaker_and_idle_thresholds_match_c_dispatch() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), WPAD_ERR_NONE as i32);

        assert_eq!(rumble(WPAD_CHAN_ALL, 1), WPAD_ERR_NONE as i32);
        assert_eq!(control_speaker(WPAD_CHAN_ALL, 1), WPAD_ERR_NONE as i32);
        assert_eq!(
            set_idle_thresholds(WPAD_CHAN_ALL, 7, 2, 3, 4, 5, 6),
            WPAD_ERR_NONE as i32
        );
        for chan in 0..WPAD_MAX_DEVICES {
            assert!(WPAD_RUMBLE[chan].load(Ordering::Acquire));
            assert!(WPAD_SPEAKER[chan].load(Ordering::Acquire));
            assert_eq!(WPAD_IDLE_BTNS[chan].load(Ordering::Acquire), 0);
            assert_eq!(WPAD_IDLE_IR[chan].load(Ordering::Acquire), 2);
            assert_eq!(WPAD_IDLE_ACCEL[chan].load(Ordering::Acquire), 3);
            assert_eq!(WPAD_IDLE_JS[chan].load(Ordering::Acquire), 4);
            assert_eq!(WPAD_IDLE_WB[chan].load(Ordering::Acquire), 5);
            assert_eq!(WPAD_IDLE_MP[chan].load(Ordering::Acquire), 6);
        }

        assert_eq!(
            set_idle_thresholds(WPAD_CHAN_ALL, -99, 2, 3, 4, 5, 6),
            WPAD_ERR_NONE as i32
        );
        for chan in 0..WPAD_MAX_DEVICES {
            assert_eq!(
                WPAD_IDLE_BTNS[chan].load(Ordering::Acquire),
                WPAD_THRESH_IGNORE
            );
        }

        // C stores non-button threshold values directly, including values below
        // the public ignore sentinel.
        assert_eq!(
            set_idle_thresholds(WPAD_CHAN_ALL, 0, -2, -3, -4, -5, -6),
            WPAD_ERR_NONE as i32
        );
        for chan in 0..WPAD_MAX_DEVICES {
            assert_eq!(WPAD_IDLE_IR[chan].load(Ordering::Acquire), -2);
            assert_eq!(WPAD_IDLE_ACCEL[chan].load(Ordering::Acquire), -3);
            assert_eq!(WPAD_IDLE_JS[chan].load(Ordering::Acquire), -4);
            assert_eq!(WPAD_IDLE_WB[chan].load(Ordering::Acquire), -5);
            assert_eq!(WPAD_IDLE_MP[chan].load(Ordering::Acquire), -6);
        }

        assert_eq!(shutdown(), WPAD_ERR_NONE as i32);
        assert_eq!(
            set_idle_thresholds(WPAD_CHAN_0, 0, 1, 2, 3, 4, 5),
            WPAD_ERR_NOT_READY as i32
        );
    }

    #[test]
    fn idle_thresholds_ir_and_speaker_stream_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        assert_eq!(search(), 0);
        assert_eq!(
            set_idle_thresholds(WPAD_CHAN_0, 3, 4, 5, 6, 7, 8),
            WPAD_ERR_NONE as i32
        );
        test_set_ir(WPAD_CHAN_0 as usize, 320.0, 200.0, 2);
        let mut ir_state = Ir::default();
        unsafe { ir(WPAD_CHAN_0, &mut ir_state) };
        assert_eq!(ir_state.num_dots, 2);
        assert_eq!(ir_state.valid, 1);
        let payload = [1_u8, 2, 3, 4];
        assert_eq!(
            unsafe {
                send_stream_data(
                    WPAD_CHAN_0,
                    payload.as_ptr().cast_mut().cast(),
                    payload.len() as u32,
                )
            },
            WPAD_ERR_NOT_READY as i32
        );
        assert_eq!(control_speaker(WPAD_CHAN_0, 1), 0);
        assert_eq!(
            unsafe {
                send_stream_data(
                    WPAD_CHAN_0,
                    payload.as_ptr().cast_mut().cast(),
                    payload.len() as u32,
                )
            },
            WPAD_ERR_NONE as i32
        );
        assert_eq!(
            WPAD_STREAM_LEN[WPAD_CHAN_0 as usize].load(Ordering::Acquire),
            payload.len() as u32
        );
    }

    #[test]
    fn extension_payloads_round_trip_for_nunchuk_classic_and_guitar() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(init(), 0);
        assert_eq!(search(), 0);

        test_set_nunchuk(WPAD_CHAN_0 as usize, 0x03, (17, 19), (100, 200, 300));
        let mut exp = Expansion::default();
        unsafe { expansion(WPAD_CHAN_0, &mut exp) };
        assert_eq!(exp.type_, WPAD_EXP_NUNCHUK as i32);
        let nunchuk = unsafe { exp.u.nunchuk };
        assert_eq!(nunchuk.btns as u32, 0x03);
        assert_eq!(nunchuk.js.pos.x, 17);
        assert_eq!(nunchuk.accel.z, 300);

        test_set_classic(
            WPAD_CHAN_0 as usize,
            WPAD_CLASSIC_BUTTON_A | WPAD_CLASSIC_BUTTON_PLUS,
            (8, 9),
            (10, 11),
        );
        unsafe { expansion(WPAD_CHAN_0, &mut exp) };
        assert_eq!(exp.type_, WPAD_EXP_CLASSIC as i32);
        let classic = unsafe { exp.u.classic };
        assert_eq!(
            classic.btns,
            WPAD_CLASSIC_BUTTON_A | WPAD_CLASSIC_BUTTON_PLUS
        );
        assert_eq!(classic.ljs.pos.y, 9);
        assert_eq!(classic.rjs.pos.x, 10);

        test_set_guitar(WPAD_CHAN_0 as usize, 0x4050, 0.5, 3);
        unsafe { expansion(WPAD_CHAN_0, &mut exp) };
        assert_eq!(exp.type_, WPAD_EXP_GUITARHERO3 as i32);
        let gh3 = unsafe { exp.u.gh3 };
        assert_eq!(gh3.btns as u16 as u32, 0x4050);
        assert_eq!(gh3.touch_bar, 3);
        assert_eq!(gh3.whammy_bar, 0.5);
    }
}
