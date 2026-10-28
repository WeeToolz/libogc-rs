#![no_std]

extern crate alloc;
#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use alloc::collections::VecDeque;
use core::cell::UnsafeCell;
use core::ffi::c_char;
#[cfg(target_arch = "powerpc")]
use core::ffi::c_void;
#[cfg(target_arch = "powerpc")]
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU16, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_lwp_sys as lwp;
#[cfg(target_arch = "powerpc")]
use ogc_usb as usb;
pub const IPC_OK: i32 = 0;
pub const IPC_ENOHEAP: i32 = -5;

pub const MOD_SHIFT_L: u16 = 1 << 0;
pub const MOD_SHIFT_R: u16 = 1 << 1;
pub const MOD_SHIFTLOCK: u16 = 1 << 2;
pub const MOD_CAPSLOCK: u16 = 1 << 3;
pub const MOD_CONTROL_L: u16 = 1 << 4;
pub const MOD_CONTROL_R: u16 = 1 << 5;
pub const MOD_META_L: u16 = 1 << 6;
pub const MOD_META_R: u16 = 1 << 7;
pub const MOD_MODESHIFT: u16 = 1 << 8;
pub const MOD_NUMLOCK: u16 = 1 << 9;
pub const MOD_COMPOSE: u16 = 1 << 10;
pub const MOD_HOLDSCREEN: u16 = 1 << 11;
pub const MOD_COMMAND: u16 = 1 << 12;
pub const MOD_COMMAND1: u16 = 1 << 13;
pub const MOD_COMMAND2: u16 = 1 << 14;
pub const MOD_MODELOCK: u16 = 1 << 15;

pub const KS_F1: u16 = 0xf340;
pub const KS_F2: u16 = 0xf341;
pub const KS_F3: u16 = 0xf342;
pub const KS_F4: u16 = 0xf343;
pub const KS_F5: u16 = 0xf344;
pub const KS_F6: u16 = 0xf345;
pub const KS_F7: u16 = 0xf346;
pub const KS_F8: u16 = 0xf347;
pub const KS_F9: u16 = 0xf348;
pub const KS_F10: u16 = 0xf349;
pub const KS_F11: u16 = 0xf34a;
pub const KS_F12: u16 = 0xf34b;
pub const KS_F13: u16 = 0xf34c;
pub const KS_F14: u16 = 0xf34d;
pub const KS_F15: u16 = 0xf34e;
pub const KS_F16: u16 = 0xf34f;
pub const KS_F17: u16 = 0xf350;
pub const KS_F18: u16 = 0xf351;
pub const KS_F19: u16 = 0xf352;
pub const KS_F20: u16 = 0xf353;

const KS_LOWER_F1: u16 = 0xf300;
const KS_CMD_SCREEN0: u16 = 0xf400;

// Modifier and lock keysyms from wsksymdef.h. Keep the event values aligned
// with ukbd_keydesc_us in libogc's ukbdmap.c.
const KS_SHIFT_L: u16 = 0xf101;
const KS_SHIFT_R: u16 = 0xf102;
const KS_CONTROL_L: u16 = 0xf103;
const KS_CONTROL_R: u16 = 0xf104;
const KS_CAPS_LOCK: u16 = 0xf105;
const KS_ALT_L: u16 = 0xf107;
const KS_ALT_R: u16 = 0xf108;
const KS_NUM_LOCK: u16 = 0xf10b;
const KS_HOLD_SCREEN: u16 = 0xf10c;
const KS_CMD1: u16 = 0xf10e;
const KS_CMD2: u16 = 0xf10f;
const KS_META_L: u16 = 0xf110;
const KS_META_R: u16 = 0xf111;

pub const KEYBOARD_CONNECTED: i32 = 0;
pub const KEYBOARD_DISCONNECTED: i32 = 1;
pub const KEYBOARD_PRESSED: i32 = 2;
pub const KEYBOARD_RELEASED: i32 = 3;

pub const USBKEYBOARD_PRESSED: i32 = 0;
pub const USBKEYBOARD_RELEASED: i32 = 1;
pub const USBKEYBOARD_DISCONNECTED: i32 = 2;

pub const USBKEYBOARD_LEDNUM: i32 = 0;
pub const USBKEYBOARD_LEDCAPS: i32 = 1;
pub const USBKEYBOARD_LEDSCROLL: i32 = 2;

const MAX_SCAN_KEYS: usize = 6;
const KEY_ERROR: u8 = 0x01;

pub type KeyPressCallback = Option<unsafe extern "C" fn(symbol: c_char)>;
pub type EventCallback = Option<unsafe extern "C" fn(event: UsbKeyboardEvent)>;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct KeyboardEvent {
    pub type_: i32,
    pub modifiers: u16,
    pub keycode: u8,
    pub symbol: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct UsbKeyboardEvent {
    pub type_: i32,
    pub key_code: u8,
}

#[derive(Copy, Clone, Default)]
struct ScanSnapshot {
    modifiers: u16,
    keycodes: [u8; MAX_SCAN_KEYS],
}

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct UsbKeyboardByte([u8; 1]);

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct UsbKeyboardReport([u8; 8]);

static USB_INITIALIZED: AtomicBool = AtomicBool::new(false);
static USB_CONNECTED: AtomicBool = AtomicBool::new(false);
static USB_LAST_STATUS: AtomicI32 = AtomicI32::new(IPC_OK);
static KEYBOARD_INITIALIZED: AtomicBool = AtomicBool::new(false);
static HOST_DEVICE_PRESENT: AtomicBool = AtomicBool::new(false);
static LED_BITS: AtomicU8 = AtomicU8::new(0);
static ACTIVE_MODIFIERS: AtomicU16 = AtomicU16::new(0);
static CALLBACK_PRESENT: AtomicBool = AtomicBool::new(false);
static KEYBOARD_LAYOUT: AtomicU8 = AtomicU8::new(0);

#[cfg(target_arch = "powerpc")]
const KBD_THREAD_STACK_SIZE: usize = 4 * 1024;
#[cfg(target_arch = "powerpc")]
const KBD_THREAD_PRIORITY: u8 = 64;
#[cfg(target_arch = "powerpc")]
const KBD_POLL_TICKS: u64 = lwp::TICKS_PER_SECOND / 100;
#[cfg(target_arch = "powerpc")]
const KBD_DISCOVERY_INTERVAL_POLLS: u32 = 300;
#[cfg(target_arch = "powerpc")]
const KBD_CONNECT_LED_FLASH_TICKS: u64 = lwp::TICKS_PER_SECOND / 5;
const STDIN_BUFFER_CAPACITY: usize = 255;
#[cfg(target_arch = "powerpc")]
#[repr(align(8))]
struct KeyboardThreadStack([u8; KBD_THREAD_STACK_SIZE]);
#[cfg(target_arch = "powerpc")]
static mut KBD_THREAD_STACK: KeyboardThreadStack = KeyboardThreadStack([0; KBD_THREAD_STACK_SIZE]);
#[cfg(target_arch = "powerpc")]
static mut KBD_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;
#[cfg(target_arch = "powerpc")]
static KBD_THREAD_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static KBD_THREAD_QUIT: AtomicBool = AtomicBool::new(false);

static mut USB_CALLBACK: EventCallback = None;
static mut KEYBOARD_CALLBACK: KeyPressCallback = None;
#[cfg(target_arch = "powerpc")]
static mut PREVIOUS_STDIN_DEVICE: *const ogc_sysbase::Devoptab = core::ptr::null();
static mut LAST_SCAN: ScanSnapshot = ScanSnapshot {
    modifiers: 0,
    keycodes: [0; MAX_SCAN_KEYS],
};
static mut NEXT_SCAN: ScanSnapshot = ScanSnapshot {
    modifiers: 0,
    keycodes: [0; MAX_SCAN_KEYS],
};
static mut HELD_SYMBOLS: [u16; 256] = [0; 256];

#[cfg(target_arch = "powerpc")]
#[derive(Copy, Clone)]
struct UsbKeyboardDevice {
    connected: bool,
    fd: i32,
    sc_ndata: ScanSnapshot,
    sc_odata: ScanSnapshot,
    leds: u8,
    cb: EventCallback,
    configuration: u8,
    interface: u8,
    alt_interface: u8,
    ep: u8,
    ep_size: u16,
}

#[cfg(target_arch = "powerpc")]
impl UsbKeyboardDevice {
    const fn new() -> Self {
        Self {
            connected: false,
            fd: -1,
            sc_ndata: ScanSnapshot {
                modifiers: 0,
                keycodes: [0; MAX_SCAN_KEYS],
            },
            sc_odata: ScanSnapshot {
                modifiers: 0,
                keycodes: [0; MAX_SCAN_KEYS],
            },
            leds: 0,
            cb: None,
            configuration: 0,
            interface: 0,
            alt_interface: 0,
            ep: 0,
            ep_size: 0,
        }
    }
}

#[cfg(target_arch = "powerpc")]
static mut POWERPC_KEYBOARD: UsbKeyboardDevice = UsbKeyboardDevice::new();

#[inline]
const fn keyboard_host_compatibility_enabled() -> bool {
    !cfg!(target_arch = "powerpc")
}

struct LockedQueue<T> {
    locked: AtomicBool,
    queue: UnsafeCell<Option<VecDeque<T>>>,
}

unsafe impl<T: Send> Sync for LockedQueue<T> {}

impl<T> LockedQueue<T> {
    const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            queue: UnsafeCell::new(None),
        }
    }

    fn with_queue<R>(&self, f: impl FnOnce(&mut VecDeque<T>) -> R) -> R {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
            #[cfg(target_arch = "powerpc")]
            lwp::yield_thread();
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _guard = Guard(&self.locked);
        let slot = unsafe { &mut *self.queue.get() };
        let queue = slot.get_or_insert_with(VecDeque::new);
        f(queue)
    }
}

// Real USB callback events are queued here for get_event and stdin consumers.
static EVENT_QUEUE: LockedQueue<KeyboardEvent> = LockedQueue::new();
static STDIN_KEY_QUEUE: LockedQueue<u8> = LockedQueue::new();

fn queue_push(event: KeyboardEvent) -> bool {
    EVENT_QUEUE.with_queue(|queue| queue.push_back(event));
    true
}

fn queue_pop() -> Option<KeyboardEvent> {
    EVENT_QUEUE.with_queue(VecDeque::pop_front)
}

