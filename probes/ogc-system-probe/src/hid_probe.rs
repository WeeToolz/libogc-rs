#[cfg(target_arch = "powerpc")]
use core::ffi::{CStr, c_void};
use core::fmt::Write;
#[cfg(target_arch = "powerpc")]
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_ios as ios;
#[cfg(target_arch = "powerpc")]
use ogc_ipc::{IPC_OPEN_NONE, open_file_async, poll as ipc_poll, set_debug_hook};

#[cfg(target_arch = "powerpc")]
use ogc_usb as usb;
use ogc_video as video;

use crate::probe_support::ProbeLogger;

#[cfg(feature = "dolphin-automation")]
const AUTOMATION_FRAMES: u32 = 600;

pub fn monitor(logger: &mut ProbeLogger) -> ! {
    logger.section("HID PROBE START");
    logger.note("stationary backend dashboard follows");
    logger.note("USB init deferred so IPC polling can run");

    let mut state = HidProbeState::new();
    #[cfg(target_arch = "powerpc")]
    {
        reset_debug_probe();
        set_debug_hook(Some(ipc_debug_hook));
        state.ios_version = ios::get_version();
        state.ios_revision = ios::get_revision();
        state.ios_revision_major = ios::get_revision_major();
        state.ios_revision_minor = ios::get_revision_minor();
    }
    state.backend = query_usb_backend_state();

    #[cfg(feature = "dolphin-automation")]
    let mut automation_frames = 0u32;

    loop {
        refresh_hid_state(&mut state);

        #[cfg(feature = "dolphin-automation")]
        {
            automation_frames = automation_frames.saturating_add(1);
            if automation_frames >= AUTOMATION_FRAMES {
                logger.section("HID AUTOMATION RESULT");
                logger.note_i32("IOS version", state.ios_version);
                logger.note_i32("ven open rc", state.backend.ven_open_rc);
                logger.note_i32("hid open rc", state.backend.hid_open_rc);
                logger.note_u32("IPC poll calls", state.ipc_poll_calls);
                logger.note_u32("IPC poll hits", state.ipc_poll_hits);
                logger.note_u32("IPC debug events", state.debug_count);
                logger.record("IOS metadata available", state.ios_version >= 0);
                logger.record(
                    "USB backend opens reached terminal state",
                    backend_terminal(&state.backend),
                );
                logger.record("IPC debug events observed", state.debug_count > 0);
                logger.summary();
                loop {
                    logger.display().wait_vsync();
                }
            }
        }

        draw_dashboard(&state);
        logger.display().wait_vsync();
    }
}

struct HidProbeState {
    usb_init_rc: i32,
    hid_list_rc: i32,
    hid_count: usize,
    hid_entries: [usb::UsbDeviceEntry; 8],
    all_list_rc: i32,
    all_count: usize,
    all_entries: [usb::UsbDeviceEntry; 8],
    backend: UsbBackendState,
    ipc_poll_calls: u32,
    ipc_poll_hits: u32,
    ios_version: i32,
    ios_revision: i32,
    ios_revision_major: i32,
    ios_revision_minor: i32,
    debug_stage: u32,
    debug_a: u32,
    debug_b: u32,
    debug_c: u32,
    debug_count: u32,
}

impl HidProbeState {
    fn new() -> Self {
        Self {
            usb_init_rc: USB_INIT_DEFERRED,
            hid_list_rc: USB_INIT_DEFERRED,
            hid_count: 0,
            hid_entries: [usb::UsbDeviceEntry::default(); 8],
            all_list_rc: USB_INIT_DEFERRED,
            all_count: 0,
            all_entries: [usb::UsbDeviceEntry::default(); 8],
            backend: UsbBackendState::new(),
            ipc_poll_calls: 0,
            ipc_poll_hits: 0,
            ios_version: -9999,
            ios_revision: -9999,
            ios_revision_major: -9999,
            ios_revision_minor: -9999,
            debug_stage: 0,
            debug_a: 0,
            debug_b: 0,
            debug_c: 0,
            debug_count: 0,
        }
    }
}

fn refresh_hid_state(state: &mut HidProbeState) {
    refresh_backend_state(state);

    state.hid_count = 0;
    state.all_count = 0;

    if state.usb_init_rc != usb::USB_OK {
        state.hid_list_rc = USB_INIT_DEFERRED;
        state.all_list_rc = USB_INIT_DEFERRED;
        return;
    }

    match usb::device_list(&mut state.hid_entries, usb::USB_CLASS_HID) {
        Ok(count) => {
            state.hid_list_rc = 0;
            state.hid_count = count.min(state.hid_entries.len());
        }
        Err(err) => {
            state.hid_list_rc = err.raw();
        }
    }

    match usb::device_list(&mut state.all_entries, 0) {
        Ok(count) => {
            state.all_list_rc = 0;
            state.all_count = count.min(state.all_entries.len());
        }
        Err(err) => {
            state.all_list_rc = err.raw();
        }
    }
}

