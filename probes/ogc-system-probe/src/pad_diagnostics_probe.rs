use alloc::format;
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ogc_input as pad;
use ogc_lwp::{self as lwp, ThreadHandle, sys as lwp_sys};
use ogc_si as si;
use ogc_system::{self as system, LwpNode, SysResetInfo};

use crate::probe_support::{self, ProbeLogger};

const TRACE_BATCH: usize = 32;
const SYNC_FRAMES: u32 = 300;
const OPERATOR_FRAMES: u32 = 1_800;

static PAD_HOOK_CALLS: AtomicU32 = AtomicU32::new(0);
static RESET_HOOK_CALLS: AtomicU32 = AtomicU32::new(0);
static LWP_HOOK_CALLS: AtomicU32 = AtomicU32::new(0);
static STRESS_STOP: AtomicBool = AtomicBool::new(false);
static STRESS_OPERATIONS: AtomicU32 = AtomicU32::new(0);
static TRACE_VERBOSE: AtomicBool = AtomicBool::new(true);

#[repr(align(32))]
struct ThreadStack([u8; 16 * 1024]);

static mut STRESS_STACKS: [ThreadStack; 4] = [
    ThreadStack([0; 16 * 1024]),
    ThreadStack([0; 16 * 1024]),
    ThreadStack([0; 16 * 1024]),
    ThreadStack([0; 16 * 1024]),
];

#[derive(Default)]
struct TraceSummary {
    events: u32,
    selected_mask: u32,
    selected_count: [u32; 4],
    command_41: u32,
    command_42: u32,
    command_4d: u32,
    transfer_rejections: u32,
    callback_errors: u32,
    reset_callbacks: u32,
    polling_disable_mask: u32,
}

unsafe fn pad_trace_hook(_event: pad::PadDebugEvent) {
    PAD_HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
}

unsafe fn reset_trace_hook(_stage: u32, _priority: u32, _result: i32) {
    RESET_HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
}

