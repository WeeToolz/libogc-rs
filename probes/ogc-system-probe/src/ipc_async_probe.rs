#[cfg(target_arch = "powerpc")]
use core::ffi::{CStr, c_void};
use core::fmt::Write;
#[cfg(target_arch = "powerpc")]
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_ipc::{IPC_OPEN_NONE, open_file_async, poll as ipc_poll, set_debug_hook};
#[cfg(target_arch = "powerpc")]
use ogc_system as system;
use ogc_video as video;

use crate::probe_support::ProbeLogger;

const IPC_ASYNC_PENDING: i32 = -20000;
const IPC_ASYNC_UNSET: i32 = -9999;
#[cfg(target_arch = "powerpc")]
const WII_TIMER_CLOCK_HZ: u64 = 60_750_000;
const AUTO_WINDOW_USEC: u64 = 1_000_000;
#[cfg(feature = "dolphin-automation")]
const AUTOMATION_FRAMES: u32 = 600;

#[cfg(target_arch = "powerpc")]
static RESULT_FS: AtomicI32 = AtomicI32::new(IPC_ASYNC_UNSET);
#[cfg(target_arch = "powerpc")]
static COUNT_FS: AtomicU32 = AtomicU32::new(0);
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

pub fn monitor(logger: &mut ProbeLogger) -> ! {
    logger.section("IPC ASYNC FS PROBE START");
    logger.note("no USB calls in this diagnostic");
    logger.note("manual IPC poll delayed until auto window ends");

    let mut state = IpcAsyncProbeState::new();

    #[cfg(target_arch = "powerpc")]
    {
        state.start_ticks = system::time();
        reset_probe();
        set_debug_hook(Some(ipc_debug_hook));
    }

    submit_fs_open(&mut state);

    loop {
        refresh_state(&mut state);
        service_ipc_after_auto_window(&mut state);
        refresh_state(&mut state);

        #[cfg(feature = "dolphin-automation")]
        if state.elapsed_frames >= AUTOMATION_FRAMES {
            logger.section("IPC ASYNC AUTOMATION RESULT");
            logger.note_i32("submit rc", state.submit_rc);
            logger.note_i32("callback result", state.callback_result);
            logger.note_u32("callback count", state.callback_count);
            logger.note_u32("manual poll calls", state.manual_poll_calls);
            logger.note_u32("manual poll hits", state.manual_poll_hits);
            logger.note_u32("IPC debug events", state.debug_count);
            logger.record("async open submission accepted", state.submit_rc >= 0);
            logger.record("async callback completed once", state.callback_count == 1);
            logger.record(
                "async open returned a descriptor",
                state.callback_result >= 0,
            );
            logger.record(
                "automatic IRQ or manual poll delivered completion",
                state.callback_count > 0
                    && (!state.manual_poll_started || state.manual_poll_calls > 0),
            );
            logger.summary();
            loop {
                logger.display().wait_vsync();
            }
        }

        draw_dashboard(&state);
        logger.display().wait_vsync();
    }
}

struct IpcAsyncProbeState {
    phase: &'static str,
    submit_rc: i32,
    callback_result: i32,
    callback_count: u32,
    callback_count_before_poll: u32,
    manual_poll_started: bool,
    manual_poll_calls: u32,
    manual_poll_hits: u32,
    elapsed_frames: u32,
    elapsed_usec: u64,
    start_ticks: u64,
    debug_stage: u32,
    debug_a: u32,
    debug_b: u32,
    debug_c: u32,
    debug_count: u32,
}

impl IpcAsyncProbeState {
    const fn new() -> Self {
        Self {
            phase: "init",
            submit_rc: IPC_ASYNC_UNSET,
            callback_result: IPC_ASYNC_UNSET,
            callback_count: 0,
            callback_count_before_poll: 0,
            manual_poll_started: false,
            manual_poll_calls: 0,
            manual_poll_hits: 0,
            elapsed_frames: 0,
            elapsed_usec: 0,
            start_ticks: 0,
            debug_stage: 0,
            debug_a: 0,
            debug_b: 0,
            debug_c: 0,
            debug_count: 0,
        }
    }
}