fn stdin_key_try_push(key: u8) -> bool {
    STDIN_KEY_QUEUE.with_queue(|queue| {
        if queue.len() >= STDIN_BUFFER_CAPACITY {
            return false;
        }
        queue.push_back(key);
        true
    })
}

fn stdin_key_has_space() -> bool {
    STDIN_KEY_QUEUE.with_queue(|queue| queue.len() < STDIN_BUFFER_CAPACITY)
}

fn stdin_key_pop() -> Option<u8> {
    STDIN_KEY_QUEUE.with_queue(VecDeque::pop_front)
}

fn stdin_key_clear() {
    STDIN_KEY_QUEUE.with_queue(VecDeque::clear);
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn keyboard_stdin_read(
    _reent: *mut ogc_sysbase::Reent,
    _file: *mut c_void,
    output: *mut c_char,
    len: usize,
) -> isize {
    if output.is_null() {
        return -1;
    }
    let mut read = 0;
    while read < len {
        let byte = loop {
            if let Some(byte) = stdin_key_pop() {
                break byte;
            }
            core::hint::spin_loop();
            lwp::yield_thread();
        };
        unsafe { *output.add(read) = byte as c_char };
        read += 1;
        unsafe {
            if let Some(callback) = KEYBOARD_CALLBACK {
                callback(byte as c_char);
            }
        }
    }
    read as isize
}

#[cfg(target_arch = "powerpc")]
static STDIN_DEVICE_NAME: &[u8] = b"stdin\0";

#[cfg(target_arch = "powerpc")]
static mut STDIN_DEVICE: ogc_sysbase::Devoptab = ogc_sysbase::Devoptab {
    name: STDIN_DEVICE_NAME.as_ptr().cast(),
    struct_size: 0,
    open_r: None,
    close_r: None,
    write_r: None,
    read_r: Some(keyboard_stdin_read),
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
};

fn queue_flush() -> i32 {
    let mut count = 0;
    while queue_pop().is_some() {
        count += 1;
    }
    count
}

fn modifier_mask_for_keycode(keycode: u8) -> u16 {
    match keycode {
        224 => MOD_CONTROL_L,
        225 => MOD_SHIFT_L,
        226 => MOD_META_L,
        227 => 0,
        228 => MOD_CONTROL_R,
        229 => MOD_SHIFT_R,
        230 => MOD_META_R,
        231 => 0,
        _ => 0,
    }
}

const US_LAYOUT: &[(u8, u16, u16)] = &[
    (4, b'a' as u16, b'A' as u16),
    (5, b'b' as u16, b'B' as u16),
    (6, b'c' as u16, b'C' as u16),
    (7, b'd' as u16, b'D' as u16),
    (8, b'e' as u16, b'E' as u16),
    (9, b'f' as u16, b'F' as u16),
    (10, b'g' as u16, b'G' as u16),
    (11, b'h' as u16, b'H' as u16),
    (12, b'i' as u16, b'I' as u16),
    (13, b'j' as u16, b'J' as u16),
    (14, b'k' as u16, b'K' as u16),
    (15, b'l' as u16, b'L' as u16),
    (16, b'm' as u16, b'M' as u16),
    (17, b'n' as u16, b'N' as u16),
    (18, b'o' as u16, b'O' as u16),
    (19, b'p' as u16, b'P' as u16),
    (20, b'q' as u16, b'Q' as u16),
    (21, b'r' as u16, b'R' as u16),
    (22, b's' as u16, b'S' as u16),
    (23, b't' as u16, b'T' as u16),
    (24, b'u' as u16, b'U' as u16),
    (25, b'v' as u16, b'V' as u16),
    (26, b'w' as u16, b'W' as u16),
    (27, b'x' as u16, b'X' as u16),
    (28, b'y' as u16, b'Y' as u16),
    (29, b'z' as u16, b'Z' as u16),
    (30, b'1' as u16, b'!' as u16),
    (31, b'2' as u16, b'@' as u16),
    (32, b'3' as u16, b'#' as u16),
    (33, b'4' as u16, b'$' as u16),
    (34, b'5' as u16, b'%' as u16),
    (35, b'6' as u16, b'^' as u16),
    (36, b'7' as u16, b'&' as u16),
    (37, b'8' as u16, b'*' as u16),
    (38, b'9' as u16, b'(' as u16),
    (39, b'0' as u16, b')' as u16),
    (40, 0x000d, 0x000d),
    (41, 0x1b, 0x1b),
    (42, 0x08, 0x08),
    (43, b'\t' as u16, b'\t' as u16),
    (44, b' ' as u16, b' ' as u16),
    (45, b'-' as u16, b'_' as u16),
    (46, b'=' as u16, b'+' as u16),
    (47, b'[' as u16, b'{' as u16),
    (48, b']' as u16, b'}' as u16),
    (49, b'\\' as u16, b'|' as u16),
    (50, b'\\' as u16, b'|' as u16),
    (51, b';' as u16, b':' as u16),
    (52, b'\'' as u16, b'"' as u16),
    (53, b'`' as u16, b'~' as u16),
    (54, b',' as u16, b'<' as u16),
    (55, b'.' as u16, b'>' as u16),
    (56, b'/' as u16, b'?' as u16),
];

fn function_key_symbol(key_code: u8, modifiers: u16) -> Option<u16> {
    if (58..=69).contains(&key_code) {
        let offset = u16::from(key_code - 58);
        let shifted = modifiers & (MOD_SHIFT_L | MOD_SHIFT_R | MOD_SHIFTLOCK) != 0;
        Some(if shifted {
            KS_LOWER_F1 + offset
        } else {
            KS_CMD_SCREEN0 + offset
        })
    } else {
        None
    }
}

fn modifier_lock_key_symbol(key_code: u8, modifiers: u16) -> Option<u16> {
    let shifted = modifiers & (MOD_SHIFT_L | MOD_SHIFT_R | MOD_SHIFTLOCK) != 0;
    Some(match key_code {
        57 => KS_CAPS_LOCK,
        71 => KS_HOLD_SCREEN,
        83 => KS_NUM_LOCK,
        224 => {
            if shifted {
                KS_CONTROL_L
            } else {
                KS_CMD1
            }
        }
        225 => KS_SHIFT_L,
        226 => {
            if shifted {
                KS_ALT_L
            } else {
                KS_CMD2
            }
        }
        227 => KS_META_L,
        228 => {
            if shifted {
                KS_CONTROL_R
            } else {
                KS_CMD1
            }
        }
        229 => KS_SHIFT_R,
        230 => {
            if shifted {
                KS_ALT_R
            } else {
                KS_CMD2
            }
        }
        231 => KS_META_R,
        _ => return None,
    })
}

fn translate_symbol(key_code: u8, modifiers: u16) -> u16 {
    if let Some(symbol) = modifier_lock_key_symbol(key_code, modifiers) {
        return symbol;
    }
    if let Some(symbol) = function_key_symbol(key_code, modifiers) {
        return symbol;
    }

    let layout = KEYBOARD_LAYOUT.load(Ordering::Acquire);
    let shift = (modifiers & (MOD_SHIFT_L | MOD_SHIFT_R | MOD_SHIFTLOCK)) != 0;
    let caps = (modifiers & MOD_CAPSLOCK) != 0;
    let is_letter = ((4..=29).contains(&key_code) && !(layout == 2 && key_code == 16))
        || (matches!(layout, 2 | 3) && key_code == 51);
    let use_shifted = if is_letter { shift ^ caps } else { shift };
    if let Some(symbol) = layout_symbol(layout, key_code, use_shifted) {
        return symbol;
    }
    let key_code = match layout {
        // German keyboards swap Y and Z positions (QWERTZ).
        1 if key_code == 28 => 29,
        1 if key_code == 29 => 28,
        // French keyboards use AZERTY letter positions.
        2 => match key_code {
            20 => 4,  // Q position produces A
            4 => 20,  // A position produces Q
            29 => 26, // Z position produces W
            26 => 29, // W position produces Z
            _ => key_code,
        },
        _ => key_code,
    };
    US_LAYOUT
        .iter()
        .find(|entry| entry.0 == key_code)
        .map(|entry| if use_shifted { entry.2 } else { entry.1 })
        .unwrap_or(0)
}

fn layout_symbol(layout: u8, key: u8, shifted: bool) -> Option<u16> {
    let pair = match (layout, key) {
        // The following pairs copy the printable normal/shifted columns from
        // ukbd_keydesc_*. Unlisted codes use the US base table.
        (1, 31) => (b'2' as u16, b'"' as u16),
        (1, 32) => (b'3' as u16, 0x00a7),
        (1, 35) => (b'6' as u16, b'&' as u16),
        (1, 36) => (b'7' as u16, b'/' as u16),
        (1, 37) => (b'8' as u16, b'(' as u16),
        (1, 38) => (b'9' as u16, b')' as u16),
        (1, 39) => (b'0' as u16, b'=' as u16),
        (1, 45) => (0x00df, b'?' as u16),
        (1, 46) => (0, 0),
        (1, 47) => (0x00fc, 0x00dc),
        (1, 48) => (b'+' as u16, b'*' as u16),
        (1, 49) => (b'#' as u16, b'\'' as u16),
        (1, 50) => (b'#' as u16, b'\'' as u16),
        (1, 51) => (0x00f6, 0x00d6),
        (1, 52) => (0x00e4, 0x00c4),
        (1, 53) => (0, 0),
        (1, 54) => (b',' as u16, b';' as u16),
        (1, 55) => (b'.' as u16, b':' as u16),
        (1, 56) => (b'-' as u16, b'_' as u16),
        (2, 20) => (b'a' as u16, b'A' as u16),
        (2, 4) => (b'q' as u16, b'Q' as u16),
        (2, 29) => (b'w' as u16, b'W' as u16),
        (2, 26) => (b'z' as u16, b'Z' as u16),
        (2, 30) => (b'&' as u16, b'1' as u16),
        (2, 31) => (0x00e9, b'2' as u16),
        (2, 32) => (b'"' as u16, b'3' as u16),
        (2, 33) => (b'\'' as u16, b'4' as u16),
        (2, 34) => (b'(' as u16, b'5' as u16),
        (2, 35) => (b'-' as u16, b'6' as u16),
        (2, 36) => (0x00e8, b'7' as u16),
        (2, 38) => (0x00e7, b'9' as u16),
        (2, 39) => (0x00e0, b'0' as u16),
        (2, 45) => (b')' as u16, 0x00b0),
        (2, 46) => (b'=' as u16, b'+' as u16),
        (2, 47) => (0, 0),
        (2, 48) => (b'$' as u16, 0x00a3),
        (2, 49) => (b'*' as u16, 0x00b5),
        (2, 50) => (b'*' as u16, 0x00b5),
        (2, 16) => (b',' as u16, b'?' as u16),
        (2, 51) => (b'm' as u16, b'M' as u16),
        (2, 52) => (0x00f9, b'%' as u16),
        (2, 53) => (0x00b2, 0x00b2),
        (2, 54) => (b';' as u16, b'.' as u16),
        (2, 55) => (b':' as u16, b'/' as u16),
        (2, 56) => (b'!' as u16, 0x00a7),
        (3, 30) => (b'1' as u16, b'!' as u16),
        (3, 31) => (b'2' as u16, b'"' as u16),
        (3, 32) => (b'3' as u16, 0x00b7),
        (3, 33) => (b'4' as u16, b'$' as u16),
        (3, 35) => (b'6' as u16, b'&' as u16),
        (3, 36) => (b'7' as u16, b'/' as u16),
        (3, 37) => (b'8' as u16, b'(' as u16),
        (3, 38) => (b'9' as u16, b')' as u16),
        (3, 39) => (b'0' as u16, b'=' as u16),
        (3, 45) => (b'\'' as u16, b'?' as u16),
        (3, 46) => (0x00a1, 0x00bf),
        (3, 47) => (0, 0),
        (3, 48) => (b'+' as u16, b'*' as u16),
        (3, 49) => (0x00e7, 0x00c7),
        (3, 50) => (0x00e7, 0x00c7),
        (3, 51) => (0x00f1, 0x00d1),
        (3, 52) => (0, 0),
        (3, 53) => (0x00b0, 0x00aa),
        (3, 54) => (b',' as u16, b';' as u16),
        (3, 55) => (b'.' as u16, b':' as u16),
        (3, 56) => (b'-' as u16, b'_' as u16),
        (4, 31) => (b'2' as u16, b'"' as u16),
        (4, 32) => (b'3' as u16, 0x00a3),
        (4, 34) => (b'5' as u16, b'%' as u16),
        (4, 35) => (b'6' as u16, b'&' as u16),
        (4, 36) => (b'7' as u16, b'/' as u16),
        (4, 37) => (b'8' as u16, b'(' as u16),
        (4, 38) => (b'9' as u16, b')' as u16),
        (4, 39) => (b'0' as u16, b'=' as u16),
        (4, 45) => (b'\'' as u16, b'?' as u16),
        (4, 46) => (0x00ec, b'^' as u16),
        (4, 47) => (0x00e8, 0x00e9),
        (4, 48) => (b'+' as u16, b'*' as u16),
        (4, 49) => (0x00f9, 0x00a7),
        (4, 50) => (0x00f9, 0x00a7),
        (4, 51) => (0x00f2, 0x00c7),
        (4, 52) => (0x00e0, 0x00b0),
        (4, 53) => (b'\\' as u16, b'|' as u16),
        (4, 54) => (b',' as u16, b';' as u16),
        (4, 55) => (b'.' as u16, b':' as u16),
        (4, 56) => (b'-' as u16, b'_' as u16),
        (5, 30) => (b'1' as u16, b'!' as u16),
        (5, 31) => (b'2' as u16, b'"' as u16),
        (5, 32) => (b'3' as u16, b'#' as u16),
        (5, 33) => (b'4' as u16, b'$' as u16),
        (5, 34) => (b'5' as u16, b'%' as u16),
        (5, 35) => (b'6' as u16, b'&' as u16),
        (5, 36) => (b'7' as u16, b'_' as u16),
        (5, 37) => (b'8' as u16, b'(' as u16),
        (5, 38) => (b'9' as u16, b')' as u16),
        (5, 39) => (b'0' as u16, b'\'' as u16),
        (5, 45) => (b'/' as u16, b'?' as u16),
        (5, 46) => (0x00b0, 0),
        (5, 47) => (0, 0),
        (5, 48) => (b'*' as u16, b'|' as u16),
        (5, 49) => (b'<' as u16, b'>' as u16),
        (5, 50) => (b'<' as u16, b'>' as u16),
        (5, 51) => (b'+' as u16, 0x00b1),
        (5, 52) => (0, 0),
        (5, 53) => (b'@' as u16, 0x00a7),
        (5, 54) => (b',' as u16, b';' as u16),
        (5, 55) => (b'.' as u16, b':' as u16),
        (5, 56) => (b'-' as u16, b'=' as u16),
        (6, 31) => (b'2' as u16, b'"' as u16),
        (6, 35) => (b'6' as u16, b'&' as u16),
        (6, 36) => (b'7' as u16, b'\'' as u16),
        (6, 37) => (b'8' as u16, b'(' as u16),
        (6, 38) => (b'9' as u16, b')' as u16),
        (6, 39) => (b'0' as u16, b'0' as u16),
        (6, 45) => (b'-' as u16, b'=' as u16),
        (6, 46) => (b'^' as u16, b'~' as u16),
        (6, 47) => (b'@' as u16, b'`' as u16),
        (6, 48) => (b'[' as u16, b'{' as u16),
        (6, 49) | (6, 50) => (b']' as u16, b'}' as u16),
        (6, 51) => (b';' as u16, b'+' as u16),
        (6, 52) => (b':' as u16, b'*' as u16),
        (6, 53) => (0, 0),
        _ => return None,
    };
    Some(if shifted { pair.1 } else { pair.0 })
}

fn layout_from_name(name: &[u8]) -> Option<u8> {
    match name {
        b"us" => Some(0),
        b"de" | b"de-de" => Some(1),
        b"de-nodead" | b"de-de-nodead" => Some(1),
        b"fr" | b"fr-fr" | b"fr-azerty" => Some(2),
        b"es" | b"es-es" => Some(3),
        b"it" | b"it-it" => Some(4),
        b"nl" | b"nl-nl" => Some(5),
        b"jp" | b"jp-jp" => Some(6),
        _ => None,
    }
}

fn layout_from_map_contents(contents: &[u8]) -> Option<u8> {
    if contents.is_empty() || contents.len() >= 64 {
        return None;
    }
    let end = contents
        .iter()
        .position(|byte| !byte.is_ascii_alphabetic() && *byte != b'-')
        .unwrap_or(contents.len());
    layout_from_name(&contents[..end])
}

fn layout_from_console_language(language: i32) -> u8 {
    match language {
        x if x == ogc_conf::CONF_LANG_GERMAN as i32 => 1,
        x if x == ogc_conf::CONF_LANG_FRENCH as i32 => 2,
        x if x == ogc_conf::CONF_LANG_SPANISH as i32 => 3,
        x if x == ogc_conf::CONF_LANG_ITALIAN as i32 => 4,
        x if x == ogc_conf::CONF_LANG_DUTCH as i32 => 5,
        x if x == ogc_conf::CONF_LANG_JAPANESE as i32 => 6,
        _ => 0,
    }
}

#[cfg(target_arch = "powerpc")]
fn select_keyboard_layout() {
    let path = c"/wiikbd.map";
    if let Some(file) = ogc_sysbase::DeviceFile::open_read(path) {
        let mut name = [0u8; 64];
        if let Ok(count) = file.read(&mut name)
            && let Some(layout) = layout_from_map_contents(&name[..count])
        {
            KEYBOARD_LAYOUT.store(layout, Ordering::Release);
            return;
        }
    }

    KEYBOARD_LAYOUT.store(
        layout_from_console_language(ogc_conf::get_language()),
        Ordering::Release,
    );
}

fn lock_modifier_for_keycode(keycode: u8) -> Option<(u16, i32)> {
    match keycode {
        57 => Some((MOD_CAPSLOCK, USBKEYBOARD_LEDCAPS)),
        71 => Some((MOD_HOLDSCREEN, USBKEYBOARD_LEDSCROLL)),
        83 => Some((MOD_NUMLOCK, USBKEYBOARD_LEDNUM)),
        _ => None,
    }
}

fn toggle_lock_modifier(modifiers: u16, keycode: u8) -> Option<(u16, i32, bool)> {
    let (mask, led) = lock_modifier_for_keycode(keycode)?;
    let modifiers = modifiers ^ mask;
    Some((modifiers, led, modifiers & mask != 0))
}

fn submit_usb_event(type_: i32, key_code: u8) {
    let event = UsbKeyboardEvent { type_, key_code };
    if CALLBACK_PRESENT.load(Ordering::Acquire) {
        unsafe {
            if let Some(cb) = USB_CALLBACK {
                cb(event);
            }
        }
    }

    if !KEYBOARD_INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    let mut modifiers = ACTIVE_MODIFIERS.load(Ordering::Acquire);
    let mod_mask = modifier_mask_for_keycode(key_code);
    let keyboard_type = match type_ {
        USBKEYBOARD_PRESSED => KEYBOARD_PRESSED,
        USBKEYBOARD_RELEASED => KEYBOARD_RELEASED,
        USBKEYBOARD_DISCONNECTED => KEYBOARD_DISCONNECTED,
        _ => return,
    };
    let symbol = unsafe {
        if keyboard_type == KEYBOARD_DISCONNECTED {
            ACTIVE_MODIFIERS.store(0, Ordering::Release);
            HELD_SYMBOLS = [0; 256];
            modifiers = 0;
            0
        } else {
            if mod_mask != 0 {
                match keyboard_type {
                    KEYBOARD_PRESSED => modifiers |= mod_mask,
                    KEYBOARD_RELEASED => modifiers &= !mod_mask,
                    _ => {}
                }
                ACTIVE_MODIFIERS.store(modifiers, Ordering::Release);
            }
            if keyboard_type == KEYBOARD_PRESSED
                && let Some((next_modifiers, led, led_on)) =
                    toggle_lock_modifier(modifiers, key_code)
            {
                modifiers = next_modifiers;
                ACTIVE_MODIFIERS.store(modifiers, Ordering::Release);
                let _ = set_led(led, led_on);
            }

            match keyboard_type {
                KEYBOARD_RELEASED => {
                    let symbol = HELD_SYMBOLS[key_code as usize];
                    HELD_SYMBOLS[key_code as usize] = 0;
                    symbol
                }
                KEYBOARD_PRESSED => {
                    let symbol = translate_symbol(key_code, modifiers);
                    if symbol != 0 {
                        HELD_SYMBOLS[key_code as usize] = symbol;
                    }
                    symbol
                }
                _ => 0,
            }
        }
    };
    let _ = queue_push(KeyboardEvent {
        type_: keyboard_type,
        modifiers,
        keycode: key_code,
        symbol,
    });
}

#[cfg(target_arch = "powerpc")]
fn pump_stdin_event() {
    let callback = unsafe { core::ptr::addr_of!(KEYBOARD_CALLBACK).read() };
    if callback.is_none() || !stdin_key_has_space() {
        return;
    }
    let Some(event) = queue_pop() else {
        return;
    };
    if event.type_ == KEYBOARD_PRESSED && event.symbol != 0 {
        let _ = stdin_key_try_push(event.symbol as u8);
    }
}

fn push_connected_event() {
    if KEYBOARD_INITIALIZED.load(Ordering::Acquire) {
        let _ = queue_push(KeyboardEvent {
            type_: KEYBOARD_CONNECTED,
            modifiers: 0,
            keycode: 0,
            symbol: 0,
        });
    }
}

fn set_led_bit(led: i32, on: bool) -> i32 {
    if !USB_CONNECTED.load(Ordering::Acquire) {
        return -1;
    }
    if !(USBKEYBOARD_LEDNUM..=USBKEYBOARD_LEDSCROLL).contains(&led) {
        return -1;
    }
    let mask = 1u8 << led;
    let mut leds = LED_BITS.load(Ordering::Acquire);
    if on {
        leds |= mask;
    } else {
        leds &= !mask;
    }
    LED_BITS.store(leds, Ordering::Release);
    1
}

fn reset_scan_state() {
    unsafe {
        LAST_SCAN = ScanSnapshot::default();
        NEXT_SCAN = ScanSnapshot::default();
        HELD_SYMBOLS = [0; 256];
    }
    ACTIVE_MODIFIERS.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
unsafe fn powerpc_keyboard() -> &'static mut UsbKeyboardDevice {
    unsafe { &mut *core::ptr::addr_of_mut!(POWERPC_KEYBOARD) }
}

#[cfg(target_arch = "powerpc")]
fn powerpc_reset_scan_state() {
    unsafe {
        POWERPC_KEYBOARD.sc_ndata = ScanSnapshot::default();
        POWERPC_KEYBOARD.sc_odata = ScanSnapshot::default();
    }
    reset_scan_state();
}

#[cfg(target_arch = "powerpc")]
fn powerpc_set_leds(leds: u8) -> i32 {
    unsafe {
        let kbd = powerpc_keyboard();
        if !kbd.connected || kbd.fd < 0 {
            return -1;
        }
        let mut report = UsbKeyboardByte([leds]);
        let rc = usb::write_ctrl_msg(
            kbd.fd,
            usb::USB_REQTYPE_INTERFACE_SET,
            usb::USB_REQ_SETREPORT,
            (usb::USB_REPTYPE_OUTPUT as u16) << 8,
            kbd.interface as u16,
            1,
            report.0.as_mut_ptr().cast::<c_void>(),
        );
        if rc < 0 {
            return -2;
        }
        kbd.leds = leds;
        1
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn powerpc_disconnect_callback(_result: i32, _userdata: *mut c_void) -> i32 {
    unsafe {
        let kbd = powerpc_keyboard();
        kbd.connected = false;
        USB_CONNECTED.store(false, Ordering::Release);
        // The USB-layer callback exists independently of KEYBOARD_Init in C.
        // submit_usb_event invokes that callback before applying the high-level
        // keyboard initialization gate.
        submit_usb_event(USBKEYBOARD_DISCONNECTED, 0);
        IPC_OK
    }
}

#[cfg(target_arch = "powerpc")]
fn powerpc_find_keyboard() -> i32 {
    let mut entries = [usb::UsbDeviceEntry::default(); 8];
    let count = match usb::device_list(&mut entries, usb::USB_CLASS_HID) {
        Ok(count) => count,
        Err(rc) => return rc.raw(),
    };

    for entry in entries.iter().take(count) {
        if entry.vid == 0 || entry.pid == 0 {
            continue;
        }

        let fd = match usb::open_device_handle(entry.device_id, entry.vid, entry.pid) {
            Ok(fd) => fd,
            Err(_) => continue,
        };

        let mut desc = match usb::descriptors(fd) {
            Ok(desc) => desc,
            Err(_) => {
                let mut fd = fd;
                let _ = usb::close_device_handle(&mut fd);
                continue;
            }
        };

        let mut found = None;
        for i_conf in 0..desc.b_num_configurations as usize {
            unsafe {
                let cfg = &*desc.configurations.add(i_conf);
                for i_iface in 0..cfg.b_num_interfaces as usize {
                    let iface = &*cfg.interfaces.add(i_iface);
                    if iface.b_interface_class != usb::USB_CLASS_HID
                        || iface.b_interface_sub_class != usb::USB_SUBCLASS_BOOT
                        || iface.b_interface_protocol != usb::USB_PROTOCOL_KEYBOARD
                    {
                        continue;
                    }

                    for i_ep in 0..iface.b_num_endpoints as usize {
                        let ep = &*iface.endpoints.add(i_ep);
                        if ep.bm_attributes != usb::USB_ENDPOINT_INTERRUPT {
                            continue;
                        }
                        if ep.b_endpoint_address & usb::USB_ENDPOINT_IN == 0 {
                            continue;
                        }
                        found = Some((
                            cfg.b_configuration_value,
                            iface.b_interface_number,
                            iface.b_alternate_setting,
                            ep.b_endpoint_address,
                            ep.w_max_packet_size,
                        ));
                        break;
                    }

                    if found.is_some() {
                        break;
                    }
                }
            }

            if found.is_some() {
                break;
            }
        }

        unsafe { usb::free_descriptors(core::ptr::addr_of_mut!(desc)) };

        let Some((configuration, interface, alt_interface, ep, ep_size)) = found else {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        };

        let current_configuration = match usb::configuration(fd) {
            Ok(configuration) => configuration,
            Err(_) => {
                let mut fd = fd;
                let _ = usb::close_device_handle(&mut fd);
                continue;
            }
        };
        if current_configuration != configuration && usb::set_configuration(fd, configuration) < 0 {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        }

        if alt_interface != 0 && usb::set_alternative_interface(fd, interface, alt_interface) < 0 {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        }

        let mut protocol = UsbKeyboardByte([0]);
        if unsafe {
            usb::read_ctrl_msg(
                fd,
                usb::USB_REQTYPE_INTERFACE_GET,
                usb::USB_REQ_GETPROTOCOL,
                0,
                interface as u16,
                1,
                protocol.0.as_mut_ptr().cast::<c_void>(),
            )
        } < 0
        {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        }

        if protocol.0[0] != 0
            && unsafe {
                usb::write_ctrl_msg(
                    fd,
                    usb::USB_REQTYPE_INTERFACE_SET,
                    usb::USB_REQ_SETPROTOCOL,
                    0,
                    interface as u16,
                    0,
                    null_mut(),
                )
            } < 0
        {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        }

        if protocol.0[0] != 0 {
            protocol.0[0] = 0xff;
            if unsafe {
                usb::read_ctrl_msg(
                    fd,
                    usb::USB_REQTYPE_INTERFACE_GET,
                    usb::USB_REQ_GETPROTOCOL,
                    0,
                    interface as u16,
                    1,
                    protocol.0.as_mut_ptr().cast::<c_void>(),
                )
            } < 0
                || protocol.0[0] != 0
            {
                let mut fd = fd;
                let _ = usb::close_device_handle(&mut fd);
                continue;
            }
        }

        if unsafe {
            usb::device_removal_notify_async(fd, Some(powerpc_disconnect_callback), null_mut())
        } < 0
        {
            let mut fd = fd;
            let _ = usb::close_device_handle(&mut fd);
            continue;
        }

        unsafe {
            let kbd = powerpc_keyboard();
            kbd.connected = true;
            kbd.fd = fd;
            kbd.cb = None;
            kbd.configuration = configuration;
            kbd.interface = interface;
            kbd.alt_interface = alt_interface;
            kbd.ep = ep;
            kbd.ep_size = ep_size;
            kbd.leds = 0;
            kbd.sc_ndata = ScanSnapshot::default();
            kbd.sc_odata = ScanSnapshot::default();
        }

        USB_CONNECTED.store(true, Ordering::Release);
        LED_BITS.store(0, Ordering::Release);
        reset_scan_state();
        // Match libwiikeyboard: flash all lock LEDs for 200 ms before
        // publishing the connected event. LED transport errors are ignored.
        let _ = powerpc_set_leds(0b111);
        LED_BITS.store(0b111, Ordering::Release);
        lwp::sleep_ticks(KBD_CONNECT_LED_FLASH_TICKS.max(1));
        let _ = powerpc_set_leds(0);
        LED_BITS.store(0, Ordering::Release);
        push_connected_event();
        return 1;
    }

    -3
}

pub fn initialize() -> i32 {
    if cfg!(target_arch = "powerpc") {
        if USB_INITIALIZED.load(Ordering::Acquire) {
            return 0;
        }
        let rc = usb::initialize();
        if rc == usb::USB_OK {
            USB_INITIALIZED.store(true, Ordering::Release);
        }
        return rc;
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    USB_INITIALIZED.store(true, Ordering::Release);
    IPC_OK
}

pub fn deinitialize() -> i32 {
    if cfg!(target_arch = "powerpc") {
        let close_result = try_close();
        if close_result < 0 {
            return close_result;
        }
        let result = usb::deinitialize();
        if result == usb::USB_OK {
            USB_INITIALIZED.store(false, Ordering::Release);
        }
        return result;
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    USB_CONNECTED.store(false, Ordering::Release);
    USB_INITIALIZED.store(false, Ordering::Release);
    CALLBACK_PRESENT.store(false, Ordering::Release);
    LED_BITS.store(0, Ordering::Release);
    reset_scan_state();
    IPC_OK
}

/// # Safety
///
/// `cb` must remain valid for any subsequent host-compat scan events while the
/// device is open.
pub unsafe fn open(cb: EventCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if !USB_INITIALIZED.load(Ordering::Acquire) {
            return -1;
        }
        let close_result = try_close();
        if close_result < 0 {
            return close_result;
        }
        unsafe {
            USB_CALLBACK = cb;
        }
        CALLBACK_PRESENT.store(cb.is_some(), Ordering::Release);
        let rc = powerpc_find_keyboard();
        if rc != 1 {
            CALLBACK_PRESENT.store(false, Ordering::Release);
            unsafe {
                USB_CALLBACK = None;
            }
        }
        return rc;
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    if !USB_INITIALIZED.load(Ordering::Acquire) {
        return -1;
    }
    if !HOST_DEVICE_PRESENT.load(Ordering::Acquire) {
        return -3;
    }
    unsafe {
        USB_CALLBACK = cb;
    }
    CALLBACK_PRESENT.store(cb.is_some(), Ordering::Release);
    USB_CONNECTED.store(true, Ordering::Release);
    LED_BITS.store(0, Ordering::Release);
    reset_scan_state();
    push_connected_event();
    1
}

fn clear_connection_state() {
    CALLBACK_PRESENT.store(false, Ordering::Release);
    unsafe {
        USB_CALLBACK = None;
    }
    USB_CONNECTED.store(false, Ordering::Release);
    LED_BITS.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
fn close_powerpc_keyboard_with(
    kbd: &mut UsbKeyboardDevice,
    clear_halt: impl FnOnce(i32, u8) -> i32,
    close_device: impl FnOnce(&mut i32) -> i32,
) -> i32 {
    let (halt_result, close_result) = if kbd.fd != -1 {
        let halt_result = clear_halt(kbd.fd, kbd.ep);
        let close_result = close_device(&mut kbd.fd);
        (halt_result, close_result)
    } else {
        (usb::USB_OK, usb::USB_OK)
    };

    // Clear all keyboard state even when transport close fails. Keep `fd` as
    // returned by the backend so the caller can retry closing the resource.
    kbd.connected = false;
    kbd.cb = None;
    kbd.leds = 0;
    clear_connection_state();
    powerpc_reset_scan_state();

    if close_result < 0 {
        close_result
    } else {
        halt_result
    }
}

pub fn try_close() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return unsafe {
            close_powerpc_keyboard_with(powerpc_keyboard(), usb::clear_halt, |fd| {
                match usb::close_device_handle(fd) {
                    Ok(()) => usb::USB_OK,
                    Err(error) => error.raw(),
                }
            })
        };
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    if USB_CONNECTED.swap(false, Ordering::AcqRel) {
        submit_usb_event(USBKEYBOARD_DISCONNECTED, 0);
    }
    clear_connection_state();
    reset_scan_state();
    IPC_OK
}

pub fn close() {
    let result = try_close();
    USB_LAST_STATUS.store(result, Ordering::Release);
}

pub fn last_status() -> i32 {
    USB_LAST_STATUS.load(Ordering::Acquire)
}

pub fn is_connected() -> bool {
    if cfg!(target_arch = "powerpc") {
        return unsafe { POWERPC_KEYBOARD.connected };
    }
    if !keyboard_host_compatibility_enabled() {
        return false;
    }
    USB_CONNECTED.load(Ordering::Acquire)
}

pub fn scan() -> i32 {
    if cfg!(target_arch = "powerpc") {
        unsafe {
            let kbd = powerpc_keyboard();
            if !kbd.connected || kbd.fd < 0 {
                return -1;
            }

            let mut buffer = UsbKeyboardReport([0u8; 8]);
            let rc = usb::read_intr_msg(
                kbd.fd,
                kbd.ep,
                buffer.0.len().min(u16::MAX as usize) as u16,
                buffer.0.as_mut_ptr().cast::<c_void>(),
            );
            if rc < 0 {
                return -2;
            }

            kbd.sc_ndata.modifiers = u16::from(buffer.0[0]);
            kbd.sc_ndata.keycodes.copy_from_slice(&buffer.0[2..8]);
            if kbd.sc_ndata.keycodes[0] == KEY_ERROR {
                return 0;
            }

            if kbd.sc_odata.modifiers != kbd.sc_ndata.modifiers {
                for &(mask, keycode) in &[
                    (0x01u16, 224u8),
                    (0x02u16, 225u8),
                    (0x04u16, 226u8),
                    (0x08u16, 227u8),
                    (0x10u16, 228u8),
                    (0x20u16, 229u8),
                    (0x40u16, 230u8),
                    (0x80u16, 231u8),
                ] {
                    if (kbd.sc_odata.modifiers & mask) != 0 && (kbd.sc_ndata.modifiers & mask) == 0
                    {
                        submit_usb_event(USBKEYBOARD_RELEASED, keycode);
                    } else if (kbd.sc_ndata.modifiers & mask) != 0
                        && (kbd.sc_odata.modifiers & mask) == 0
                    {
                        submit_usb_event(USBKEYBOARD_PRESSED, keycode);
                    }
                }
            }

            for old in kbd.sc_odata.keycodes {
                if old > 3 && !kbd.sc_ndata.keycodes.contains(&old) {
                    submit_usb_event(USBKEYBOARD_RELEASED, old);
                }
            }
            for new in kbd.sc_ndata.keycodes {
                if new > 3 && !kbd.sc_odata.keycodes.contains(&new) {
                    submit_usb_event(USBKEYBOARD_PRESSED, new);
                }
            }

            kbd.sc_odata = kbd.sc_ndata;
        }
        return 0;
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    if !USB_CONNECTED.load(Ordering::Acquire) {
        return -1;
    }
    let previous = unsafe { LAST_SCAN };
    let next = unsafe { NEXT_SCAN };
    if next.keycodes[0] == KEY_ERROR {
        return 0;
    }

    if previous.modifiers != next.modifiers {
        for &(mask, keycode) in &[
            (0x01u16, 224u8),
            (0x02u16, 225u8),
            (0x04u16, 226u8),
            (0x08u16, 227u8),
            (0x10u16, 228u8),
            (0x20u16, 229u8),
            (0x40u16, 230u8),
            (0x80u16, 231u8),
        ] {
            if (previous.modifiers & mask) != 0 && (next.modifiers & mask) == 0 {
                submit_usb_event(USBKEYBOARD_RELEASED, keycode);
            } else if (next.modifiers & mask) != 0 && (previous.modifiers & mask) == 0 {
                submit_usb_event(USBKEYBOARD_PRESSED, keycode);
            }
        }
    }

    for old in previous.keycodes {
        if old > 3 && !next.keycodes.contains(&old) {
            submit_usb_event(USBKEYBOARD_RELEASED, old);
        }
    }
    for new in next.keycodes {
        if new > 3 && !previous.keycodes.contains(&new) {
            submit_usb_event(USBKEYBOARD_PRESSED, new);
        }
    }

    unsafe {
        LAST_SCAN = next;
    }
    0
}

pub fn set_led(led: i32, on: bool) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if !(USBKEYBOARD_LEDNUM..=USBKEYBOARD_LEDSCROLL).contains(&led) {
            return -1;
        }
        unsafe {
            let kbd = powerpc_keyboard();
            let mut leds = kbd.leds;
            let mask = 1u8 << led;
            if on {
                leds |= mask;
            } else {
                leds &= !mask;
            }
            return powerpc_set_leds(leds);
        }
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    set_led_bit(led, on)
}

pub fn toggle_led(led: i32) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if !(USBKEYBOARD_LEDNUM..=USBKEYBOARD_LEDSCROLL).contains(&led) {
            return -1;
        }
        unsafe {
            let kbd = powerpc_keyboard();
            let leds = kbd.leds ^ (1u8 << led);
            return powerpc_set_leds(leds);
        }
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    if !USB_CONNECTED.load(Ordering::Acquire) {
        return -1;
    }
    if !(USBKEYBOARD_LEDNUM..=USBKEYBOARD_LEDSCROLL).contains(&led) {
        return -1;
    }
    let mask = 1u8 << led;
    LED_BITS.fetch_xor(mask, Ordering::AcqRel);
    1
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn keyboard_thread_main(_arg: *mut c_void) -> *mut c_void {
    let mut polls = 0u32;
    while !KBD_THREAD_QUIT.load(Ordering::Acquire) {
        if polls == 0 && !is_connected() {
            let _ = unsafe { open(None) };
        }
        if is_connected() {
            let _ = scan();
        }
        // C's stdin buffer thread consumes the same public event FIFO.
        // Consume at most one event per poll while the 255-byte ring has room.
        pump_stdin_event();
        polls = (polls + 1) % KBD_DISCOVERY_INTERVAL_POLLS;
        lwp::sleep_ticks(KBD_POLL_TICKS.max(1));
    }
    core::ptr::null_mut()
}

#[cfg(target_arch = "powerpc")]
fn start_keyboard_thread() -> i32 {
    if KBD_THREAD_RUNNING.load(Ordering::Acquire) {
        return 0;
    }
    KBD_THREAD_QUIT.store(false, Ordering::Release);
    let mut thread = lwp::LWP_THREAD_NULL;
    let rc = unsafe {
        lwp::create_thread(
            &mut thread,
            Some(keyboard_thread_main),
            core::ptr::null_mut(),
            core::ptr::addr_of_mut!(KBD_THREAD_STACK.0)
                .cast::<u8>()
                .cast::<c_void>(),
            KBD_THREAD_STACK_SIZE as u32,
            KBD_THREAD_PRIORITY,
        )
    };
    if rc != lwp::LWP_SUCCESSFUL {
        return -6;
    }
    unsafe { KBD_THREAD = thread };
    KBD_THREAD_RUNNING.store(true, Ordering::Release);
    0
}

#[doc(hidden)]
pub fn start_worker_before_stdin_install(
    start_worker: impl FnOnce() -> i32,
    install_stdin: impl FnOnce(),
) -> i32 {
    let result = start_worker();
    if result == 0 {
        install_stdin();
    }
    result
}

#[cfg(target_arch = "powerpc")]
fn stop_keyboard_thread() {
    if !KBD_THREAD_RUNNING.swap(false, Ordering::AcqRel) {
        return;
    }
    KBD_THREAD_QUIT.store(true, Ordering::Release);
    let thread = unsafe { KBD_THREAD };
    if thread != lwp::LWP_THREAD_NULL {
        let _ = unsafe { lwp::join_thread(thread, core::ptr::null_mut()) };
    }
    unsafe { KBD_THREAD = lwp::LWP_THREAD_NULL };
}

/// # Safety
///
/// `keypress_cb` must remain valid for the duration of the keyboard layer if a
/// future backend begins invoking it.
pub unsafe fn init(keypress_cb: KeyPressCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if !USB_INITIALIZED.load(Ordering::Acquire) {
            let result = initialize();
            if result != IPC_OK {
                return result;
            }
        }
        select_keyboard_layout();
        KEYBOARD_INITIALIZED.store(true, Ordering::Release);
        unsafe {
            KEYBOARD_CALLBACK = keypress_cb;
        }
        ACTIVE_MODIFIERS.store(0, Ordering::Release);
        let _ = queue_flush();
        let thread_result = start_worker_before_stdin_install(start_keyboard_thread, || {
            if keypress_cb.is_some() {
                stdin_key_clear();
                let previous =
                    unsafe { ogc_sysbase::set_stdin_device(core::ptr::addr_of!(STDIN_DEVICE)) };
                if previous != core::ptr::addr_of!(STDIN_DEVICE) {
                    unsafe { PREVIOUS_STDIN_DEVICE = previous };
                }
            }
        });
        if thread_result != 0 {
            KEYBOARD_INITIALIZED.store(false, Ordering::Release);
            unsafe { KEYBOARD_CALLBACK = None };
            return thread_result;
        }
        return 0;
    }
    if !keyboard_host_compatibility_enabled() {
        return -1;
    }
    if !USB_INITIALIZED.load(Ordering::Acquire) {
        let result = initialize();
        if result != IPC_OK {
            return result;
        }
    }
    KEYBOARD_INITIALIZED.store(true, Ordering::Release);
    unsafe {
        KEYBOARD_CALLBACK = keypress_cb;
    }
    ACTIVE_MODIFIERS.store(0, Ordering::Release);
    let _ = queue_flush();
    if keypress_cb.is_some() {
        stdin_key_clear();
    }
    0
}

pub fn deinit() -> i32 {
    if !cfg!(target_arch = "powerpc") && !keyboard_host_compatibility_enabled() {
        return -1;
    }
    #[cfg(target_arch = "powerpc")]
    stop_keyboard_thread();
    // C exposes no close result, but Rust returns a failed transport close
    // after completing local keyboard cleanup.
    let close_result = try_close();
    USB_LAST_STATUS.store(close_result, Ordering::Release);
    KEYBOARD_INITIALIZED.store(false, Ordering::Release);
    unsafe {
        KEYBOARD_CALLBACK = None;
    }
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let _ = ogc_sysbase::restore_stdin_device(
            core::ptr::addr_of!(STDIN_DEVICE),
            PREVIOUS_STDIN_DEVICE,
        );
        PREVIOUS_STDIN_DEVICE = core::ptr::null();
    }
    flush_events();
    if close_result < 0 { close_result } else { 1 }
}

/// # Safety
///
/// `event` must be a valid writable pointer.
pub unsafe fn get_event(event: *mut KeyboardEvent) -> i32 {
    if cfg!(target_arch = "powerpc") {
        if event.is_null() {
            return 0;
        }
        let Some(next) = queue_pop() else {
            return 0;
        };
        unsafe {
            *event = next;
        }
        return 1;
    }
    if !keyboard_host_compatibility_enabled() {
        return 0;
    }
    if event.is_null() {
        return 0;
    }
    let Some(next) = queue_pop() else {
        return 0;
    };
    unsafe {
        *event = next;
    }
    1
}

#[cfg(all(test, target_arch = "powerpc"))]
mod stdin_tests {
    use super::*;

    static CALLBACK_COUNT: AtomicU8 = AtomicU8::new(0);
    static CALLBACK_BYTE: AtomicU8 = AtomicU8::new(0);

    unsafe extern "C" fn record_keypress(symbol: c_char) {
        CALLBACK_BYTE.store(symbol as u8, Ordering::Release);
        CALLBACK_COUNT.fetch_add(1, Ordering::AcqRel);
    }

    #[test]
    fn stdin_pump_consumes_public_event_fifo() {
        stdin_key_clear();
        let _ = queue_flush();
        unsafe { KEYBOARD_CALLBACK = Some(record_keypress) };
        assert!(queue_push(KeyboardEvent {
            type_: KEYBOARD_CONNECTED,
            modifiers: 0,
            keycode: 0,
            symbol: 0,
        }));
        assert!(queue_push(KeyboardEvent {
            type_: KEYBOARD_PRESSED,
            modifiers: 0,
            keycode: 4,
            symbol: b'a' as u16,
        }));

        pump_stdin_event();
        assert_eq!(queue_pop().map(|event| event.type_), Some(KEYBOARD_PRESSED));
        assert_eq!(stdin_key_pop(), None);

        assert!(queue_push(KeyboardEvent {
            type_: KEYBOARD_PRESSED,
            modifiers: 0,
            keycode: 4,
            symbol: b'a' as u16,
        }));
        pump_stdin_event();
        assert_eq!(stdin_key_pop(), Some(b'a'));
        unsafe { KEYBOARD_CALLBACK = None };
        let _ = queue_flush();
    }

    #[test]
    fn stdin_callback_runs_once_when_read_consumes_key() {
        CALLBACK_COUNT.store(0, Ordering::Release);
        CALLBACK_BYTE.store(0, Ordering::Release);
        stdin_key_clear();
        let _ = queue_flush();
        ACTIVE_MODIFIERS.store(0, Ordering::Release);
        KEYBOARD_INITIALIZED.store(true, Ordering::Release);
        unsafe { KEYBOARD_CALLBACK = Some(record_keypress) };

        submit_usb_event(USBKEYBOARD_PRESSED, 4);
        assert_eq!(CALLBACK_COUNT.load(Ordering::Acquire), 0);

        let mut byte = 0;
        let result = unsafe {
            keyboard_stdin_read(core::ptr::null_mut(), core::ptr::null_mut(), &mut byte, 1)
        };

        assert_eq!(result, 1);
        assert_eq!(byte as u8, b'a');
        assert_eq!(CALLBACK_COUNT.load(Ordering::Acquire), 1);
        assert_eq!(CALLBACK_BYTE.load(Ordering::Acquire), b'a');

        unsafe { KEYBOARD_CALLBACK = None };
        KEYBOARD_INITIALIZED.store(false, Ordering::Release);
        stdin_key_clear();
    }
}

pub fn flush_events() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return queue_flush();
    }
    if !keyboard_host_compatibility_enabled() {
        return 0;
    }
    queue_flush()
}

#[cfg(all(test, target_arch = "powerpc"))]
fn host_set_device_present(present: bool) {
    HOST_DEVICE_PRESENT.store(present, Ordering::Release);
}

#[cfg(all(test, target_arch = "powerpc"))]
fn host_set_scan_report(modifiers: u16, keycodes: [u8; MAX_SCAN_KEYS]) {
    unsafe {
        NEXT_SCAN = ScanSnapshot {
            modifiers,
            keycodes,
        };
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
fn host_led_bits() -> u8 {
    LED_BITS.load(Ordering::Acquire)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};
    use core::sync::atomic::AtomicBool;
    use core::sync::atomic::{AtomicI32, AtomicU32};

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);
    static CALLBACK_EVENTS: AtomicU32 = AtomicU32::new(0);
    static CALLBACK_LAST_TYPE: AtomicI32 = AtomicI32::new(-1);
    static CALLBACK_LAST_KEY: AtomicU8 = AtomicU8::new(0);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            let _ = deinit();
            let _ = queue_flush();
            host_set_device_present(false);
            CALLBACK_EVENTS.store(0, Ordering::Release);
            CALLBACK_LAST_TYPE.store(-1, Ordering::Release);
            CALLBACK_LAST_KEY.store(0, Ordering::Release);
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        let _ = deinitialize();
        let _ = flush_events();
        TestGuard
    }

    unsafe extern "C" fn event_cb(event: UsbKeyboardEvent) {
        CALLBACK_EVENTS.fetch_add(1, Ordering::AcqRel);
        CALLBACK_LAST_TYPE.store(event.type_, Ordering::Release);
        CALLBACK_LAST_KEY.store(event.key_code, Ordering::Release);
    }

    #[test]
    fn constants_and_abi_shapes_match_headers() {
        let _guard = test_guard();
        assert_eq!(MOD_SHIFT_L, 1);
        assert_eq!(MOD_NUMLOCK, 1 << 9);
        assert_eq!(KEYBOARD_CONNECTED, 0);
        assert_eq!(KEYBOARD_RELEASED, 3);
        assert_eq!(USBKEYBOARD_PRESSED, 0);
        assert_eq!(USBKEYBOARD_DISCONNECTED, 2);
        assert_eq!(USBKEYBOARD_LEDNUM, 0);
        assert_eq!(USBKEYBOARD_LEDSCROLL, 2);

        assert_eq!(size_of::<KeyboardEvent>(), 12);
        assert_eq!(align_of::<KeyboardEvent>(), align_of::<i32>());
        assert_eq!(offset_of!(KeyboardEvent, type_), 0);
        assert_eq!(offset_of!(KeyboardEvent, modifiers), 4);
        assert_eq!(offset_of!(KeyboardEvent, keycode), 6);
        assert_eq!(offset_of!(KeyboardEvent, symbol), 8);

        assert_eq!(size_of::<UsbKeyboardEvent>(), 8);
        assert_eq!(offset_of!(UsbKeyboardEvent, type_), 0);
        assert_eq!(offset_of!(UsbKeyboardEvent, key_code), 4);
    }

    #[test]
    fn modifier_keycodes_match_libogc_mapping() {
        let _guard = test_guard();
        assert_eq!(modifier_mask_for_keycode(224), MOD_CONTROL_L);
        assert_eq!(modifier_mask_for_keycode(225), MOD_SHIFT_L);
        assert_eq!(modifier_mask_for_keycode(226), MOD_META_L);
        assert_eq!(modifier_mask_for_keycode(227), 0);
        assert_eq!(modifier_mask_for_keycode(228), MOD_CONTROL_R);
        assert_eq!(modifier_mask_for_keycode(229), MOD_SHIFT_R);
        assert_eq!(modifier_mask_for_keycode(230), MOD_META_R);
        assert_eq!(modifier_mask_for_keycode(231), 0);
    }

    #[test]
    fn lock_keys_toggle_c_modifiers_and_leds() {
        let _guard = test_guard();

        assert_eq!(
            toggle_lock_modifier(0, 57),
            Some((MOD_CAPSLOCK, USBKEYBOARD_LEDCAPS, true))
        );
        assert_eq!(
            toggle_lock_modifier(MOD_CAPSLOCK, 57),
            Some((0, USBKEYBOARD_LEDCAPS, false))
        );
        assert_eq!(
            toggle_lock_modifier(0, 71),
            Some((MOD_HOLDSCREEN, USBKEYBOARD_LEDSCROLL, true))
        );
        assert_eq!(
            toggle_lock_modifier(0, 83),
            Some((MOD_NUMLOCK, USBKEYBOARD_LEDNUM, true))
        );
        assert_eq!(toggle_lock_modifier(0, 4), None);
    }

    #[test]
    fn modifier_and_lock_key_events_keep_libogc_symbols() {
        let _guard = test_guard();
        let _ = queue_flush();
        ACTIVE_MODIFIERS.store(0, Ordering::Release);
        KEYBOARD_INITIALIZED.store(true, Ordering::Release);

        // C's US map emits KS_Caps_Lock for HID code 57 after toggling the
        // Caps Lock state, and KS_Shift_L for code 225.
        submit_usb_event(USBKEYBOARD_PRESSED, 57);
        let caps = queue_pop().expect("Caps Lock event is queued");
        assert_eq!(caps.keycode, 57);
        assert_eq!(caps.symbol, 0xf105);
        assert_eq!(caps.modifiers, MOD_CAPSLOCK);

        submit_usb_event(USBKEYBOARD_PRESSED, 225);
        let shift = queue_pop().expect("Shift event is queued");
        assert_eq!(shift.keycode, 225);
        assert_eq!(shift.symbol, 0xf101);
        assert_eq!(shift.modifiers, MOD_CAPSLOCK | MOD_SHIFT_L);
    }

    #[test]
    fn function_keys_match_libogc_symbols() {
        for (offset, keycode) in (58u8..=69).enumerate() {
            let offset = offset as u16;
            assert_eq!(translate_symbol(keycode, 0), KS_CMD_SCREEN0 + offset);
            assert_eq!(translate_symbol(keycode, MOD_SHIFT_L), KS_LOWER_F1 + offset);
            assert_eq!(translate_symbol(keycode, MOD_SHIFT_R), KS_LOWER_F1 + offset);
            assert_eq!(
                translate_symbol(keycode, MOD_SHIFTLOCK),
                KS_LOWER_F1 + offset
            );
            assert_eq!(
                translate_symbol(keycode, MOD_CAPSLOCK),
                KS_CMD_SCREEN0 + offset
            );
        }
        assert_eq!(KS_F20, 0xf353);
    }

    #[test]
    fn event_queue_grows_past_the_old_ring_limit() {
        let _guard = test_guard();
        let _ = queue_flush();
        for index in 0..64u8 {
            assert!(queue_push(KeyboardEvent {
                type_: KEYBOARD_PRESSED,
                modifiers: 0,
                keycode: index,
                symbol: u16::from(index),
            }));
        }
        for index in 0..64u8 {
            assert_eq!(queue_pop().map(|event| event.keycode), Some(index));
        }
        assert_eq!(queue_pop(), None);
    }

    #[test]
    fn stdin_queue_matches_libogc_ring_capacity() {
        let _guard = test_guard();
        stdin_key_clear();
        for value in 0..STDIN_BUFFER_CAPACITY {
            assert!(stdin_key_try_push(value as u8));
        }
        assert!(!stdin_key_try_push(0xff));
        for value in 0..STDIN_BUFFER_CAPACITY {
            assert_eq!(stdin_key_pop(), Some(value as u8));
        }
        assert_eq!(stdin_key_pop(), None);
    }

    #[test]
    fn return_key_matches_libogc_carriage_return_symbol() {
        let _guard = test_guard();
        for layout in 0..=6 {
            KEYBOARD_LAYOUT.store(layout, Ordering::Release);
            assert_eq!(translate_symbol(40, 0), 0x000d, "layout {layout}");
            assert_eq!(translate_symbol(40, MOD_SHIFT_L), 0x000d, "layout {layout}");
        }
    }

    #[test]
    fn layout_names_and_console_layouts_change_symbol_translation() {
        let _guard = test_guard();
        assert_eq!(layout_from_name(b"de"), Some(1));
        assert_eq!(layout_from_name(b"fr-azerty"), Some(2));
        assert_eq!(layout_from_name(b"es"), Some(3));
        assert_eq!(layout_from_name(b"it"), Some(4));
        assert_eq!(layout_from_name(b"nl"), Some(5));
        assert_eq!(layout_from_name(b"jp"), Some(6));
        assert_eq!(layout_from_name(b"unknown"), None);

        KEYBOARD_LAYOUT.store(0, Ordering::Release);
        assert_eq!(translate_symbol(28, 0), b'y' as u16);
        KEYBOARD_LAYOUT.store(1, Ordering::Release);
        assert_eq!(translate_symbol(28, 0), b'z' as u16);
        KEYBOARD_LAYOUT.store(2, Ordering::Release);
        assert_eq!(translate_symbol(20, 0), b'a' as u16);
        assert_eq!(translate_symbol(20, MOD_CAPSLOCK), b'A' as u16);
        assert_eq!(translate_symbol(16, MOD_CAPSLOCK), b',' as u16);
        KEYBOARD_LAYOUT.store(3, Ordering::Release);
        assert_eq!(translate_symbol(51, MOD_CAPSLOCK), 0x00d1);
    }

    #[test]
    fn keyboard_map_matches_c_file_size_limits() {
        assert_eq!(layout_from_map_contents(b"de"), Some(1));
        assert_eq!(layout_from_map_contents(b""), None);
        assert_eq!(layout_from_map_contents(&[b'd'; 64]), None);
    }

    #[test]
    fn selected_layouts_translate_shifted_and_unshifted_symbols() {
        let _guard = test_guard();
        let expected = [
            (1, 45, 0x00df, b'?' as u16),
            (2, 31, 0x00e9, b'2' as u16),
            (3, 51, 0x00f1, 0x00d1),
            (4, 47, 0x00e8, 0x00e9),
            (5, 46, 0x00b0, 0),
            (6, 47, b'@' as u16, b'`' as u16),
        ];
        for (layout, key, plain, shifted) in expected {
            assert_eq!(layout_symbol(layout, key, false), Some(plain));
            assert_eq!(layout_symbol(layout, key, true), Some(shifted));
        }
        assert_eq!(
            layout_from_console_language(ogc_conf::CONF_LANG_JAPANESE as i32),
            6
        );
        assert_eq!(
            layout_from_console_language(ogc_conf::CONF_LANG_ENGLISH as i32),
            0
        );
    }

    #[test]
    fn all_selected_layouts_match_reference_number_row() {
        let _guard = test_guard();
        let expected: [[(u16, u16); 10]; 7] = [
            [
                (49, 33),
                (50, 64),
                (51, 35),
                (52, 36),
                (53, 37),
                (54, 94),
                (55, 38),
                (56, 42),
                (57, 40),
                (48, 41),
            ],
            [
                (49, 33),
                (50, 34),
                (51, 0x00a7),
                (52, 36),
                (53, 37),
                (54, 38),
                (55, 47),
                (56, 40),
                (57, 41),
                (48, 61),
            ],
            [
                (38, 49),
                (0x00e9, 50),
                (34, 51),
                (39, 52),
                (40, 53),
                (45, 54),
                (0x00e8, 55),
                (95, 56),
                (0x00e7, 57),
                (0x00e0, 48),
            ],
            [
                (49, 33),
                (50, 34),
                (51, 0x00b7),
                (52, 36),
                (53, 37),
                (54, 38),
                (55, 47),
                (56, 40),
                (57, 41),
                (48, 61),
            ],
            [
                (49, 33),
                (50, 34),
                (51, 0x00a3),
                (52, 36),
                (53, 37),
                (54, 38),
                (55, 47),
                (56, 40),
                (57, 41),
                (48, 61),
            ],
            [
                (49, 33),
                (50, 34),
                (51, 35),
                (52, 36),
                (53, 37),
                (54, 38),
                (55, 95),
                (56, 40),
                (57, 41),
                (48, 39),
            ],
            [
                (49, 33),
                (50, 34),
                (51, 35),
                (52, 36),
                (53, 37),
                (54, 38),
                (55, 39),
                (56, 40),
                (57, 41),
                (48, 48),
            ],
        ];
        for (layout, row) in expected.iter().enumerate() {
            KEYBOARD_LAYOUT.store(layout as u8, Ordering::Release);
            for (offset, &(plain, shifted)) in row.iter().enumerate() {
                let code = 30 + offset as u8;
                assert_eq!(
                    translate_symbol(code, 0),
                    plain,
                    "layout {layout}, key {code}"
                );
                assert_eq!(
                    translate_symbol(code, MOD_SHIFT_L),
                    shifted,
                    "shifted layout {layout}, key {code}"
                );
            }
        }
    }

    #[test]
    fn selected_layout_letters_and_punctuation_match_reference_pairs() {
        let _guard = test_guard();
        let expected = [
            (0, 4, b'a' as u16, b'A' as u16),
            (1, 28, b'z' as u16, b'Z' as u16),
            (2, 4, b'q' as u16, b'Q' as u16),
            (3, 51, 0x00f1, 0x00d1),
            (4, 51, 0x00f2, 0x00c7),
            (5, 45, b'/' as u16, b'?' as u16),
            (6, 48, b'[' as u16, b'{' as u16),
        ];
        for (layout, key, plain, shifted) in expected {
            KEYBOARD_LAYOUT.store(layout, Ordering::Release);
            assert_eq!(translate_symbol(key, 0), plain);
            assert_eq!(translate_symbol(key, MOD_SHIFT_L), shifted);
        }

        for (layout, key, plain, shifted) in [
            (1, 49, b'#' as u16, b'\'' as u16),
            (2, 56, b'!' as u16, 0x00a7),
            (3, 46, 0x00a1, 0x00bf),
            (4, 51, 0x00f2, 0x00c7),
            (5, 55, b'.' as u16, b':' as u16),
            (6, 51, b';' as u16, b'+' as u16),
        ] {
            KEYBOARD_LAYOUT.store(layout, Ordering::Release);
            assert_eq!(translate_symbol(key, 0), plain);
            assert_eq!(translate_symbol(key, MOD_SHIFT_L), shifted);
        }
    }

    #[test]
    fn every_selected_layout_maps_the_full_number_row() {
        let _guard = test_guard();
        let rows: [[(u16, u16); 10]; 7] = [
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'@' as u16),
                (b'3' as u16, b'#' as u16),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'^' as u16),
                (b'7' as u16, b'&' as u16),
                (b'8' as u16, b'*' as u16),
                (b'9' as u16, b'(' as u16),
                (b'0' as u16, b')' as u16),
            ],
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'"' as u16),
                (b'3' as u16, 0x00a7),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'&' as u16),
                (b'7' as u16, b'/' as u16),
                (b'8' as u16, b'(' as u16),
                (b'9' as u16, b')' as u16),
                (b'0' as u16, b'=' as u16),
            ],
            [
                (b'&' as u16, b'1' as u16),
                (0x00e9, b'2' as u16),
                (b'"' as u16, b'3' as u16),
                (b'\'' as u16, b'4' as u16),
                (b'(' as u16, b'5' as u16),
                (b'-' as u16, b'6' as u16),
                (0x00e8, b'7' as u16),
                (b'_' as u16, b'8' as u16),
                (0x00e7, b'9' as u16),
                (0x00e0, b'0' as u16),
            ],
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'"' as u16),
                (b'3' as u16, 0x00b7),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'&' as u16),
                (b'7' as u16, b'/' as u16),
                (b'8' as u16, b'(' as u16),
                (b'9' as u16, b')' as u16),
                (b'0' as u16, b'=' as u16),
            ],
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'"' as u16),
                (b'3' as u16, 0x00a3),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'&' as u16),
                (b'7' as u16, b'/' as u16),
                (b'8' as u16, b'(' as u16),
                (b'9' as u16, b')' as u16),
                (b'0' as u16, b'=' as u16),
            ],
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'"' as u16),
                (b'3' as u16, b'#' as u16),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'&' as u16),
                (b'7' as u16, b'_' as u16),
                (b'8' as u16, b'(' as u16),
                (b'9' as u16, b')' as u16),
                (b'0' as u16, b'\'' as u16),
            ],
            [
                (b'1' as u16, b'!' as u16),
                (b'2' as u16, b'"' as u16),
                (b'3' as u16, b'#' as u16),
                (b'4' as u16, b'$' as u16),
                (b'5' as u16, b'%' as u16),
                (b'6' as u16, b'&' as u16),
                (b'7' as u16, b'\'' as u16),
                (b'8' as u16, b'(' as u16),
                (b'9' as u16, b')' as u16),
                (b'0' as u16, b'0' as u16),
            ],
        ];
        for (layout, row) in rows.iter().enumerate() {
            KEYBOARD_LAYOUT.store(layout as u8, Ordering::Release);
            for (offset, &(plain, shifted)) in row.iter().enumerate() {
                let key = 30 + offset as u8;
                assert_eq!(
                    translate_symbol(key, 0),
                    plain,
                    "layout {layout}, key {key}"
                );
                assert_eq!(
                    translate_symbol(key, MOD_SHIFT_L),
                    shifted,
                    "shifted layout {layout}, key {key}"
                );
            }
        }
    }

    #[test]
    fn exports_exist() {
        let _guard = test_guard();
        let _ = init as unsafe fn(KeyPressCallback) -> i32;
        let _ = deinit as fn() -> i32;
        let _ = get_event as unsafe fn(*mut KeyboardEvent) -> i32;
        let _ = flush_events as fn() -> i32;
        let _ = initialize as fn() -> i32;
        let _ = deinitialize as fn() -> i32;
        let _ = open as unsafe fn(EventCallback) -> i32;
        let _ = close as fn();
        let _ = try_close as fn() -> i32;
        let _ = last_status as fn() -> i32;
        let _ = is_connected as fn() -> bool;
        let _ = scan as fn() -> i32;
        let _ = set_led as fn(i32, bool) -> i32;
        let _ = toggle_led as fn(i32) -> i32;
    }

    #[test]
    fn init_deinit_and_flush_lifecycle_are_host_compat() {
        let _guard = test_guard();
        unsafe {
            assert_eq!(init(None), 0);
        }
        let event = KeyboardEvent {
            type_: KEYBOARD_CONNECTED,
            modifiers: 0,
            keycode: 0,
            symbol: 0,
        };
        assert!(queue_push(event));
        assert_eq!(flush_events(), 1);
        assert_eq!(deinit(), 1);
        assert!(!is_connected());
        assert_eq!(flush_events(), 0);
    }

    #[test]
    fn keyboard_deinit_keeps_shared_usb_service_initialized() {
        let _guard = test_guard();
        assert_eq!(initialize(), 0);
        unsafe {
            assert_eq!(init(None), 0);
        }

        assert_eq!(deinit(), 1);
        assert!(USB_INITIALIZED.load(Ordering::Acquire));

        // The keyboard client has closed, but the shared USB service remains
        // available to other USB clients.
        assert_eq!(initialize(), 0);
        assert!(USB_INITIALIZED.load(Ordering::Acquire));
    }

    #[test]
    fn failed_keyboard_close_clears_state_and_preserves_handle_for_retry() {
        let _guard = test_guard();
        let mut kbd = UsbKeyboardDevice {
            connected: true,
            fd: 17,
            cb: Some(event_cb),
            leds: 0b111,
            ep: 0x81,
            ..UsbKeyboardDevice::new()
        };
        CALLBACK_PRESENT.store(true, Ordering::Release);
        USB_CONNECTED.store(true, Ordering::Release);
        LED_BITS.store(0b111, Ordering::Release);
        unsafe {
            USB_CALLBACK = Some(event_cb);
        }

        let result = close_powerpc_keyboard_with(&mut kbd, |_, _| usb::USB_OK, |_| -77);

        assert_eq!(result, -77);
        assert_eq!(kbd.fd, 17);
        assert!(!kbd.connected);
        assert!(kbd.cb.is_none());
        assert_eq!(kbd.leds, 0);
        assert!(!CALLBACK_PRESENT.load(Ordering::Acquire));
        assert!(!USB_CONNECTED.load(Ordering::Acquire));
        assert_eq!(LED_BITS.load(Ordering::Acquire), 0);
        let usb_callback = unsafe { USB_CALLBACK };
        assert!(usb_callback.is_none());
    }

    #[test]
    fn failed_poll_thread_creation_keeps_previous_stdin_device() {
        let previous_stdin = core::cell::Cell::new(0x1234usize);
        let result = start_worker_before_stdin_install(|| -6, || previous_stdin.set(0x5678usize));

        assert_eq!(result, -6);
        assert_eq!(previous_stdin.get(), 0x1234);
    }

    #[test]
    fn open_close_and_is_connected_follow_host_device_presence() {
        let _guard = test_guard();
        assert_eq!(initialize(), 0);
        assert_eq!(unsafe { open(None) }, -3);
        host_set_device_present(true);
        assert_eq!(unsafe { open(Some(event_cb)) }, 1);
        assert!(is_connected());
        close();
        assert!(!is_connected());
        assert_eq!(
            CALLBACK_LAST_TYPE.load(Ordering::Acquire),
            USBKEYBOARD_DISCONNECTED
        );
    }

    #[test]
    fn scan_with_no_device_and_host_device_behaves_as_expected() {
        let _guard = test_guard();
        unsafe {
            assert_eq!(init(None), 0);
        }
        assert_eq!(scan(), -1);
        host_set_device_present(true);
        assert_eq!(unsafe { open(Some(event_cb)) }, 1);

        let mut event = KeyboardEvent::default();
        unsafe {
            assert_eq!(get_event(&mut event), 1);
        }
        assert_eq!(event.type_, KEYBOARD_CONNECTED);

        host_set_scan_report(0x02, [4, 0, 0, 0, 0, 0]);
        assert_eq!(scan(), 0);
        assert_eq!(CALLBACK_EVENTS.load(Ordering::Acquire), 2);
        assert_eq!(
            CALLBACK_LAST_TYPE.load(Ordering::Acquire),
            USBKEYBOARD_PRESSED
        );
        assert_eq!(CALLBACK_LAST_KEY.load(Ordering::Acquire), b'A');
    }

    #[test]
    fn event_dequeue_and_flush_behavior_is_host_compat() {
        let _guard = test_guard();
        unsafe {
            assert_eq!(init(None), 0);
        }
        host_set_device_present(true);
        assert_eq!(unsafe { open(None) }, 1);
        host_set_scan_report(0, [30, 0, 0, 0, 0, 0]);
        assert_eq!(scan(), 0);
        host_set_scan_report(0, [0, 0, 0, 0, 0, 0]);
        assert_eq!(scan(), 0);

        let mut first = KeyboardEvent::default();
        let mut second = KeyboardEvent::default();
        let mut third = KeyboardEvent::default();
        unsafe {
            assert_eq!(get_event(&mut first), 1);
            assert_eq!(get_event(&mut second), 1);
            assert_eq!(get_event(&mut third), 1);
        }
        assert_eq!(first.type_, KEYBOARD_CONNECTED);
        assert_eq!(second.type_, KEYBOARD_PRESSED);
        assert_eq!(second.keycode, 30);
        assert_eq!(second.symbol, b'1' as u16);
        assert_eq!(third.type_, KEYBOARD_RELEASED);
        assert_eq!(third.keycode, 30);
        assert_eq!(third.symbol, b'1' as u16);
        assert_eq!(flush_events(), 0);
    }

    #[test]
    fn led_set_and_toggle_behavior_is_host_compat() {
        let _guard = test_guard();
        assert_eq!(initialize(), 0);
        host_set_device_present(true);
        assert_eq!(unsafe { open(None) }, 1);
        assert_eq!(set_led(USBKEYBOARD_LEDNUM, true), 1);
        assert_eq!(host_led_bits(), 1);
        assert_eq!(set_led(USBKEYBOARD_LEDCAPS, true), 1);
        assert_eq!(host_led_bits(), 0b11);
        assert_eq!(toggle_led(USBKEYBOARD_LEDNUM), 1);
        assert_eq!(host_led_bits(), 0b10);
        assert_eq!(toggle_led(USBKEYBOARD_LEDSCROLL), 1);
        assert_eq!(host_led_bits(), 0b110);
    }

    #[test]
    fn invalid_inputs_keep_c_style_failure_codes() {
        let _guard = test_guard();
        let mut event = KeyboardEvent::default();
        assert_eq!(unsafe { get_event(core::ptr::null_mut()) }, 0);
        assert_eq!(unsafe { get_event(&mut event) }, 0);
        assert_eq!(set_led(USBKEYBOARD_LEDNUM, true), -1);
        assert_eq!(toggle_led(USBKEYBOARD_LEDCAPS), -1);
        assert_eq!(unsafe { open(None) }, -1);
        assert_eq!(scan(), -1);
        assert_eq!(unsafe { init(None) }, 0);
        host_set_device_present(true);
        assert_eq!(unsafe { open(None) }, 1);
        assert_eq!(set_led(99, true), -1);
        host_set_scan_report(0, [KEY_ERROR, 0, 0, 0, 0, 0]);
        assert_eq!(scan(), 0);
    }
}