unsafe fn lwp_trace_hook(_event: lwp_sys::DebugEvent) {
    LWP_HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn drain_lwp_trace() -> u32 {
    let mut events = [lwp_sys::DebugEvent::default(); 64];
    let mut count = 0u32;
    loop {
        let drained = lwp_sys::debug_drain(&mut events);
        count = count.saturating_add(drained as u32);
        if drained < events.len() {
            return count;
        }
    }
}

fn begin(logger: &mut ProbeLogger, id: &str) {
    logger.note(&format!("BEGIN {id}"));
}

fn persistent_log_ready(logger: &ProbeLogger) -> bool {
    logger.has_log_file() || cfg!(feature = "dolphin-automation")
}

fn end(logger: &mut ProbeLogger, summary: &mut TraceSummary, id: &str, ok: bool) {
    drain_trace(logger, summary);
    let faults = si::si_fault_report();
    logger.note(&format!(
        "SIFAULT id={id} installed={} consumed={} unmatched={} dropped={} held={}",
        faults.installed,
        faults.consumed,
        faults.unmatched_operations,
        faults.dropped_steps,
        faults.held_operations
    ));
    let fault_state_ok = faults.installed == faults.consumed
        && faults.dropped_steps == 0
        && faults.held_operations == 0;
    logger.record(id, ok && fault_state_ok);
    logger.note(&format!("END {id}"));
    let checkpointed = logger.checkpoint(id);
    logger.record(&format!("{id} log checkpoint"), checkpointed);
}

fn install_pad_trace(logger: &mut ProbeLogger) -> bool {
    PAD_HOOK_CALLS.store(0, Ordering::Release);
    let previous = pad::set_trace_hook(Some(pad_trace_hook));
    logger.record("PAD trace hook installed", previous.is_none());
    previous.is_none()
}

fn finish_pad_trace(logger: &mut ProbeLogger, summary: &mut TraceSummary) {
    drain_trace(logger, summary);
    let counters = pad::pad_debug_counters();
    logger.note_u32("pad trace produced", counters.produced);
    logger.note_u32("pad trace drained", counters.drained);
    logger.note_u32("pad trace dropped", counters.dropped);
    logger.note_u32(
        "pad trace hook calls",
        PAD_HOOK_CALLS.load(Ordering::Acquire),
    );
    logger.record(
        "PAD trace hook received events",
        PAD_HOOK_CALLS.load(Ordering::Acquire) != 0,
    );
    logger.record("PAD trace had no drops", counters.dropped == 0);
    let previous = pad::set_trace_hook(None);
    logger.record("PAD trace hook restored", previous.is_some());
}

fn drain_trace(logger: &mut ProbeLogger, summary: &mut TraceSummary) {
    let mut events = [pad::PadDebugEvent::default(); TRACE_BATCH];
    loop {
        let count = pad::pad_debug_drain(&mut events);
        if count == 0 {
            break;
        }
        for event in &events[..count] {
            summary.events = summary.events.saturating_add(1);
            if event.kind == pad::PAD_TRACE_SELECT_CHANNEL && (0..4).contains(&event.channel) {
                summary.selected_mask |= pad::PAD_CHAN0_BIT >> event.channel;
                let channel = event.channel as usize;
                summary.selected_count[channel] = summary.selected_count[channel].saturating_add(1);
            }
            if event.kind == pad::PAD_TRACE_TRANSFER {
                match event.a {
                    0x41 => summary.command_41 = summary.command_41.saturating_add(1),
                    0x42 => summary.command_42 = summary.command_42.saturating_add(1),
                    0x4d => summary.command_4d = summary.command_4d.saturating_add(1),
                    _ => {}
                }
            }
            if event.kind == pad::PAD_TRACE_DISABLE_POLLING {
                summary.polling_disable_mask |= event.a;
            }
            if event.kind == pad::PAD_TRACE_TRANSFER_RESULT && event.b == 0 {
                summary.transfer_rejections = summary.transfer_rejections.saturating_add(1);
            }
            if event.kind == pad::PAD_TRACE_CALLBACK && event.c != 0 {
                summary.callback_errors = summary.callback_errors.saturating_add(1);
            }
            if event.kind == pad::PAD_TRACE_RESET_CALLBACK {
                summary.reset_callbacks = summary.reset_callbacks.saturating_add(1);
            }
            if TRACE_VERBOSE.load(Ordering::Relaxed) || event.sequence % 1_024 == 0 {
                logger.note(&format!(
                    "PADTRACE seq={} kind={} chan={} a={:08x} b={:08x} c={:08x} d={:08x}",
                    event.sequence, event.kind, event.channel, event.a, event.b, event.c, event.d
                ));
            }
        }
    }
}

fn type_step(channel: i32, result: u32, outcome: u32) -> si::SiFaultStep {
    si::SiFaultStep::new(
        si::SI_FAULT_OPERATION_TYPE_ASYNC,
        channel,
        0xff,
        0,
        outcome,
        result,
        0,
        [0; 10],
    )
}

fn transfer_step(
    channel: i32,
    command: u8,
    result: u32,
    outcome: u32,
    payload: [u8; 10],
) -> si::SiFaultStep {
    si::SiFaultStep::new(
        si::SI_FAULT_OPERATION_TRANSFER,
        channel,
        command,
        0,
        outcome,
        result,
        payload.len() as u32,
        payload,
    )
}

fn initialize_empty_pad(spec: u32) -> bool {
    pad::set_spec(spec);
    let steps = [
        type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(1, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(2, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(3, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
    ];
    si::si_fault_install(&steps) == steps.len() && pad::init() == 1 && pad::sync() == 1
}

fn state_clean() -> bool {
    let state = pad::pad_debug_state();
    state.resetting_bits == 0
        && state.pending_bits == 0
        && state.resetting_chan == 32
        && state.recalibrate_bits == 0
}

fn wait_sync(logger: &mut ProbeLogger, summary: &mut TraceSummary, frames: u32) -> bool {
    for _ in 0..frames {
        drain_trace(logger, summary);
        if pad::sync() != 0 {
            drain_trace(logger, summary);
            return true;
        }
        probe_support::wait_vsync();
    }
    drain_trace(logger, summary);
    false
}

fn run_mask_case(logger: &mut ProbeLogger, summary: &mut TraceSummary, id: &str, mask: u32) {
    begin(logger, id);
    let before = summary.selected_count;
    let mut steps = [type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE); 4];
    let mut count = 0;
    for channel in 0..4 {
        if mask & (pad::PAD_CHAN0_BIT >> channel) != 0 {
            steps[count] = type_step(channel, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE);
            count += 1;
        }
    }

    si::si_fault_install(&steps[..count]);
    let accepted = pad::recalibrate(mask) == 1;
    let completed = wait_sync(logger, summary, SYNC_FRAMES);
    let mut selected = 0;
    for channel in 0..4 {
        if summary.selected_count[channel] != before[channel] {
            selected |= pad::PAD_CHAN0_BIT >> channel;
        }
    }
    let faults = si::si_fault_report();
    end(
        logger,
        summary,
        id,
        accepted
            && completed
            && selected == (mask & pad::PAD_VALID_MASK)
            && faults.consumed == count as u32
            && state_clean(),
    );
}

pub fn run_state(logger: &mut ProbeLogger) {
    logger.section("PAD deterministic state and fault probe");
    logger.note("schema=pad-recalibration-v1 environment=deterministic-si-boundary");
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    install_pad_trace(logger);
    logger.record(
        "PAD initialized through production callbacks",
        initialize_empty_pad(5),
    );
    drain_trace(logger, &mut summary);
    si::si_fault_clear();

    run_mask_case(logger, &mut summary, "mask-channel-0", pad::PAD_CHAN0_BIT);
    run_mask_case(logger, &mut summary, "mask-channel-1", pad::PAD_CHAN1_BIT);
    run_mask_case(logger, &mut summary, "mask-channel-2", pad::PAD_CHAN2_BIT);
    run_mask_case(logger, &mut summary, "mask-channel-3", pad::PAD_CHAN3_BIT);
    run_mask_case(logger, &mut summary, "mask-all", pad::PAD_VALID_MASK);
    run_mask_case(
        logger,
        &mut summary,
        "mask-mixed-0-2",
        pad::PAD_CHAN0_BIT | pad::PAD_CHAN2_BIT,
    );

    begin(logger, "invalid-mask-filter");
    si::si_fault_clear();
    let accepted = pad::recalibrate(0x0fff_ffff) == 1 && pad::recalibrate(0) == 1;
    let invalid_state = pad::pad_debug_state();
    end(
        logger,
        &mut summary,
        "invalid-mask-filter",
        accepted
            && invalid_state.resetting_bits == 0
            && invalid_state.resetting_chan == 32
            && invalid_state.pending_bits == 0,
    );

    begin(logger, "return-busy-read-lifecycle");
    let hold = [type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_HOLD)];
    si::si_fault_install(&hold);
    let accepted = pad::recalibrate(pad::PAD_CHAN0_BIT) == 1;
    let busy = pad::sync() == 0;
    let mut statuses = [pad::PadStatus::default(); 4];
    let _ = pad::read(&mut statuses);
    let read_busy = matches!(
        statuses[0].err as i32,
        pad::PAD_ERR_NOT_READY | pad::PAD_ERR_TRANSFER
    );
    let released = si::si_fault_release(0) == 1;
    let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    end(
        logger,
        &mut summary,
        "return-busy-read-lifecycle",
        accepted && busy && read_busy && released && completed && state_clean(),
    );

    begin(logger, "active-reset-queue");
    let queue_steps = [
        type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_HOLD),
        type_step(1, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
    ];
    si::si_fault_install(&queue_steps);
    let first = pad::recalibrate(pad::PAD_CHAN0_BIT) == 1;
    let second = pad::recalibrate(pad::PAD_CHAN1_BIT) == 1;
    let queued = {
        let state = pad::pad_debug_state();
        state.resetting_chan == 0 && state.resetting_bits == pad::PAD_CHAN1_BIT
    };
    let released = si::si_fault_release(0) == 1;
    let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    end(
        logger,
        &mut summary,
        "active-reset-queue",
        first && second && queued && released && completed && state_clean(),
    );

    begin(logger, "transfer-submission-failure-recovery");
    let reject_steps = [
        type_step(0, si::SI_GC_CONTROLLER, si::SI_FAULT_COMPLETE),
        transfer_step(0, 0x42, 0, si::SI_FAULT_REJECT, [0; 10]),
    ];
    si::si_fault_install(&reject_steps);
    let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
    drain_trace(logger, &mut summary);
    let pending = pad::pad_debug_state().pending_bits == pad::PAD_CHAN0_BIT;
    let recover_steps = [
        type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(1, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
    ];
    si::si_fault_install(&recover_steps);
    let _ = pad::reset(pad::PAD_CHAN1_BIT);
    let recovered = wait_sync(logger, &mut summary, SYNC_FRAMES) && state_clean();
    let rejection_observed = summary.transfer_rejections != 0;
    end(
        logger,
        &mut summary,
        "transfer-submission-failure-recovery",
        pending && recovered && rejection_observed,
    );

    let errors = [
        si::SI_ERROR_UNDER_RUN,
        si::SI_ERROR_OVER_RUN,
        si::SI_ERROR_COLLISION,
        si::SI_ERROR_NO_RESPONSE,
        si::SI_ERROR_UNKNOWN,
        si::SI_ERROR_BUSY,
    ];
    for (index, error) in errors.iter().enumerate() {
        let id = format!("si-type-error-{index}");
        begin(logger, &id);
        si::si_fault_install(&[type_step(0, *error, si::SI_FAULT_COMPLETE)]);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        end(logger, &mut summary, &id, completed && state_clean());

        let id = format!("si-origin-error-{index}");
        begin(logger, &id);
        let steps = [
            type_step(0, si::SI_GC_CONTROLLER, si::SI_FAULT_COMPLETE),
            transfer_step(0, 0x42, *error, si::SI_FAULT_COMPLETE, [0; 10]),
        ];
        si::si_fault_install(&steps);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        end(logger, &mut summary, &id, completed && state_clean());
    }

    for (id, type_) in [
        ("non-controller-keyboard", si::SI_GC_KEYBOARD),
        ("non-controller-steering", si::SI_GC_STEERING),
        ("no-controller", si::SI_ERROR_NO_RESPONSE),
    ] {
        begin(logger, id);
        let commands_before = summary.command_41 + summary.command_42;
        si::si_fault_install(&[type_step(0, type_, si::SI_FAULT_COMPLETE)]);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let commands_after = summary.command_41 + summary.command_42;
        end(
            logger,
            &mut summary,
            id,
            completed && state_clean() && commands_before == commands_after,
        );
    }

    begin(logger, "waiting-checking-exclusion");
    let receiver = si::SI_GC_WAVEBIRD & !si::SI_WIRELESS_RECEIVED;
    let steps = [
        type_step(0, receiver, si::SI_FAULT_COMPLETE),
        transfer_step(0, 0x4d, 0, si::SI_FAULT_COMPLETE, [0; 10]),
    ];
    si::si_fault_install(&steps);
    let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
    drain_trace(logger, &mut summary);
    let waiting_before = pad::pad_debug_state().waiting_bits == pad::PAD_CHAN0_BIT;
    si::si_fault_clear();
    let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
    let excluded = {
        let state = pad::pad_debug_state();
        state.waiting_bits == pad::PAD_CHAN0_BIT
            && state.resetting_bits == 0
            && state.resetting_chan == 32
    };
    end(
        logger,
        &mut summary,
        "waiting-checking-exclusion",
        waiting_before && excluded,
    );

    si::si_fault_clear();
    logger.note_u32("selected channel mask", summary.selected_mask);
    logger.note_u32("callback errors observed", summary.callback_errors);
    finish_pad_trace(logger, &mut summary);
}

fn run_origin_transfer(
    logger: &mut ProbeLogger,
    summary: &mut TraceSummary,
    mask: u32,
    command: u8,
    payload: [u8; 10],
    recalibrate: bool,
) -> bool {
    let channel = mask.leading_zeros() as i32;
    let steps = [
        type_step(channel, si::SI_GC_CONTROLLER, si::SI_FAULT_COMPLETE),
        transfer_step(channel, command, 0, si::SI_FAULT_COMPLETE, payload),
    ];
    si::si_fault_install(&steps);
    let accepted = if recalibrate {
        pad::recalibrate(mask) == 1
    } else {
        pad::reset(mask) == 1
    };
    accepted && wait_sync(logger, summary, SYNC_FRAMES)
}

pub fn run_command(logger: &mut ProbeLogger, spec: u32) {
    logger.section("PAD command, origin, and legacy specification probe");
    logger.note(&format!("schema=pad-recalibration-v1 spec={spec}"));
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    install_pad_trace(logger);
    logger.record(
        "PAD initialized with requested legacy spec",
        initialize_empty_pad(spec),
    );
    drain_trace(logger, &mut summary);

    begin(logger, "legacy-origin-transfer-selection");
    let before_41 = summary.command_41;
    let before_42 = summary.command_42;
    if spec < 2 {
        si::si_fault_install(&[type_step(0, si::SI_GC_CONTROLLER, si::SI_FAULT_COMPLETE)]);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
        let _ = wait_sync(logger, &mut summary, SYNC_FRAMES);
    } else {
        let expected = if spec == 4 { 0x42 } else { 0x41 };
        let _ = run_origin_transfer(
            logger,
            &mut summary,
            pad::PAD_CHAN0_BIT,
            expected,
            [0, 0, 128, 128, 128, 128, 0, 0, 0, 0],
            false,
        );
    }
    let ordinary_ok = if spec < 2 {
        summary.command_41 == before_41 && summary.command_42 == before_42
    } else if spec == 4 {
        summary.command_42 == before_42 + 1
    } else {
        summary.command_41 == before_41 + 1
    };
    end(
        logger,
        &mut summary,
        "legacy-origin-transfer-selection",
        ordinary_ok && state_clean(),
    );

    if spec >= 2 {
        begin(logger, "calibration-command-42");
        let before = summary.command_42;
        let completed = run_origin_transfer(
            logger,
            &mut summary,
            pad::PAD_CHAN0_BIT,
            0x42,
            [0, 0, 130, 126, 140, 120, 10, 20, 30, 40],
            true,
        );
        drain_trace(logger, &mut summary);
        let command_ok = summary.command_42 == before + 1;
        end(
            logger,
            &mut summary,
            "calibration-command-42",
            completed && command_ok,
        );

        begin(logger, "full-origin-payload-transform");
        let expected = [0, 0, 2, 254, 12, 248, 10, 20, 30, 40];
        let mut origin_ok = true;
        for (index, expected_byte) in expected.iter().enumerate() {
            let actual = pad::pad_debug_origin_byte(0, index);
            logger.note(&format!(
                "origin[{index}]={actual} expected={expected_byte}"
            ));
            origin_ok &= actual == *expected_byte;
        }
        let status = pad::pad_debug_make_status(0, [0x0000_8080, 0x8080_1e28]);
        logger.note(&format!(
            "status sx={} sy={} cx={} cy={} l={} r={} aa={} ab={}",
            status.stick_x,
            status.stick_y,
            status.substick_x,
            status.substick_y,
            status.trigger_l,
            status.trigger_r,
            status.analog_a,
            status.analog_b
        ));
        end(
            logger,
            &mut summary,
            "full-origin-payload-transform",
            origin_ok
                && status.stick_x == -2
                && status.stick_y == 2
                && status.substick_x == -12
                && status.substick_y == 8
                && status.trigger_l == 20
                && status.trigger_r == 20,
        );

        begin(logger, "main-stick-x-patch");
        let completed = run_origin_transfer(
            logger,
            &mut summary,
            pad::PAD_CHAN0_BIT,
            0x42,
            [0, 0, 200, 128, 128, 128, 0, 0, 0, 0],
            true,
        );
        end(
            logger,
            &mut summary,
            "main-stick-x-patch",
            completed && pad::pad_debug_origin_byte(0, 2) == 0,
        );

        begin(logger, "recalibration-disabled-low-memory");
        let old = pad::_pad_disable_recalibration(1);
        let before = summary.command_41;
        let steps = [
            type_step(0, si::SI_GC_CONTROLLER, si::SI_FAULT_COMPLETE),
            transfer_step(
                0,
                0x41,
                0,
                si::SI_FAULT_COMPLETE,
                [0, 0, 128, 128, 128, 128, 0, 0, 0, 0],
            ),
        ];
        si::si_fault_install(&steps);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::_pad_disable_recalibration(old as i32);
        let command_ok = summary.command_41 == before + 1;
        end(
            logger,
            &mut summary,
            "recalibration-disabled-low-memory",
            completed && command_ok,
        );
    }

    si::si_fault_clear();
    finish_pad_trace(logger, &mut summary);
}

fn connected_mask(statuses: &[pad::PadStatus; 4]) -> u32 {
    let mut mask = 0;
    for (channel, status) in statuses.iter().enumerate() {
        if status.err as i32 == pad::PAD_ERR_NONE {
            mask |= pad::PAD_CHAN0_BIT >> channel;
        }
    }
    mask
}

fn wait_for_start(frames: u32) -> bool {
    for _ in 0..frames {
        let _ = pad::scan_pads();
        if (0..4).any(|channel| pad::buttons_down(channel) & pad::PAD_BUTTON_START != 0) {
            return true;
        }
        probe_support::wait_vsync();
    }
    false
}

pub fn run_wired(logger: &mut ProbeLogger) {
    logger.section("PAD guided wired hardware probe");
    logger.note("Connect up to four wired controllers. START confirms operator prompts.");
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    install_pad_trace(logger);
    let _ = pad::init();
    let discovery_completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    logger.record("initial discovery completed", discovery_completed);
    let mut statuses = [pad::PadStatus::default(); 4];
    let _ = pad::read(&mut statuses);
    let connected = connected_mask(&statuses);
    logger.note_hex32("connected wired mask", connected);
    logger.record("at least one wired controller present", connected != 0);

    for channel in 0..4 {
        let mask = pad::PAD_CHAN0_BIT >> channel;
        let id = format!("wired-channel-{channel}-recalibrate");
        begin(logger, &id);
        if connected & mask == 0 {
            end(logger, &mut summary, &id, false);
            continue;
        }
        let before = statuses[channel as usize];
        let accepted = pad::recalibrate(mask) == 1;
        let busy = pad::sync() == 0;
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::read(&mut statuses);
        let after = statuses[channel as usize];
        for index in 0..10 {
            logger.note(&format!(
                "channel={channel} origin[{index}]={}",
                pad::pad_debug_origin_byte(channel, index)
            ));
        }
        logger.note(&format!(
            "before sx={} sy={} cx={} cy={} l={} r={} aa={} ab={} after sx={} sy={} cx={} cy={} l={} r={} aa={} ab={}",
            before.stick_x, before.stick_y, before.substick_x, before.substick_y,
            before.trigger_l, before.trigger_r, before.analog_a, before.analog_b,
            after.stick_x, after.stick_y, after.substick_x, after.substick_y,
            after.trigger_l, after.trigger_r, after.analog_a, after.analog_b
        ));
        end(
            logger,
            &mut summary,
            &id,
            accepted && busy && completed && after.err as i32 == pad::PAD_ERR_NONE,
        );
    }

    begin(logger, "wired-full-analog-origin");
    logger.note("Move main X/Y, C-stick X/Y, L/R, analog A/B, then press START with controls released at the requested new origin.");
    let analog_confirmed = wait_for_start(OPERATOR_FRAMES);
    let first_channel = connected.leading_zeros() as i32;
    let analog_completed = if first_channel < 4 {
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> first_channel);
        wait_sync(logger, &mut summary, SYNC_FRAMES)
    } else {
        false
    };
    let _ = pad::read(&mut statuses);
    if first_channel < 4 {
        let value = statuses[first_channel as usize];
        logger.note(&format!(
            "full-analog sx={} sy={} cx={} cy={} l={} r={} aa={} ab={}",
            value.stick_x,
            value.stick_y,
            value.substick_x,
            value.substick_y,
            value.trigger_l,
            value.trigger_r,
            value.analog_a,
            value.analog_b
        ));
    }
    end(
        logger,
        &mut summary,
        "wired-full-analog-origin",
        analog_confirmed && analog_completed,
    );

    if connected.count_ones() >= 2 {
        begin(logger, "wired-polling-isolation");
        let selected = connected.leading_zeros() as i32;
        let unselected = (selected + 1..4)
            .find(|channel| connected & (pad::PAD_CHAN0_BIT >> channel) != 0)
            .unwrap_or(selected);
        let before_unselected = statuses[unselected as usize];
        let selected_mask = pad::PAD_CHAN0_BIT >> selected;
        let _ = pad::recalibrate(selected_mask);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::read(&mut statuses);
        let after_unselected = statuses[unselected as usize];
        let polling_mask_ok = summary.polling_disable_mask & selected_mask != 0;
        end(
            logger,
            &mut summary,
            "wired-polling-isolation",
            completed && polling_mask_ok && after_unselected.err == 0 && before_unselected.err == 0,
        );
    } else {
        logger.record("wired-polling-isolation requires two controllers", false);
    }

    begin(logger, "wired-multi-channel-isolation");
    let before = statuses;
    let _ = pad::recalibrate(connected);
    let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    let _ = pad::read(&mut statuses);
    let independent = statuses.iter().enumerate().all(|(channel, status)| {
        connected & (pad::PAD_CHAN0_BIT >> channel) == 0 || status.err == 0
    });
    logger.note(&format!("pre-read={before:?} post-read={statuses:?}"));
    end(
        logger,
        &mut summary,
        "wired-multi-channel-isolation",
        completed && independent,
    );

    if connected.count_ones() >= 2 {
        begin(logger, "unselected-controller-and-rumble");
        let selected = connected.leading_zeros() as i32;
        let unselected = (selected + 1..4)
            .find(|channel| connected & (pad::PAD_CHAN0_BIT >> channel) != 0)
            .unwrap_or(selected);
        pad::control_motor(selected, pad::PAD_MOTOR_RUMBLE);
        pad::control_motor(unselected, pad::PAD_MOTOR_RUMBLE);
        logger.note("Confirm both motors run, then press START.");
        let started = wait_for_start(OPERATOR_FRAMES);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> selected);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        logger.note(
            "Confirm selected motor stopped and unselected motor continued, then press START.",
        );
        let stopped = wait_for_start(OPERATOR_FRAMES);
        pad::control_motor(unselected, pad::PAD_MOTOR_STOP_HARD);
        end(
            logger,
            &mut summary,
            "unselected-controller-and-rumble",
            started && stopped && completed,
        );
    } else {
        logger.record(
            "unselected-controller-and-rumble requires two controllers",
            false,
        );
    }

    begin(logger, "wired-x-patch-hardware");
    logger.note("Hold the main stick far right on the first connected controller and press START.");
    let confirmed = wait_for_start(OPERATOR_FRAMES);
    let first = connected.leading_zeros() as i32;
    let completed = if first < 4 {
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> first);
        wait_sync(logger, &mut summary, SYNC_FRAMES)
    } else {
        false
    };
    end(
        logger,
        &mut summary,
        "wired-x-patch-hardware",
        confirmed && completed && pad::pad_debug_origin_byte(first, 2) == 0,
    );

    if first < 4 {
        begin(logger, "unplug-during-type-query");
        logger.note("Start removing the first controller now, then press START as soon as the plug leaves the socket.");
        let unplugged = wait_for_start(OPERATOR_FRAMES);
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> first);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::read(&mut statuses);
        let no_controller = statuses[first as usize].err as i32 == pad::PAD_ERR_NO_CONTROLLER;
        end(
            logger,
            &mut summary,
            "unplug-during-type-query",
            unplugged && completed && no_controller,
        );

        begin(logger, "wired-empty-socket");
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> first);
        let empty_completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::read(&mut statuses);
        end(
            logger,
            &mut summary,
            "wired-empty-socket",
            empty_completed && statuses[first as usize].err as i32 == pad::PAD_ERR_NO_CONTROLLER,
        );

        begin(logger, "unplug-during-calibration-transfer");
        logger.note("Reconnect the controller, press START, then remove it immediately when the recalibration request starts.");
        let reconnected = wait_for_start(OPERATOR_FRAMES);
        let _ = pad::reset(pad::PAD_CHAN0_BIT >> first);
        let ready = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let command_before = summary.command_42;
        let _ = pad::recalibrate(pad::PAD_CHAN0_BIT >> first);
        let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
        let _ = pad::read(&mut statuses);
        let bounded = matches!(
            statuses[first as usize].err as i32,
            pad::PAD_ERR_NONE | pad::PAD_ERR_NO_CONTROLLER
        );
        let calibration_submitted = summary.command_42 > command_before;
        end(
            logger,
            &mut summary,
            "unplug-during-calibration-transfer",
            reconnected && ready && completed && bounded && calibration_submitted,
        );
    }

    finish_pad_trace(logger, &mut summary);
}

