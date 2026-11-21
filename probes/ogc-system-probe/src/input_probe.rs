use core::cell::UnsafeCell;
use core::fmt::Write;
use core::sync::atomic::{AtomicUsize, Ordering};

use ogc_input as pad;
use ogc_si as si;
use ogc_video as video;
use wpad_sys as wpad;

use crate::controller::{
    Buttons, ControllerKind, ControllerManager, ControllerMetadata, ControllerSnapshot,
    ControllerSource, ControllerState, InputTraceEvent, MAX_CONTROLLERS, MAX_GAMECUBE_CONTROLLERS,
    RawInputFrame, TraceError, UsbTraceIdentity,
};
use crate::event_line::{EventLineBuf, source_slots, source_text};
use crate::probe_support::ProbeLogger;
use ogc_system as system;

#[path = "controller_cleanup.rs"]
pub(crate) mod controller_cleanup;
#[path = "controller_cleanup_state.rs"]
mod controller_cleanup_state;

const TRACE_QUEUE_CAPACITY: usize = 80;
const TRACE_REPORT_CAPACITY: usize = 512;
const MONITOR_FRAME_LIMIT: u32 = 9_000;

#[derive(Clone, Copy)]
struct RawTraceRecord {
    source: ControllerSource,
    generation: Option<u32>,
    usb_identity: Option<UsbTraceIdentity>,
    timestamp_ticks: Option<u64>,
    len: usize,
    bytes: [u8; TRACE_REPORT_CAPACITY],
}

struct TraceQueue {
    head: AtomicUsize,
    tail: AtomicUsize,
    events: UnsafeCell<[Option<RawTraceRecord>; TRACE_QUEUE_CAPACITY]>,
}

// The manager calls the observer synchronously while scanning. The same probe
// thread drains the queue after each scan. No interrupt or second producer uses
// this queue.
unsafe impl Sync for TraceQueue {}

impl TraceQueue {
    const fn new() -> Self {
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            events: UnsafeCell::new([None; TRACE_QUEUE_CAPACITY]),
        }
    }

    fn push(&self, record: RawTraceRecord) -> Result<(), TraceError> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= TRACE_QUEUE_CAPACITY {
            return Err(TraceError(1));
        }
        // SAFETY: one synchronous producer owns this slot until publishing head.
        unsafe { (*self.events.get())[head % TRACE_QUEUE_CAPACITY] = Some(record) };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<RawTraceRecord> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: the published slot is owned by this consumer until tail moves.
        let event = unsafe { (*self.events.get())[tail % TRACE_QUEUE_CAPACITY].take() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        event
    }
}

static TRACE_QUEUE: TraceQueue = TraceQueue::new();
static BLUETOOTH_STARTUP_HOOK_CALLS: AtomicUsize = AtomicUsize::new(0);
static USB_STARTUP_HOOK_CALLS: AtomicUsize = AtomicUsize::new(0);
static INPUT_TRACE_OBSERVER_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dolphin-automation")]
static PAD_SCAN_HOOKS: AtomicUsize = AtomicUsize::new(0);