#[derive(Copy, Clone)]
struct UsbBackendState {
    oh0_open_rc: i32,
    ven_open_rc: i32,
    ven_version_rc: i32,
    ven_version: u32,
    hid_open_rc: i32,
    hid_v4_version_rc: i32,
    hid_v5_version_rc: i32,
    hid_version: u32,
}

impl UsbBackendState {
    const fn new() -> Self {
        Self {
            oh0_open_rc: -9999,
            ven_open_rc: -9999,
            ven_version_rc: -9999,
            ven_version: 0,
            hid_open_rc: -9999,
            hid_v4_version_rc: -9999,
            hid_v5_version_rc: -9999,
            hid_version: 0,
        }
    }
}

const USB_INIT_DEFERRED: i32 = -30000;

#[cfg(target_arch = "powerpc")]
const USB_BACKEND_PENDING: i32 = -20000;
#[cfg(target_arch = "powerpc")]
const USB_BACKEND_SKIPPED: i32 = -10000;

#[cfg(target_arch = "powerpc")]
static VEN_OPEN_RC: AtomicI32 = AtomicI32::new(-9999);
#[cfg(target_arch = "powerpc")]
static HID_OPEN_RC: AtomicI32 = AtomicI32::new(-9999);
#[cfg(target_arch = "powerpc")]
static DEBUG_STAGE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_A: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_B: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_C: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "powerpc")]
fn reset_debug_probe() {
    DEBUG_STAGE.store(0, Ordering::Release);
    DEBUG_A.store(0, Ordering::Release);
    DEBUG_B.store(0, Ordering::Release);
    DEBUG_C.store(0, Ordering::Release);
    DEBUG_COUNT.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
unsafe fn ipc_debug_hook(stage: u32, a: u32, b: u32, c: u32) {
    DEBUG_STAGE.store(stage, Ordering::Release);
    DEBUG_A.store(a, Ordering::Release);
    DEBUG_B.store(b, Ordering::Release);
    DEBUG_C.store(c, Ordering::Release);
    let _ = DEBUG_COUNT.fetch_add(1, Ordering::AcqRel);
}

#[cfg(target_arch = "powerpc")]
fn c_path(bytes: &'static [u8]) -> &'static CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn backend_open_cb(result: i32, userdata: *mut c_void) -> i32 {
    match userdata as usize {
        1 => VEN_OPEN_RC.store(result, Ordering::Release),
        2 => HID_OPEN_RC.store(result, Ordering::Release),
        _ => {}
    }
    result
}

#[cfg(target_arch = "powerpc")]
fn refresh_backend_state(state: &mut HidProbeState) {
    state.ipc_poll_calls = state.ipc_poll_calls.wrapping_add(1);
    if ipc_poll() {
        state.ipc_poll_hits = state.ipc_poll_hits.wrapping_add(1);
    }

    state.backend.ven_open_rc = VEN_OPEN_RC.load(Ordering::Acquire);
    state.backend.hid_open_rc = HID_OPEN_RC.load(Ordering::Acquire);
    state.debug_stage = DEBUG_STAGE.load(Ordering::Acquire);
    state.debug_a = DEBUG_A.load(Ordering::Acquire);
    state.debug_b = DEBUG_B.load(Ordering::Acquire);
    state.debug_c = DEBUG_C.load(Ordering::Acquire);
    state.debug_count = DEBUG_COUNT.load(Ordering::Acquire);
}

#[cfg(not(target_arch = "powerpc"))]
fn refresh_backend_state(_state: &mut HidProbeState) {}

#[cfg(target_arch = "powerpc")]
fn query_usb_backend_state() -> UsbBackendState {
    let mut state = UsbBackendState::new();

    state.oh0_open_rc = USB_BACKEND_SKIPPED;

    VEN_OPEN_RC.store(USB_BACKEND_PENDING, Ordering::Release);
    HID_OPEN_RC.store(USB_BACKEND_PENDING, Ordering::Release);

    state.ven_open_rc = USB_BACKEND_PENDING;
    let rc = open_file_async(
        c_path(b"/dev/usb/ven\0"),
        IPC_OPEN_NONE,
        Some(backend_open_cb),
        core::ptr::dangling_mut::<c_void>(),
    );
    if rc < 0 {
        VEN_OPEN_RC.store(rc, Ordering::Release);
        state.ven_open_rc = rc;
    }

    state.hid_open_rc = USB_BACKEND_PENDING;
    let rc = open_file_async(
        c_path(b"/dev/usb/hid\0"),
        IPC_OPEN_NONE,
        Some(backend_open_cb),
        2usize as *mut c_void,
    );
    if rc < 0 {
        HID_OPEN_RC.store(rc, Ordering::Release);
        state.hid_open_rc = rc;
    }

    state
}

#[cfg(not(target_arch = "powerpc"))]
fn query_usb_backend_state() -> UsbBackendState {
    UsbBackendState::new()
}

fn draw_dashboard(state: &HidProbeState) {
    video::console_clear();

    draw_at(0, 0, "USB HID BACKEND PROBE");
    draw_at(0, 1, "USB init is deferred");
    draw_at(0, 2, "Manual IPC poll is active");

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "usb init rc={}", state.usb_init_rc);
    draw_at(0, 4, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "ipc poll calls/hits={}/{}",
        state.ipc_poll_calls, state.ipc_poll_hits
    );
    draw_at(0, 5, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "ios version/revision={}/{}",
        state.ios_version, state.ios_revision
    );
    draw_at(0, 6, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "ios rev major/minor={}/{}",
        state.ios_revision_major, state.ios_revision_minor
    );
    draw_at(0, 7, line.as_str());

    draw_backend_state(&state.backend);

    if backend_terminal(&state.backend) {
        draw_at(0, 18, "Backend async opens reached terminal state.");
        draw_at(0, 19, "USB init still deferred in this diagnostic.");
    } else {
        draw_at(0, 18, "Waiting for backend async open callbacks.");
    }

    draw_at(40, 4, "HID listing disabled");
    draw_at(40, 5, "until backend opens are known");

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug stage/count={}/{}",
        state.debug_stage, state.debug_count
    );
    draw_at(0, 21, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug a/b/c=0x{:08x}/0x{:08x}/0x{:08x}",
        state.debug_a, state.debug_b, state.debug_c
    );
    draw_at(0, 22, line.as_str());
}