pub fn run_wavebird(logger: &mut ProbeLogger) {
    logger.section("PAD guided WaveBird probe");
    logger.note("Insert a WaveBird receiver. START confirms link changes.");
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    install_pad_trace(logger);
    let _ = pad::init();
    let _ = wait_sync(logger, &mut summary, SYNC_FRAMES);

    let mut receiver_channel = None;
    for channel in 0..4 {
        let type_ = si::get_type(channel);
        logger.note(&format!("channel={channel} si_type=0x{type_:08x}"));
        if type_ & si::SI_GC_WIRELESS != 0 {
            receiver_channel = Some(channel);
            break;
        }
    }
    logger.record("WaveBird receiver detected", receiver_channel.is_some());
    let Some(channel) = receiver_channel else {
        finish_pad_trace(logger, &mut summary);
        return;
    };
    let mask = pad::PAD_CHAN0_BIT >> channel;

    begin(logger, "wavebird-connected-command-path");
    let before_41 = summary.command_41;
    let before_42 = summary.command_42;
    let _ = pad::recalibrate(mask);
    let completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    let connected_path_ok =
        completed && summary.command_41 > before_41 && summary.command_42 == before_42;
    end(
        logger,
        &mut summary,
        "wavebird-connected-command-path",
        connected_path_ok,
    );

    begin(logger, "wavebird-receiver-only-wait");
    logger.note("Turn off the WaveBird controller and press START.");
    let disconnected = wait_for_start(OPERATOR_FRAMES);
    let _ = pad::recalibrate(mask);
    let waiting = {
        let state = pad::pad_debug_state();
        state.waiting_bits & mask != 0 || state.checking_bits & mask != 0
    };
    let waiting_path_ok = disconnected && waiting && summary.command_4d != 0;
    end(
        logger,
        &mut summary,
        "wavebird-receiver-only-wait",
        waiting_path_ok,
    );

    begin(logger, "wavebird-disconnect-reconnect");
    logger.note("Turn on the linked WaveBird and press START.");
    let reconnected = wait_for_start(OPERATOR_FRAMES);
    let mut statuses = [pad::PadStatus::default(); 4];
    let mut recovered = false;
    for _ in 0..SYNC_FRAMES {
        let _ = pad::read(&mut statuses);
        drain_trace(logger, &mut summary);
        let state = pad::pad_debug_state();
        if statuses[channel as usize].err == 0
            && state.waiting_bits & mask == 0
            && state.checking_bits & mask == 0
        {
            recovered = true;
            break;
        }
        probe_support::wait_vsync();
    }
    end(
        logger,
        &mut summary,
        "wavebird-disconnect-reconnect",
        reconnected && recovered,
    );
    finish_pad_trace(logger, &mut summary);
}