fn submit_fs_open(state: &mut IpcAsyncProbeState) {
    state.phase = "submitted /dev/fs async open";
    state.submit_rc = IPC_ASYNC_PENDING;
    state.callback_result = IPC_ASYNC_PENDING;
    state.callback_count = 0;

    #[cfg(target_arch = "powerpc")]
    {
        RESULT_FS.store(IPC_ASYNC_PENDING, Ordering::Release);
        COUNT_FS.store(0, Ordering::Release);
        let rc = open_file_async(
            fs_path(),
            IPC_OPEN_NONE,
            Some(ipc_async_open_cb),
            core::ptr::null_mut(),
        );
        state.submit_rc = rc;
        if rc < 0 {
            state.phase = "submit failed";
            state.callback_result = IPC_ASYNC_UNSET;
            RESULT_FS.store(IPC_ASYNC_UNSET, Ordering::Release);
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        state.submit_rc = 0;
    }
}

fn service_ipc_after_auto_window(state: &mut IpcAsyncProbeState) {
    if state.submit_rc < 0 || state.callback_count > 0 {
        return;
    }

    if !state.manual_poll_started {
        if state.elapsed_usec < AUTO_WINDOW_USEC {
            state.phase = "auto IRQ window; not polling";
            return;
        }

        state.manual_poll_started = true;
        state.callback_count_before_poll = state.callback_count;
        state.phase = "manual poll started";
    }

    state.manual_poll_calls = state.manual_poll_calls.wrapping_add(1);

    #[cfg(target_arch = "powerpc")]
    {
        if ipc_poll() {
            state.manual_poll_hits = state.manual_poll_hits.wrapping_add(1);
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn reset_probe() {
    RESULT_FS.store(IPC_ASYNC_UNSET, Ordering::Release);
    COUNT_FS.store(0, Ordering::Release);
    DEBUG_STAGE.store(0, Ordering::Release);
    DEBUG_A.store(0, Ordering::Release);
    DEBUG_B.store(0, Ordering::Release);
    DEBUG_C.store(0, Ordering::Release);
    DEBUG_COUNT.store(0, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
fn fs_path() -> &'static CStr {
    c"/dev/fs"
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn ipc_async_open_cb(result: i32, _userdata: *mut c_void) -> i32 {
    RESULT_FS.store(result, Ordering::Release);
    let _ = COUNT_FS.fetch_add(1, Ordering::AcqRel);
    result
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
fn refresh_state(state: &mut IpcAsyncProbeState) {
    state.callback_result = RESULT_FS.load(Ordering::Acquire);
    state.callback_count = COUNT_FS.load(Ordering::Acquire);
    state.debug_stage = DEBUG_STAGE.load(Ordering::Acquire);
    state.debug_a = DEBUG_A.load(Ordering::Acquire);
    state.debug_b = DEBUG_B.load(Ordering::Acquire);
    state.debug_c = DEBUG_C.load(Ordering::Acquire);
    state.debug_count = DEBUG_COUNT.load(Ordering::Acquire);
    state.elapsed_frames = state.elapsed_frames.wrapping_add(1);

    let now = system::time();
    let elapsed_ticks = now.wrapping_sub(state.start_ticks);
    state.elapsed_usec = elapsed_ticks.saturating_mul(1_000_000) / WII_TIMER_CLOCK_HZ;
}

#[cfg(not(target_arch = "powerpc"))]
fn refresh_state(state: &mut IpcAsyncProbeState) {
    state.elapsed_frames = state.elapsed_frames.wrapping_add(1);
}

fn draw_dashboard(state: &IpcAsyncProbeState) {
    video::console_clear();

    draw_at(0, 0, "IPC ASYNC /dev/fs PROBE");
    draw_at(0, 1, "No USB backend calls");
    draw_at(0, 2, "Manual poll starts after 1000 ms");

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "phase={}", state.phase);
    draw_at(0, 4, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "request=/dev/fs async open");
    draw_at(0, 6, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "submit rc=");
    write_rc(&mut line, state.submit_rc);
    draw_at(0, 7, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "callback result=");
    write_rc(&mut line, state.callback_result);
    draw_at(0, 8, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "callback count={}", state.callback_count);
    draw_at(0, 9, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "manual poll active={} calls/hits={}/{}",
        state.manual_poll_started, state.manual_poll_calls, state.manual_poll_hits
    );
    draw_at(0, 11, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "callback count before poll={}",
        state.callback_count_before_poll
    );
    draw_at(0, 12, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "elapsed frames/usec={}/{}",
        state.elapsed_frames, state.elapsed_usec
    );
    draw_at(0, 14, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug stage/count={}/{}",
        state.debug_stage, state.debug_count
    );
    draw_at(0, 16, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "debug a/b/c=0x{:08x}/0x{:08x}/0x{:08x}",
        state.debug_a, state.debug_b, state.debug_c
    );
    draw_at(0, 17, line.as_str());

    draw_interpretation(state);
}

fn write_rc(line: &mut LineBuf, rc: i32) {
    if rc == IPC_ASYNC_PENDING {
        let _ = write!(line, "pending");
    } else {
        let _ = write!(line, "{}", rc);
    }
}

fn draw_interpretation(state: &IpcAsyncProbeState) {
    if state.callback_count > 0 && !state.manual_poll_started {
        draw_at(0, 20, "Result: callback fired before manual poll.");
    } else if state.callback_count > 0 {
        draw_at(0, 20, "Result: callback fired after manual poll.");
    } else if state.manual_poll_hits > 0 {
        draw_at(
            0,
            20,
            "Manual poll serviced IPC status; waiting for callback.",
        );
    } else if state.manual_poll_started {
        draw_at(0, 20, "Manual poll active; callback still pending.");
    } else {
        draw_at(0, 20, "Waiting for automatic IRQ delivery.");
    }
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