fn backend_terminal(backend: &UsbBackendState) -> bool {
    backend.ven_open_rc != USB_BACKEND_PENDING && backend.hid_open_rc != USB_BACKEND_PENDING
}

fn draw_backend_state(backend: &UsbBackendState) {
    let mut line = LineBuf::new();
    if backend.oh0_open_rc == USB_BACKEND_SKIPPED {
        let _ = write!(&mut line, "oh0 open rc=skipped");
    } else {
        let _ = write!(&mut line, "oh0 open rc={}", backend.oh0_open_rc);
    }
    draw_at(0, 10, line.as_str());

    let mut line = LineBuf::new();
    if backend.ven_open_rc == USB_BACKEND_PENDING {
        let _ = write!(
            &mut line,
            "ven open/version rc=pending/{}",
            backend.ven_version_rc
        );
    } else {
        let _ = write!(
            &mut line,
            "ven open/version rc={}/{}",
            backend.ven_open_rc, backend.ven_version_rc
        );
    }
    draw_at(0, 11, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "ven version=0x{:08x}", backend.ven_version);
    draw_at(0, 12, line.as_str());

    let mut line = LineBuf::new();
    if backend.hid_open_rc == USB_BACKEND_PENDING {
        let _ = write!(
            &mut line,
            "hid open/v4/v5 rc=pending/{}/{}",
            backend.hid_v4_version_rc, backend.hid_v5_version_rc
        );
    } else {
        let _ = write!(
            &mut line,
            "hid open/v4/v5 rc={}/{}/{}",
            backend.hid_open_rc, backend.hid_v4_version_rc, backend.hid_v5_version_rc
        );
    }
    draw_at(0, 13, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "hid version=0x{:08x}", backend.hid_version);
    draw_at(0, 14, line.as_str());
}

fn draw_at(col: i32, row: i32, text: &str) {
    let mut seq = LineBuf::new();
    let _ = write!(&mut seq, "\x1b[{};{}H", row + 1, col + 1);
    let _ = crate::probe_support::screen_write(seq.as_bytes());
    let _ = crate::probe_support::screen_write(text.as_bytes());
}

struct LineBuf {
    bytes: [u8; 256],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; 256],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let remaining = self.bytes.len().saturating_sub(self.len);
        if s.len() > remaining {
            return Err(core::fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}