unsafe extern "C" fn stress_thread(argument: *mut c_void) -> *mut c_void {
    let channel = argument as usize;
    let mask = pad::PAD_CHAN0_BIT >> channel;
    while !STRESS_STOP.load(Ordering::Acquire) {
        match STRESS_OPERATIONS.fetch_add(1, Ordering::AcqRel) % 4 {
            0 => {
                let _ = pad::recalibrate(mask);
            }
            1 => {
                let _ = pad::reset(mask);
            }
            2 => {
                let _ = pad::scan_pads();
            }
            _ => {
                let mut statuses = [pad::PadStatus::default(); 4];
                let _ = pad::read(&mut statuses);
            }
        }
        lwp::yield_now();
    }
    core::ptr::null_mut()
}

pub fn run_stress(logger: &mut ProbeLogger) {
    logger.section("PAD LWP stress and initialization interaction probe");
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    TRACE_VERBOSE.store(false, Ordering::Release);
    install_pad_trace(logger);
    LWP_HOOK_CALLS.store(0, Ordering::Release);
    lwp_sys::set_debug_hook(Some(lwp_trace_hook));
    logger.record("LWP debug hook installed", true);
    let initialization_steps = [
        type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_HOLD),
        type_step(0, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(1, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(2, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
        type_step(3, si::SI_ERROR_NO_RESPONSE, si::SI_FAULT_COMPLETE),
    ];
    si::si_fault_install(&initialization_steps);
    let _ = pad::init();
    let discovery_was_active = pad::sync() == 0;
    let initialization_accepted = pad::recalibrate(pad::PAD_VALID_MASK) == 1;
    let initialization_released = si::si_fault_release(0) == 1;
    logger.record(
        "recalibration accepted during discovery",
        discovery_was_active && initialization_accepted && initialization_released,
    );
    let initialization_completed = wait_sync(logger, &mut summary, SYNC_FRAMES);
    let initialization_report = si::si_fault_report();
    logger.record(
        "initialization interaction consumed its SI script",
        initialization_completed
            && initialization_report.installed == initialization_report.consumed
            && initialization_report.dropped_steps == 0
            && initialization_report.held_operations == 0,
    );
    si::si_fault_clear();
    let _ = pad::reset(pad::PAD_VALID_MASK);
    let _ = wait_sync(logger, &mut summary, SYNC_FRAMES);

    STRESS_STOP.store(false, Ordering::Release);
    STRESS_OPERATIONS.store(0, Ordering::Release);
    let mut raw_threads = [lwp_sys::LWP_THREAD_NULL; 4];
    let mut handles: [Option<ThreadHandle>; 4] = [None; 4];
    for channel in 0..4 {
        let stack = unsafe { core::ptr::addr_of_mut!(STRESS_STACKS[channel].0) };
        handles[channel] = unsafe {
            ThreadHandle::create(
                &mut raw_threads[channel],
                Some(stress_thread),
                channel as *mut c_void,
                stack.cast(),
                16 * 1024,
                64,
            )
            .ok()
        };
    }
    logger.record(
        "four PAD stress threads created",
        handles.iter().all(Option::is_some),
    );

    let target_frames = if cfg!(feature = "dolphin-automation") {
        600
    } else {
        108_000
    };
    let target_operations = if cfg!(feature = "dolphin-automation") {
        1_000
    } else {
        10_000
    };
    for frame in 0..target_frames {
        drain_trace(logger, &mut summary);
        let _ = drain_lwp_trace();
        if frame % 600 == 0 {
            logger.note_u32("stress frame", frame);
            logger.note_u32(
                "stress operations",
                STRESS_OPERATIONS.load(Ordering::Acquire),
            );
            let _ = logger.checkpoint("stress-progress");
        }
        probe_support::wait_vsync();
    }
    while STRESS_OPERATIONS.load(Ordering::Acquire) < target_operations {
        drain_trace(logger, &mut summary);
        let _ = drain_lwp_trace();
        lwp::yield_now();
    }
    STRESS_STOP.store(true, Ordering::Release);
    let joined = handles
        .into_iter()
        .flatten()
        .all(|handle| handle.join().is_ok());
    let synchronized = wait_sync(logger, &mut summary, SYNC_FRAMES);
    logger.note_u32(
        "stress operations final",
        STRESS_OPERATIONS.load(Ordering::Acquire),
    );
    let _ = drain_lwp_trace();
    let lwp_counters = lwp_sys::debug_counters();
    logger.note_u32("LWP hook calls", LWP_HOOK_CALLS.load(Ordering::Acquire));
    logger.note_u64(
        "LWP dropped debug events",
        lwp_counters.dropped_debug_events,
    );
    logger.record(
        "LWP trace hook received events without drops",
        LWP_HOOK_CALLS.load(Ordering::Acquire) != 0 && lwp_counters.dropped_debug_events == 0,
    );
    logger.record(
        "stress completed without stale PAD state",
        joined && synchronized && state_clean(),
    );
    lwp_sys::set_debug_hook(None);
    logger.record("LWP debug hook cleared", true);
    finish_pad_trace(logger, &mut summary);
    TRACE_VERBOSE.store(true, Ordering::Release);
}

static RESET_EARLY_CALLS: AtomicU32 = AtomicU32::new(0);
static RESET_LATE_CALLS: AtomicU32 = AtomicU32::new(0);

unsafe extern "C" fn reset_early(final_: i32) -> i32 {
    let calls = RESET_EARLY_CALLS.fetch_add(1, Ordering::AcqRel);
    if final_ == 0 && calls == 0 { 0 } else { 1 }
}

unsafe extern "C" fn reset_late(_final_: i32) -> i32 {
    RESET_LATE_CALLS.fetch_add(1, Ordering::AcqRel);
    1
}

static mut RESET_EARLY_INFO: SysResetInfo = SysResetInfo {
    node: LwpNode {
        next: core::ptr::null_mut(),
        prev: core::ptr::null_mut(),
    },
    func: Some(reset_early),
    prio: 10,
};

static mut RESET_LATE_INFO: SysResetInfo = SysResetInfo {
    node: LwpNode {
        next: core::ptr::null_mut(),
        prev: core::ptr::null_mut(),
    },
    func: Some(reset_late),
    prio: 200,
};

pub fn run_reset(logger: &mut ProbeLogger) {
    logger.section("PAD system-reset callback probe");
    if !persistent_log_ready(logger) {
        logger.record("mounted probe log is required", false);
        return;
    }
    let mut summary = TraceSummary::default();
    install_pad_trace(logger);
    RESET_HOOK_CALLS.store(0, Ordering::Release);
    let previous = system::sys::set_reset_debug_hook(Some(reset_trace_hook));
    logger.record("system reset debug hook installed", previous.is_none());
    let _ = pad::init();
    let _ = wait_sync(logger, &mut summary, SYNC_FRAMES);

    RESET_EARLY_CALLS.store(0, Ordering::Release);
    RESET_LATE_CALLS.store(0, Ordering::Release);
    unsafe {
        system::register_reset_func(core::ptr::addr_of_mut!(RESET_EARLY_INFO));
        system::register_reset_func(core::ptr::addr_of_mut!(RESET_LATE_INFO));
    }
    let mut passes = 0;
    while passes < SYNC_FRAMES {
        passes += 1;
        if system::sys::sys_run_reset_functions(0) != 0 {
            break;
        }
        drain_trace(logger, &mut summary);
        probe_support::wait_vsync();
    }
    let final_ready = system::sys::sys_run_reset_functions(1) != 0;
    drain_trace(logger, &mut summary);
    let callbacks_before_second_cycle = summary.reset_callbacks;
    let second_cycle_first_ready = system::sys::sys_run_reset_functions(0) != 0;
    let mut second_cycle_completed = false;
    for _ in 0..SYNC_FRAMES {
        drain_trace(logger, &mut summary);
        if system::sys::sys_run_reset_functions(0) != 0 {
            second_cycle_completed = true;
            break;
        }
        probe_support::wait_vsync();
    }
    let second_final_ready = system::sys::sys_run_reset_functions(1) != 0;
    drain_trace(logger, &mut summary);
    let one_shot_cleared = !second_cycle_first_ready
        && second_cycle_completed
        && second_final_ready
        && summary.reset_callbacks > callbacks_before_second_cycle;
    let early = RESET_EARLY_CALLS.load(Ordering::Acquire);
    let late = RESET_LATE_CALLS.load(Ordering::Acquire);
    logger.note_u32("reset prepare passes", passes);
    logger.note_u32("early callback calls", early);
    logger.note_u32("late callback calls", late);
    logger.note_u32("PAD reset callback traces", summary.reset_callbacks);
    logger.note_u32(
        "system reset hook calls",
        RESET_HOOK_CALLS.load(Ordering::Acquire),
    );
    logger.record(
        "reset queue retried and finalized PAD",
        passes >= 2
            && final_ready
            && early >= 3
            && late >= 3
            && summary.reset_callbacks >= 3
            && RESET_HOOK_CALLS.load(Ordering::Acquire) != 0
            && one_shot_cleared
            && state_clean(),
    );
    unsafe {
        system::unregister_reset_func(core::ptr::addr_of_mut!(RESET_EARLY_INFO));
        system::unregister_reset_func(core::ptr::addr_of_mut!(RESET_LATE_INFO));
    }
    let restored = system::sys::set_reset_debug_hook(None);
    logger.record("system reset debug hook restored", restored.is_some());
    finish_pad_trace(logger, &mut summary);

    logger.note("Press START to run the terminal SYS_RESTART smoke test.");
    if wait_for_start(OPERATOR_FRAMES) {
        logger.record("terminal reset operator confirmation", true);
        let checkpointed = logger.checkpoint("before-terminal-reset");
        logger.record("terminal reset log checkpoint", checkpointed);
        logger.summary();
        let _ = logger.checkpoint("terminal-reset-summary");
        unsafe { system::reset_system(system::SYS_RESTART as i32, 0, 0) };
    } else {
        logger.record("terminal reset operator confirmation", false);
    }
}