fn bluetooth_startup_marker(_stage: &'static str) {
    BLUETOOTH_STARTUP_HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn usb_startup_marker(_stage: &'static str) {
    USB_STARTUP_HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn queue_trace_event(event: &InputTraceEvent<'_>) -> Result<(), TraceError> {
    INPUT_TRACE_OBSERVER_CALLS.fetch_add(1, Ordering::Relaxed);
    let InputTraceEvent::RawReport {
        source,
        generation,
        usb_identity,
        timestamp_ticks,
        bytes,
    } = event
    else {
        // Decoded states, resets, and errors are logged from the same snapshot.
        return Ok(());
    };
    if bytes.len() > TRACE_REPORT_CAPACITY {
        return Err(TraceError(2));
    }
    let mut copy = [0; TRACE_REPORT_CAPACITY];
    copy[..bytes.len()].copy_from_slice(bytes);
    TRACE_QUEUE.push(RawTraceRecord {
        source: *source,
        generation: *generation,
        usb_identity: *usb_identity,
        timestamp_ticks: *timestamp_ticks,
        len: bytes.len(),
        bytes: copy,
    })
}

fn emit_event_record(logger: &mut ProbeLogger, line: &EventLineBuf) -> bool {
    let Ok(record) = line.as_str() else {
        logger.record("input event record formatted completely", false);
        return false;
    };
    #[cfg(feature = "agent-report")]
    let capture_ok = if !logger.display().console_ready() {
        let Some(length) = record.len().checked_add(1).filter(|length| *length <= 1301) else {
            logger.record("input event record captured completely", false);
            return false;
        };
        let mut capture_line = [0; 1301];
        capture_line[..record.len()].copy_from_slice(record.as_bytes());
        capture_line[record.len()] = b'\n';
        crate::probe_support::agent_log_checked(&capture_line[..length]).is_ok()
    } else {
        // `ProbeLogger` writes the record through `screen_write`, which also
        // captures it. Avoid appending the same event to the report twice.
        true
    };
    #[cfg(not(feature = "agent-report"))]
    let capture_ok = true;
    let log_ok = logger.write_event_record(record);
    if !capture_ok || !log_ok {
        logger.record(
            "input event record was captured and written completely",
            false,
        );
    }
    capture_ok && log_ok
}

#[cfg(feature = "dolphin-automation")]
const AUTOMATION_FRAMES: u32 = 600;

const BUTTON_LABELS: &[(Buttons, &str)] = &[
    (Buttons::A, "A"),
    (Buttons::B, "B"),
    (Buttons::X, "X"),
    (Buttons::Y, "Y"),
    (Buttons::UP, "UP"),
    (Buttons::DOWN, "DOWN"),
    (Buttons::LEFT, "LEFT"),
    (Buttons::RIGHT, "RIGHT"),
    (Buttons::START, "START"),
    (Buttons::SELECT, "SELECT"),
    (Buttons::HOME, "HOME"),
    (Buttons::L, "L"),
    (Buttons::R, "R"),
    (Buttons::ZL, "ZL"),
    (Buttons::ZR, "ZR"),
    (Buttons::SL, "SL"),
    (Buttons::SR, "SR"),
    (Buttons::CAPTURE, "CAP"),
];

#[derive(Clone, Copy)]
struct InputProbeState {
    pad_init_ok: bool,
    pad_valid_mask: u32,
    pad_scan_mask: u32,
    gc_type: [u32; MAX_GAMECUBE_CONTROLLERS],
    gc_err: [i32; MAX_GAMECUBE_CONTROLLERS],
    gc_buttons: [u32; MAX_GAMECUBE_CONTROLLERS],
    gc_response: [[u32; 2]; MAX_GAMECUBE_CONTROLLERS],
    gc_command: [u32; MAX_GAMECUBE_CONTROLLERS],
    active_controllers: u32,
    last_states: [Option<ControllerState>; MAX_CONTROLLERS],
    last_metadata: [Option<ControllerMetadata>; MAX_CONTROLLERS],
}

impl InputProbeState {
    const fn new() -> Self {
        Self {
            pad_init_ok: false,
            pad_valid_mask: 0,
            pad_scan_mask: 0,
            gc_type: [0; MAX_GAMECUBE_CONTROLLERS],
            gc_err: [0; MAX_GAMECUBE_CONTROLLERS],
            gc_buttons: [0; MAX_GAMECUBE_CONTROLLERS],
            gc_response: [[0; 2]; MAX_GAMECUBE_CONTROLLERS],
            gc_command: [0; MAX_GAMECUBE_CONTROLLERS],
            active_controllers: 0,
            last_states: [None; MAX_CONTROLLERS],
            last_metadata: [None; MAX_CONTROLLERS],
        }
    }
}

pub fn monitor(logger: &mut ProbeLogger) -> ! {
    logger.section("INPUT PROBE START");
    logger.note("fixed-window controller collection follows");

    let mut controllers = ControllerManager::new();
    let mut state = InputProbeState::new();

    controllers.init();

    BLUETOOTH_STARTUP_HOOK_CALLS.store(0, Ordering::Relaxed);
    USB_STARTUP_HOOK_CALLS.store(0, Ordering::Relaxed);
    INPUT_TRACE_OBSERVER_CALLS.store(0, Ordering::Relaxed);
    controllers.set_bluetooth_startup_hook(Some(bluetooth_startup_marker));
    controllers.set_usb_startup_hook(Some(usb_startup_marker));
    controllers.set_input_trace_observer(Some(queue_trace_event));
    let bluetooth_hid_rc = controllers.enable_bluetooth_hid();
    let wpad_ready = controllers.enable_wpad() == 0;
    controllers.enable_usb_hid();
    logger.record(
        "controller Bluetooth HID startup request accepted",
        bluetooth_hid_rc == wpad::WPAD_ERR_NONE as i32,
    );
    logger.record("WPAD initialized", wpad_ready);
    let mut capture_complete = true;
    let run_start_ticks = system::time();
    let mut run_record = EventLineBuf::new();
    let _ = write!(&mut run_record, "{{\"type\":\"run\",\"run_id\":");
    if let Some((run_id, dol_sha256)) = crate::probe_support::input_run_identity() {
        let _ = run_record.write_json_string(run_id);
        let _ = write!(
            &mut run_record,
            ",\"start_ticks\":{},\"connection\":\"wii_controller_stack\",\"dol_sha256\":",
            run_start_ticks
        );
        let _ = run_record.write_json_string(dol_sha256);
    } else {
        let _ = write!(
            &mut run_record,
            "\"input-{:016x}\",\"start_ticks\":{},\"connection\":\"wii_controller_stack\",\"dol_sha256\":null",
            run_start_ticks, run_start_ticks
        );
    }
    let _ = write!(
        &mut run_record,
        ",\"device_model\":null,\"firmware\":null}}"
    );
    capture_complete &= emit_event_record(logger, &run_record);

    #[cfg(feature = "dolphin-automation")]
    let mut automation = InputAutomationState::default();
    #[cfg(feature = "dolphin-automation")]
    let mut pad_checks = PadAccessChecks::new();
    #[cfg(feature = "dolphin-automation")]
    let mut automation_frames = 0u32;
    #[cfg(feature = "dolphin-automation")]
    let mut automation_finished = false;
    let mut monitor_frames = 0u32;
    let mut saw_device = false;

    loop {
        let scan_ticks = system::time();
        let frame = controllers.scan_with_raw_at(scan_ticks);
        let snapshot = frame.snapshot;
        if monitor_frames == 0 {
            state.pad_init_ok = snapshot.errors().next().is_none();
            logger.record(
                "controller manager init produces a clean first scan",
                state.pad_init_ok,
            );
            logger.record("GameCube PAD initialized", state.pad_init_ok);
        }
        if controllers.joycon_pair().is_none() {
            controllers.pair_joycons();
        }
        refresh_probe_state(&mut state, &frame.pad_status);
        let trace_complete = write_trace_events(logger);
        capture_complete &= trace_complete;
        if !trace_complete {
            system::report(format_args!("INPUT PROBE TRACE FAILED: raw event lost\n"));
            logger.record("raw input trace events captured completely", false);
        }
        capture_complete &=
            write_snapshot_events(logger, &mut state, &snapshot, &frame, scan_ticks);
        state.active_controllers = snapshot.len() as u32;
        saw_device |= !snapshot.controllers().is_empty();

        #[cfg(feature = "dolphin-automation")]
        {
            automation.observe(&snapshot);
            probe_pad_accessors(&mut pad_checks, &frame.pad_status[0]);
            automation_frames = automation_frames.saturating_add(1);
            if automation_frames >= AUTOMATION_FRAMES {
                automation_finished = true;
                break;
            }
        }

        draw_dashboard(&state, &snapshot);
        if !video::wait_vsync_timeout_us(100_000) {
            capture_complete = false;
            logger.record("monitor frame retrace completed", false);
            break;
        }
        crate::probe_support::service_power_shutdown();
        monitor_frames = monitor_frames.saturating_add(1);
        if monitor_frames >= MONITOR_FRAME_LIMIT {
            break;
        }
    }

    #[cfg(feature = "dolphin-automation")]
    if automation_finished {
        logger.section("INPUT AUTOMATION RESULT");
        logger.record("pad initialized", state.pad_init_ok);
        logger.record("controller discovered", automation.controller_seen);
        logger.record("A transition observed", automation.a_seen);
        logger.record("Start transition observed", automation.start_seen);
        logger.record("D-pad right transition observed", automation.right_seen);
        logger.record("main stick transition observed", automation.main_stick_seen);
        logger.record(
            "PAD accessors match button transitions from raw samples",
            pad_checks.buttons_ok && pad_checks.saw_down && pad_checks.saw_up,
        );
        logger.record(
            "PAD accessors match axes from raw samples",
            pad_checks.axes_ok && pad_checks.saw_stick,
        );
        probe_controller_scan_modes(logger, &mut controllers);
        probe_pad_low_level_state(logger);
        probe_ogc_input_public_wrappers(logger);
        probe_pad_sampling_callback(logger);
    }

    let cleanup_complete =
        controller_cleanup::shutdown_controller_manager(logger, &mut controllers);
    logger.record("controller manager shutdown completed", cleanup_complete);
    logger.record(
        "Bluetooth startup hook receives stage progress",
        BLUETOOTH_STARTUP_HOOK_CALLS.load(Ordering::Relaxed) > 0,
    );
    logger.record(
        "USB startup hook receives stage messages",
        USB_STARTUP_HOOK_CALLS.load(Ordering::Relaxed) > 0,
    );
    logger.record(
        "input trace observer receives controller events",
        INPUT_TRACE_OBSERVER_CALLS.load(Ordering::Relaxed) > 0,
    );
    let cleanup_trace_complete = write_trace_events(logger);
    capture_complete &= cleanup_trace_complete;
    logger.record(
        "cleanup trace events captured completely",
        cleanup_trace_complete,
    );
    logger.record("controller device observed", saw_device);
    let collection_complete =
        saw_device && capture_complete && cleanup_complete && logger.checks_passed();
    logger.record(
        "input collection completed without source, capture, or cleanup errors",
        collection_complete,
    );
    logger.note(if collection_complete {
        "collection_candidate=COMPLETE observation_window=finished"
    } else if !saw_device {
        "collection_candidate=INCOMPLETE reason=no_device_observed"
    } else {
        "collection_candidate=INCOMPLETE reason=source_capture_or_cleanup_error"
    });
    logger.note("hardware_verdict=PENDING");
    logger.summary();
    loop {
        logger.display().wait_vsync();
    }
}

#[cfg(feature = "dolphin-automation")]
static PAD_INIT_STAGES: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dolphin-automation")]
static PAD_TRACE_MOTOR_SEEN: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dolphin-automation")]
static PAD_SAMPLING_A: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "dolphin-automation")]
static PAD_SAMPLING_B: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "dolphin-automation")]
unsafe extern "C" fn pad_sampling_a() {
    PAD_SAMPLING_A.fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "dolphin-automation")]
unsafe extern "C" fn pad_sampling_b() {
    PAD_SAMPLING_B.fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "dolphin-automation")]
fn probe_pad_sampling_callback(logger: &mut ProbeLogger) {
    logger.section("PAD SAMPLING CALLBACK");
    PAD_SAMPLING_A.store(0, Ordering::Relaxed);
    PAD_SAMPLING_B.store(0, Ordering::Relaxed);
    use ogc_input_sys as sys;

    let original = sys::pad_set_sampling_callback(Some(pad_sampling_a));
    for _ in 0..60 {
        if PAD_SAMPLING_A.load(Ordering::Relaxed) != 0 {
            break;
        }
        if !video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let a_called = PAD_SAMPLING_A.load(Ordering::Relaxed) != 0;
    let replaced = sys::pad_set_sampling_callback(Some(pad_sampling_b));
    for _ in 0..60 {
        if PAD_SAMPLING_B.load(Ordering::Relaxed) != 0 {
            break;
        }
        if !video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let b_called = PAD_SAMPLING_B.load(Ordering::Relaxed) != 0;
    let removed = sys::pad_set_sampling_callback(None);
    let _ = video::wait_vsync_timeout_us(100_000);
    let stopped = PAD_SAMPLING_B.load(Ordering::Relaxed);
    let mut waited = true;
    for _ in 0..3 {
        waited &= video::wait_vsync_timeout_us(100_000);
    }
    let unchanged = PAD_SAMPLING_B.load(Ordering::Relaxed) == stopped;
    let previous = sys::pad_set_sampling_callback(original);
    logger.record(
        "PAD sampling callback replaces delivers and removes",
        a_called
            && b_called
            && waited
            && unchanged
            && previous.is_none()
            && replaced.is_some_and(|f| {
                core::ptr::fn_addr_eq(f, pad_sampling_a as unsafe extern "C" fn())
            })
            && removed.is_some_and(|f| {
                core::ptr::fn_addr_eq(f, pad_sampling_b as unsafe extern "C" fn())
            }),
    );
}

#[cfg(feature = "dolphin-automation")]
unsafe fn pad_init_debug_hook(stage: u32) {
    if stage < usize::BITS {
        PAD_INIT_STAGES.fetch_or(1usize << stage, Ordering::Relaxed);
    }
}

#[cfg(feature = "dolphin-automation")]
unsafe fn pad_low_level_trace(event: ogc_input_sys::PadDebugEvent) {
    if event.kind == ogc_input_sys::PAD_TRACE_MOTOR && event.channel == -1 && event.a == 2 {
        PAD_TRACE_MOTOR_SEEN.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(feature = "dolphin-automation")]
fn probe_pad_low_level_state(logger: &mut ProbeLogger) {
    use ogc_input_sys as sys;

    logger.section("PAD LOW LEVEL STATE");
    PAD_INIT_STAGES.store(0, Ordering::Relaxed);
    ogc_si::init();
    ogc_si::sys::init();
    sys::set_debug_hook(Some(pad_init_debug_hook));
    let initialized = sys::pad_init();
    sys::set_debug_hook(None);
    logger.record(
        "PAD repeated initialization reports debug stages",
        initialized == 1 && PAD_INIT_STAGES.load(Ordering::Relaxed) == 3,
    );

    PAD_TRACE_MOTOR_SEEN.store(0, Ordering::Relaxed);
    let previous = sys::set_trace_hook(Some(pad_low_level_trace));
    let before = sys::pad_debug_state();
    sys::pad_control_motor(-1, 2);
    let mut sync = 0;
    for _ in 0..120 {
        sync = sys::pad_sync();
        if sync == 1 {
            break;
        }
        if !video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let mut samples = [sys::PadStatus::default(); 4];
    let _ = video::wait_vsync_timeout_us(100_000);
    let connected = sys::pad_scan_pads_with_status(&mut samples);
    let _ = video::wait_vsync_timeout_us(100_000);
    let second_scan = sys::pad_scan_pads();
    let mut raw = [sys::PadStatus::default(); 4];
    // SAFETY: The array contains four writable status records.
    let _ = video::wait_vsync_timeout_us(100_000);
    let raw_mask = unsafe { sys::pad_read(raw.as_mut_ptr()) };
    let null_mask = unsafe { sys::pad_read(core::ptr::null_mut()) };
    let after = sys::pad_debug_state();
    let counters = sys::pad_debug_counters();
    let mut events = [sys::PadDebugEvent::default(); 128];
    let count = sys::pad_debug_drain(&mut events);
    let drained = sys::pad_debug_counters();
    let _ = sys::set_trace_hook(previous);
    let motor = events[..count].iter().any(|event| {
        event.kind == sys::PAD_TRACE_MOTOR && event.channel == -1 && event.a == 2 && event.b == 0
    });
    let sync_trace = events[..count]
        .iter()
        .any(|event| event.kind == sys::PAD_TRACE_SYNC && event.a == sync);
    logger.note_hex32("PAD low level connected", connected);
    logger.note_hex32("PAD low level second scan", second_scan);
    logger.note_hex32("PAD low level raw mask", raw_mask);
    logger.note_u32("PAD low level sync", sync);
    logger.note_u32("PAD low level reset channel", after.resetting_chan);
    logger.record(
        "PAD invalid motor request produces trace without state change",
        motor
            && PAD_TRACE_MOTOR_SEEN.load(Ordering::Relaxed) == 1
            && before.spec == after.spec
            && before.enable_bits == after.enable_bits,
    );
    logger.record(
        "PAD trace counters and drain preserve event sequence",
        count > 0
            && counters.dropped == 0
            && drained.drained == count as u32
            && events[..count]
                .iter()
                .enumerate()
                .all(|(i, event)| event.sequence == i as u32),
    );
    logger.record(
        "PAD sync matches trace and idle state",
        sync == 1 && sync_trace && after.resetting_bits == 0 && after.resetting_chan == 32,
    );
    logger.record(
        "PAD low level scans observe scripted controller",
        connected & 1 == 1
            && second_scan & 1 == 1
            && samples[0].err as i32 == sys::PAD_ERR_NONE
            && after.enable_bits & sys::PAD_CHAN0_BIT != 0,
    );
    logger.record(
        "PAD read reports connected channel and rejects null",
        raw_mask & sys::PAD_CHAN0_BIT != 0
            && null_mask == 0
            && raw[1..]
                .iter()
                .all(|status| status.err as i32 == sys::PAD_ERR_NO_CONTROLLER),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_ogc_input_public_wrappers(logger: &mut ProbeLogger) {
    use ogc_input_sys as sys;

    logger.section("PAD PUBLIC WRAPPERS");

    PAD_INIT_STAGES.store(0, Ordering::Relaxed);
    ogc_input::set_debug_hook(Some(pad_init_debug_hook));
    let initialized = ogc_input::init();
    ogc_input::set_debug_hook(None);
    logger.record(
        "PAD repeated initialization reports debug stages",
        initialized == 1 && PAD_INIT_STAGES.load(Ordering::Relaxed) == 3,
    );

    let before = sys::pad_debug_state();
    ogc_input::set_spec(before.spec);
    logger.record(
        "PAD set spec preserves the selected specification",
        sys::pad_debug_state().spec == before.spec,
    );

    PAD_TRACE_MOTOR_SEEN.store(0, Ordering::Relaxed);
    let previous_trace = sys::set_trace_hook(Some(pad_low_level_trace));
    let before_motor = sys::pad_debug_state();
    ogc_input::control_motor(-1, 2);
    for _ in 0..120 {
        let sync = sys::pad_sync();
        if sync == 1 || !video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let after_motor = sys::pad_debug_state();
    let _ = sys::set_trace_hook(previous_trace);
    logger.record(
        "PAD invalid motor request produces trace without state change",
        PAD_TRACE_MOTOR_SEEN.load(Ordering::Relaxed) == 1
            && before_motor.spec == after_motor.spec
            && before_motor.enable_bits == after_motor.enable_bits,
    );

    let mut samples = [sys::PadStatus::default(); 4];
    let _ = video::wait_vsync_timeout_us(100_000);
    let connected = ogc_input::scan_pads_with_status(&mut samples);
    let _ = video::wait_vsync_timeout_us(100_000);
    let second_scan = ogc_input::scan_pads();
    logger.record(
        "PAD low level scans observe scripted controller",
        connected & 1 == 1 && second_scan & 1 == 1 && samples[0].err as i32 == sys::PAD_ERR_NONE,
    );

    PAD_SAMPLING_A.store(0, Ordering::Relaxed);
    let original = ogc_input::set_sampling_callback(Some(pad_sampling_a));
    for _ in 0..60 {
        if PAD_SAMPLING_A.load(Ordering::Relaxed) != 0 {
            break;
        }
        if !video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let called = PAD_SAMPLING_A.load(Ordering::Relaxed) != 0;
    let removed = ogc_input::set_sampling_callback(None);
    let restored = ogc_input::set_sampling_callback(original);
    logger.record(
        "PAD sampling callback replaces delivers and removes",
        called
            && removed.is_some_and(|callback| {
                core::ptr::fn_addr_eq(callback, pad_sampling_a as unsafe extern "C" fn())
            })
            && restored.is_none(),
    );
}

#[cfg(feature = "dolphin-automation")]
fn pad_scan_hook(stage: &'static str) {
    let bit = match stage {
        "PAD SCAN START" => 1,
        "PAD SCAN DONE" => 2,
        _ => 0,
    };
    PAD_SCAN_HOOKS.fetch_or(bit, Ordering::Relaxed);
}

#[cfg(feature = "dolphin-automation")]
fn probe_controller_scan_modes(logger: &mut ProbeLogger, controllers: &mut ControllerManager) {
    fn has_pad(snapshot: &ControllerSnapshot) -> bool {
        snapshot
            .controllers()
            .iter()
            .any(|item| item.source == ControllerSource::GameCube(0) && item.connected)
    }

    PAD_SCAN_HOOKS.store(0, Ordering::Relaxed);
    controllers.set_input_scan_hook(Some(pad_scan_hook));
    logger.record(
        "controller scan sees scripted pad",
        has_pad(&controllers.scan()),
    );
    logger.record(
        "controller timed scan sees scripted pad",
        has_pad(&controllers.scan_at(system::time())),
    );
    let mut status = [pad::PadStatus::default(); MAX_GAMECUBE_CONTROLLERS];
    let snapshot = controllers.scan_with_pad_status(&mut status);
    logger.record(
        "controller scan returns matching pad status",
        has_pad(&snapshot) && status[0].err as i32 == pad::PAD_ERR_NONE,
    );
    let raw = controllers.scan_with_raw();
    logger.record(
        "controller raw scan returns matching pad status",
        has_pad(&raw.snapshot) && raw.pad_status[0].err as i32 == pad::PAD_ERR_NONE,
    );
    let timed = controllers.scan_with_raw_at(system::time());
    logger.record(
        "controller timed raw scan returns matching pad status",
        has_pad(&timed.snapshot) && timed.pad_status[0].err as i32 == pad::PAD_ERR_NONE,
    );
    controllers.set_input_scan_hook(None);
    logger.record(
        "controller scan hook observes PAD boundaries",
        PAD_SCAN_HOOKS.load(Ordering::Relaxed) == 3,
    );
}

#[cfg(feature = "dolphin-automation")]
struct PadAccessChecks {
    previous: Option<u16>,
    buttons_ok: bool,
    axes_ok: bool,
    saw_down: bool,
    saw_up: bool,
    saw_stick: bool,
}

#[cfg(feature = "dolphin-automation")]
impl PadAccessChecks {
    fn new() -> Self {
        Self {
            previous: None,
            buttons_ok: true,
            axes_ok: true,
            saw_down: false,
            saw_up: false,
            saw_stick: false,
        }
    }
}

#[cfg(feature = "dolphin-automation")]
fn probe_pad_accessors(checks: &mut PadAccessChecks, sample: &pad::PadStatus) {
    use ogc_input_sys as sys;
    if sample.err as i32 == pad::PAD_ERR_NO_CONTROLLER {
        checks.previous = None;
    }
    if sample.err as i32 != pad::PAD_ERR_NONE {
        return;
    }
    checks.buttons_ok &=
        pad::buttons_held(0) == sample.button && sys::pad_buttons_held(0) == sample.button;
    if let Some(previous) = checks.previous {
        let down = sample.button & !previous;
        let up = previous & !sample.button;
        checks.buttons_ok &= pad::buttons_down(0) == down
            && pad::buttons_up(0) == up
            && sys::pad_buttons_down(0) == down
            && sys::pad_buttons_up(0) == up;
        checks.saw_down |= down != 0;
        checks.saw_up |= up != 0;
    }
    checks.previous = Some(sample.button);
    checks.axes_ok &= pad::stick_x(0) == sample.stick_x
        && pad::stick_y(0) == sample.stick_y
        && pad::substick_x(0) == sample.substick_x
        && pad::substick_y(0) == sample.substick_y
        && pad::trigger_l(0) == sample.trigger_l
        && pad::trigger_r(0) == sample.trigger_r
        && sys::pad_stick_x(0) == sample.stick_x
        && sys::pad_stick_y(0) == sample.stick_y
        && sys::pad_sub_stick_x(0) == sample.substick_x
        && sys::pad_sub_stick_y(0) == sample.substick_y
        && sys::pad_trigger_l(0) == sample.trigger_l
        && sys::pad_trigger_r(0) == sample.trigger_r;
    checks.saw_stick |= sample.stick_x != 0 || sample.stick_y != 0;
}

#[cfg(feature = "dolphin-automation")]
#[derive(Default)]
struct InputAutomationState {
    controller_seen: bool,
    a_seen: bool,
    start_seen: bool,
    right_seen: bool,
    main_stick_seen: bool,
}

#[cfg(feature = "dolphin-automation")]
impl InputAutomationState {
    fn observe(&mut self, snapshot: &ControllerSnapshot) {
        for controller in snapshot.controllers() {
            self.controller_seen = true;
            self.a_seen |= controller.buttons.contains(Buttons::A)
                || controller.buttons_down.contains(Buttons::A);
            self.start_seen |= controller.buttons.contains(Buttons::START)
                || controller.buttons_down.contains(Buttons::START);
            self.right_seen |= controller.buttons.contains(Buttons::RIGHT)
                || controller.buttons_down.contains(Buttons::RIGHT);
            self.main_stick_seen |= controller.axes.left_x.unsigned_abs() > 8_000
                || controller.axes.left_y.unsigned_abs() > 8_000;
        }
    }
}

fn write_snapshot_events(
    logger: &mut ProbeLogger,
    probe: &mut InputProbeState,
    snapshot: &ControllerSnapshot,
    frame: &RawInputFrame,
    timestamp_ticks: u64,
) -> bool {
    let mut complete = true;
    let mut next_states = [None; MAX_CONTROLLERS];
    let mut next_metadata = [None; MAX_CONTROLLERS];
    for (index, controller) in snapshot.controllers().iter().copied().enumerate() {
        next_states[index] = Some(controller);
        let metadata = snapshot.metadata(controller.source);
        next_metadata[index] = metadata;
        let state_changed = probe.last_states[index]
            .map(|previous| previous != controller)
            .unwrap_or(true);
        let metadata_changed = probe.last_metadata[index] != metadata;
        let changed = state_changed || metadata_changed;
        if !changed {
            continue;
        }
        let (raw_down, raw_held, raw_up) = raw_masks(frame, controller.source);
        let pointer = metadata.and_then(|metadata| metadata.pointer);
        let (pointer_x, pointer_y, pointer_width, pointer_height) = pointer
            .map(|pointer| {
                let (x, y) = pointer.position();
                (x, y, pointer.resolution[0], pointer.resolution[1])
            })
            .unwrap_or((0.0, 0.0, 0, 0));
        let capabilities = metadata.map(|metadata| metadata.capabilities);
        let (slot_a, slot_b) = source_slots(controller.source);
        let mut line = EventLineBuf::new();
        let _ = write!(
            &mut line,
            "{{\"type\":\"decoded\",\"timestamp_ticks\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"kind\":\"{}\",\"connected\":{},\"buttons\":{},\"down\":{},\"up\":{},\"raw_down\":{},\"raw_held\":{},\"raw_up\":{},\"left_x\":{},\"left_y\":{},\"right_x\":{},\"right_y\":{},\"left_trigger\":{},\"right_trigger\":{},\"pointer_valid\":{},\"pointer_x\":{},\"pointer_y\":{},\"pointer_width\":{},\"pointer_height\":{},\"cap_buttons\":{},\"cap_left_stick\":{},\"cap_right_stick\":{},\"cap_triggers\":{},\"cap_motion\":{},\"cap_expansion\":{},\"hardware\":",
            timestamp_ticks,
            source_text(controller.source),
            slot_a,
            slot_b,
            kind_text(controller.kind),
            controller.connected,
            controller.buttons.bits(),
            controller.buttons_down.bits(),
            controller.buttons_up.bits(),
            raw_down,
            raw_held,
            raw_up,
            controller.axes.left_x,
            controller.axes.left_y,
            controller.axes.right_x,
            controller.axes.right_y,
            controller.axes.left_trigger,
            controller.axes.right_trigger,
            pointer.is_some(),
            pointer_x,
            pointer_y,
            pointer_width,
            pointer_height,
            capabilities.map(|cap| cap.buttons).unwrap_or(false),
            capabilities.map(|cap| cap.left_stick).unwrap_or(false),
            capabilities.map(|cap| cap.right_stick).unwrap_or(false),
            capabilities.map(|cap| cap.triggers).unwrap_or(false),
            capabilities.map(|cap| cap.motion).unwrap_or(false),
            capabilities.map(|cap| cap.expansion).unwrap_or(false)
        );
        write_hardware_sample(&mut line, frame, controller.source);
        let _ = write!(&mut line, "}}");
        complete &= emit_event_record(logger, &line);
    }
    probe.last_states = next_states;
    probe.last_metadata = next_metadata;

    for reset in snapshot.resets() {
        let mut line = EventLineBuf::new();
        let (slot_a, slot_b) = source_slots(reset.id.source);
        let _ = write!(
            &mut line,
            "{{\"type\":\"reset\",\"timestamp_ticks\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"generation\":{},\"reason\":\"{:?}\",\"released\":{}}}",
            timestamp_ticks,
            source_text(reset.id.source),
            slot_a,
            slot_b,
            reset.id.generation,
            reset.reason,
            reset.state.buttons_up.bits()
        );
        complete &= emit_event_record(logger, &line);
    }
    for error in snapshot.errors() {
        complete = false;
        logger.record("input source or trace error", false);
        let mut line = EventLineBuf::new();
        let (slot_a, slot_b) = source_slots(error.source);
        let _ = write!(
            &mut line,
            "{{\"type\":\"error\",\"timestamp_ticks\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"operation\":",
            timestamp_ticks,
            source_text(error.source),
            slot_a,
            slot_b
        );
        let _ = line.write_json_string(error.operation);
        let _ = write!(
            &mut line,
            ",\"failure\":\"{:?}\",\"cleanup_code\":{},\"cleanup_code_valid\":{}}}",
            error.failure,
            error.cleanup_code.unwrap_or(0),
            error.cleanup_code.is_some()
        );
        complete &= emit_event_record(logger, &line);
    }
    complete
}

fn write_hardware_sample(line: &mut EventLineBuf, frame: &RawInputFrame, source: ControllerSource) {
    match source {
        ControllerSource::GameCube(slot) => {
            let status = frame.pad_status[slot as usize];
            let _ = write!(
                line,
                "{{\"button_mask\":{},\"stick_x\":{},\"stick_y\":{},\"cstick_x\":{},\"cstick_y\":{},\"trigger_l\":{},\"trigger_r\":{},\"analog_a\":{},\"analog_b\":{},\"error\":{}}}",
                status.button,
                status.stick_x,
                status.stick_y,
                status.substick_x,
                status.substick_y,
                status.trigger_l,
                status.trigger_r,
                status.analog_a,
                status.analog_b,
                status.err
            );
        }
        ControllerSource::WiiBluetooth(slot) => {
            let Some(sample) = frame.wpad_data[slot as usize] else {
                let _ = write!(line, "null");
                return;
            };
            let pointer = sample.data_present & wpad::WPAD_DATA_IR != 0 && sample.ir.valid != 0;
            let (pointer_x, pointer_y, pointer_width, pointer_height) = if pointer {
                (
                    sample.ir.x,
                    sample.ir.y,
                    sample.ir.vres[0],
                    sample.ir.vres[1],
                )
            } else {
                (0.0, 0.0, 0, 0)
            };
            let expansion_present = sample.data_present & wpad::WPAD_DATA_EXPANSION != 0;
            let (left_x, left_y, right_x, right_y, trigger_l, trigger_r) = match sample.exp.type_ {
                value if expansion_present && value == wpad::WPAD_EXP_NUNCHUK as i32 => {
                    let expansion = unsafe { sample.exp.u.nunchuk };
                    (expansion.js.pos.x, expansion.js.pos.y, 0, 0, 0, 0)
                }
                value if expansion_present && value == wpad::WPAD_EXP_CLASSIC as i32 => {
                    let expansion = unsafe { sample.exp.u.classic };
                    (
                        expansion.ljs.pos.x,
                        expansion.ljs.pos.y,
                        expansion.rjs.pos.x,
                        expansion.rjs.pos.y,
                        expansion.ls_raw,
                        expansion.rs_raw,
                    )
                }
                value if expansion_present && value == wpad::WPAD_EXP_GUITARHERO3 as i32 => {
                    let expansion = unsafe { sample.exp.u.gh3 };
                    (expansion.js.pos.x, expansion.js.pos.y, 0, 0, 0, 0)
                }
                _ => (0, 0, 0, 0, 0, 0),
            };
            let _ = write!(
                line,
                "{{\"data_present\":{},\"extension_type\":{},\"buttons_held\":{},\"buttons_down\":{},\"buttons_up\":{},\"ir_valid\":{},\"ir_x\":{},\"ir_y\":{},\"ir_width\":{},\"ir_height\":{},\"raw_left_x\":{},\"raw_left_y\":{},\"raw_right_x\":{},\"raw_right_y\":{},\"raw_trigger_l\":{},\"raw_trigger_r\":{}}}",
                sample.data_present,
                sample.exp.type_,
                sample.btns_h,
                sample.btns_d,
                sample.btns_u,
                pointer,
                pointer_x,
                pointer_y,
                pointer_width,
                pointer_height,
                left_x,
                left_y,
                right_x,
                right_y,
                trigger_l,
                trigger_r
            );
        }
        _ => {
            let _ = write!(line, "null");
        }
    }
}

fn raw_masks(frame: &RawInputFrame, source: ControllerSource) -> (u32, u32, u32) {
    match source {
        ControllerSource::GameCube(slot) => {
            let slot = slot as usize;
            (
                u32::from(frame.pad_buttons_down[slot]),
                u32::from(frame.pad_buttons_held[slot]),
                u32::from(frame.pad_buttons_up[slot]),
            )
        }
        ControllerSource::WiiBluetooth(slot) => {
            let slot = slot as usize;
            (
                frame.wpad_buttons_down[slot],
                frame.wpad_buttons_held[slot],
                frame.wpad_buttons_up[slot],
            )
        }
        _ => (0, 0, 0),
    }
}

pub(crate) fn write_trace_events(logger: &mut ProbeLogger) -> bool {
    let mut complete = true;
    while let Some(event) = TRACE_QUEUE.pop() {
        let mut line = EventLineBuf::new();
        let (slot_a, slot_b) = source_slots(event.source);
        let _ = write!(
            &mut line,
            "{{\"type\":\"raw_report\",\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"generation\":{},\"generation_valid\":{},\"timestamp_ticks\":{},\"device_id\":{},\"vid\":{},\"pid\":{},\"usb_identity_valid\":{},\"data\":\"",
            source_text(event.source),
            slot_a,
            slot_b,
            event.generation.unwrap_or(0),
            event.generation.is_some(),
            event.timestamp_ticks.unwrap_or(0),
            event
                .usb_identity
                .map(|identity| identity.device_id)
                .unwrap_or(0),
            event.usb_identity.map(|identity| identity.vid).unwrap_or(0),
            event.usb_identity.map(|identity| identity.pid).unwrap_or(0),
            event.usb_identity.is_some()
        );
        let _ = line.write_hex_bytes(&event.bytes[..event.len]);
        let _ = write!(&mut line, "\"}}");
        complete &= emit_event_record(logger, &line);
    }
    complete
}

fn kind_text(kind: ControllerKind) -> &'static str {
    match kind {
        ControllerKind::Unknown => "unknown",
        ControllerKind::GameCube => "gamecube",
        ControllerKind::WiiRemote => "wii_remote",
        ControllerKind::WiiClassic => "classic_controller",
        ControllerKind::WiiUPro => "wii_u_pro",
        ControllerKind::JoyConLeft => "joycon_left",
        ControllerKind::JoyConRight => "joycon_right",
        ControllerKind::JoyConPair => "joycon_pair",
        ControllerKind::SwitchPro => "switch_pro",
        ControllerKind::XboxOne => "xbox_one",
        ControllerKind::GenericBluetoothHid => "generic_bluetooth_hid",
        ControllerKind::GenericUsbHid => "generic_usb_hid",
    }
}

fn refresh_probe_state(
    state: &mut InputProbeState,
    status: &[pad::PadStatus; MAX_GAMECUBE_CONTROLLERS],
) {
    state.pad_scan_mask = 0;
    state.pad_valid_mask = 0;

    for (chan, pad_status) in status.iter().enumerate().take(MAX_GAMECUBE_CONTROLLERS) {
        state.gc_type[chan] = si::get_type(chan as i32);
        state.gc_err[chan] = pad_status.err as i32;
        state.gc_buttons[chan] = pad_status.button as u32;
        state.gc_response[chan] = si::debug_last_response(chan as i32).unwrap_or([0; 2]);
        state.gc_command[chan] = si::get_command(chan as i32);
        if pad_status.err as i32 == pad::PAD_ERR_NONE {
            state.pad_scan_mask |= 1 << chan;
            state.pad_valid_mask |= pad::PAD_CHAN0_BIT >> chan;
        }
    }
}

fn draw_dashboard(state: &InputProbeState, snapshot: &ControllerSnapshot) {
    video::console_clear();

    draw_at(0, 0, "INPUT PROBE");
    draw_at(0, 1, "RAW / DISCOVERY");
    draw_at(40, 1, "LIVE CONTROLS");

    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "pad init={}",
        if state.pad_init_ok { "ok" } else { "fail" }
    );
    draw_at(0, 3, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "pad valid=0x{:08x}", state.pad_valid_mask);
    draw_at(0, 4, line.as_str());

    let mut line = LineBuf::new();
    let _ = write!(&mut line, "pad scan=0x{:08x}", state.pad_scan_mask);
    draw_at(0, 5, line.as_str());

    for chan in 0..MAX_GAMECUBE_CONTROLLERS {
        let mut line = LineBuf::new();
        let _ = write!(
            &mut line,
            "gc{} t=0x{:08x} e={} b=0x{:04x}",
            chan + 1,
            state.gc_type[chan],
            state.gc_err[chan],
            state.gc_buttons[chan]
        );
        draw_at(0, 7 + chan as i32, line.as_str());

        let mut response = LineBuf::new();
        let _ = write!(
            &mut response,
            "si{} {:08x} {:08x}",
            chan + 1,
            state.gc_response[chan][0],
            state.gc_response[chan][1]
        );
        draw_at(0, 12 + chan as i32 * 2, response.as_str());
        let mut command = LineBuf::new();
        let _ = write!(&mut command, " cmd={:08x}", state.gc_command[chan]);
        draw_at(0, 13 + chan as i32 * 2, command.as_str());
    }

    draw_at(0, 20, "PAD / WPAD / USB / Bluetooth live scan");

    draw_at(0, 22, "Move sticks; press buttons; read-only test");

    draw_live_controls(state, snapshot);
}

fn draw_live_controls(state: &InputProbeState, snapshot: &ControllerSnapshot) {
    let mut line = LineBuf::new();
    let _ = write!(&mut line, "active={}", state.active_controllers);
    draw_at(40, 3, line.as_str());

    if snapshot.is_empty() {
        draw_at(40, 5, "CTRL none");
        draw_at(40, 6, "Press/plug controller now");
        return;
    }

    let mut row = 5;
    for controller in snapshot.controllers() {
        if row >= 22 {
            break;
        }

        let mut source = LineBuf::new();
        write_source(&mut source, controller.source);
        let _ = write!(&mut source, " ");
        write_kind(&mut source, controller.kind);
        draw_at(40, row, source.as_str());
        row += 1;

        let mut held = LineBuf::new();
        let _ = write!(&mut held, "H:");
        write_button_list(&mut held, controller.buttons);
        draw_at(40, row, held.as_str());
        row += 1;

        let mut down = LineBuf::new();
        let _ = write!(&mut down, "D:");
        write_button_list(&mut down, controller.buttons_down);
        draw_at(40, row, down.as_str());
        row += 1;

        let mut axes = LineBuf::new();
        let _ = write!(
            &mut axes,
            "LX:{} LY:{} RX:{} RY:{}",
            controller.axes.left_x,
            controller.axes.left_y,
            controller.axes.right_x,
            controller.axes.right_y
        );
        draw_at(40, row, axes.as_str());
        row += 2;
    }
}

fn draw_at(col: i32, row: i32, text: &str) {
    let mut seq = LineBuf::new();
    let _ = write!(&mut seq, "\x1b[{};{}H", row + 1, col + 1);
    let _ = crate::probe_support::screen_write(seq.as_bytes());
    let _ = crate::probe_support::screen_write(text.as_bytes());
}

fn write_source(line: &mut LineBuf, source: ControllerSource) {
    let _ = write!(line, "{}", source_text(source));
}

fn write_kind(line: &mut LineBuf, kind: ControllerKind) {
    let _ = write!(line, "{}", kind_text(kind));
}

fn write_button_list(line: &mut LineBuf, buttons: Buttons) {
    let mut first = true;
    for &(button, name) in BUTTON_LABELS {
        if buttons.contains(button) {
            if !first {
                let _ = write!(line, "|");
            }
            let _ = write!(line, "{}", name);
            first = false;
        }
    }
    if first {
        let _ = write!(line, "-");
    }
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
