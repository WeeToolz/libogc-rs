use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_controller::{
    ControllerId, ControllerSource, ControllerState, InputTraceEvent, MAX_CONTROLLERS, TraceError,
};
use ogc_system_probe::event_line::{EventLineBuf, source_slots, source_text};
use ogc_system_probe::probe_support;

use crate::controller_input::Input;
#[path = "action_event.rs"]
mod action_event;

static TRACE_FAILED: AtomicBool = AtomicBool::new(false);
static FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);
static mut LAST_STATES: [Option<(Option<ControllerId>, ControllerState)>; MAX_CONTROLLERS] =
    [None; MAX_CONTROLLERS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionTraceError {
    Format,
    CaptureFull,
    CaptureBusy,
    StateCapacity,
}

/// Record an application action after its input branch changes game state.
pub fn record_action<F: core::fmt::Display>(
    game: &'static str,
    screen: &'static str,
    action: &'static str,
    frame: F,
    input: Input,
) -> Result<(), ActionTraceError> {
    let mut line = EventLineBuf::new();
    let source = input.source.map(|identity| identity.source);
    let (slot_a, slot_b) = source.map(source_slots).unwrap_or((u8::MAX, u8::MAX));
    action_event::write_action_event(
        &mut line,
        action_event::ActionEvent {
            game,
            screen,
            action,
            frame,
        },
        input,
        (source.map(source_text).unwrap_or("unknown"), slot_a, slot_b),
        EventLineBuf::write_json_string,
    )
    .map_err(|_| fail(ActionTraceError::Format))?;
    emit_line(&mut line)
}

/// Synchronous manager callback. It formats and appends only; it does no I/O.
pub fn observe_input(event: &InputTraceEvent<'_>) -> Result<(), TraceError> {
    let result = match event {
        InputTraceEvent::RawReport {
            source,
            generation,
            usb_identity,
            timestamp_ticks,
            bytes,
        } => write_raw_report(*source, *generation, *usb_identity, *timestamp_ticks, bytes),
        InputTraceEvent::Decoded {
            timestamp_ticks,
            id,
            state,
        } => write_decoded(*timestamp_ticks, *id, *state),
        InputTraceEvent::Reset {
            timestamp_ticks,
            reset,
        } => write_reset(*timestamp_ticks, reset.id, reset.reason, reset.state),
        InputTraceEvent::Error {
            timestamp_ticks,
            error,
        } => write_error(
            *timestamp_ticks,
            error.source,
            error.operation,
            error.failure,
            error.cleanup_code,
        ),
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            TRACE_FAILED.store(true, Ordering::Release);
            Err(TraceError(match error {
                ActionTraceError::Format => 1,
                ActionTraceError::CaptureFull => 2,
                ActionTraceError::CaptureBusy => 3,
                ActionTraceError::StateCapacity => 4,
            }))
        }
    }
}

pub fn check_failed() {
    if TRACE_FAILED.load(Ordering::Acquire)
        && FAILURE_REPORTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        ogc_system::report(format_args!(
            "ACTION TRACE FAILED: input or action record was lost\n"
        ));
    }
}

fn write_raw_report(
    source: ControllerSource,
    generation: Option<u32>,
    identity: Option<ogc_controller::UsbTraceIdentity>,
    timestamp_ticks: Option<u64>,
    bytes: &[u8],
) -> Result<(), ActionTraceError> {
    let mut line = EventLineBuf::new();
    let (slot_a, slot_b) = source_slots(source);
    write!(
        &mut line,
        "{{\"type\":\"raw_report\",\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"generation\":{},\"generation_valid\":{},\"timestamp_ticks\":{},\"timestamp_ticks_valid\":{},\"device_id\":{},\"vid\":{},\"pid\":{},\"usb_identity_valid\":{},\"data\":\"",
        source_text(source),
        slot_a,
        slot_b,
        generation.unwrap_or(0),
        generation.is_some(),
        timestamp_ticks.unwrap_or(0),
        timestamp_ticks.is_some(),
        identity.map(|value| value.device_id).unwrap_or(0),
        identity.map(|value| value.vid).unwrap_or(0),
        identity.map(|value| value.pid).unwrap_or(0),
        identity.is_some(),
    )
    .map_err(|_| ActionTraceError::Format)?;
    line.write_hex_bytes(bytes)
        .map_err(|_| ActionTraceError::Format)?;
    line.write_str("\"}")
        .map_err(|_| ActionTraceError::Format)?;
    emit_line(&mut line)
}

fn write_decoded(
    timestamp_ticks: Option<u64>,
    id: Option<ControllerId>,
    state: ControllerState,
) -> Result<(), ActionTraceError> {
    match store_changed_state(id, state) {
        Ok(false) => return Ok(()),
        Ok(true) => {}
        Err(error) => return Err(fail(error)),
    }
    let mut line = EventLineBuf::new();
    let (slot_a, slot_b) = source_slots(state.source);
    write!(
        &mut line,
        "{{\"type\":\"decoded\",\"timestamp_ticks\":{},\"timestamp_ticks_valid\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"generation\":{},\"generation_valid\":{},\"identity_valid\":{},\"kind\":\"{:?}\",\"connected\":{},\"buttons\":{},\"down\":{},\"up\":{},\"left_x\":{},\"left_y\":{},\"right_x\":{},\"right_y\":{},\"left_trigger\":{},\"right_trigger\":{}}}",
        timestamp_ticks.unwrap_or(0),
        timestamp_ticks.is_some(),
        source_text(state.source),
        slot_a,
        slot_b,
        id.map(|value| value.generation).unwrap_or(0),
        id.is_some(),
        id.is_some(),
        state.kind,
        state.connected,
        state.buttons.bits(),
        state.buttons_down.bits(),
        state.buttons_up.bits(),
        state.axes.left_x,
        state.axes.left_y,
        state.axes.right_x,
        state.axes.right_y,
        state.axes.left_trigger,
        state.axes.right_trigger,
    )
    .map_err(|_| ActionTraceError::Format)?;
    emit_line(&mut line)
}

fn write_reset(
    timestamp_ticks: Option<u64>,
    identity: ControllerId,
    reason: ogc_controller::ResetReason,
    state: ControllerState,
) -> Result<(), ActionTraceError> {
    clear_state(identity.source);
    let mut line = EventLineBuf::new();
    let (slot_a, slot_b) = source_slots(identity.source);
    write!(
        &mut line,
        "{{\"type\":\"reset\",\"timestamp_ticks\":{},\"timestamp_ticks_valid\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"generation\":{},\"generation_valid\":true,\"identity_valid\":true,\"reason\":\"{:?}\",\"released\":{}}}",
        timestamp_ticks.unwrap_or(0),
        timestamp_ticks.is_some(),
        source_text(identity.source),
        slot_a,
        slot_b,
        identity.generation,
        reason,
        state.buttons_up.bits(),
    )
    .map_err(|_| ActionTraceError::Format)?;
    emit_line(&mut line)
}

fn write_error(
    timestamp_ticks: Option<u64>,
    source: ControllerSource,
    operation: &'static str,
    failure: ogc_controller::InputFailure,
    cleanup_code: Option<i32>,
) -> Result<(), ActionTraceError> {
    let mut line = EventLineBuf::new();
    let (slot_a, slot_b) = source_slots(source);
    write!(
        &mut line,
        "{{\"type\":\"error\",\"timestamp_ticks\":{},\"timestamp_ticks_valid\":{},\"source\":\"{}\",\"slot_a\":{},\"slot_b\":{},\"operation\":",
        timestamp_ticks.unwrap_or(0),
        timestamp_ticks.is_some(),
        source_text(source),
        slot_a,
        slot_b,
    )
    .map_err(|_| ActionTraceError::Format)?;
    line.write_json_string(operation)
        .map_err(|_| ActionTraceError::Format)?;
    write!(
        &mut line,
        ",\"identity_valid\":false,\"failure\":\"{:?}\",\"cleanup_code\":{},\"cleanup_code_valid\":{}}}",
        failure,
        cleanup_code.unwrap_or(0),
        cleanup_code.is_some(),
    )
    .map_err(|_| ActionTraceError::Format)?;
    emit_line(&mut line)
}

fn emit_line(line: &mut EventLineBuf) -> Result<(), ActionTraceError> {
    line.write_str("\n")
        .map_err(|_| fail(ActionTraceError::Format))?;
    let record = line.as_str().map_err(|_| fail(ActionTraceError::Format))?;
    probe_support::agent_log_checked(record.as_bytes()).map_err(|error| {
        fail(match error {
            probe_support::AgentLogError::CaptureFull => ActionTraceError::CaptureFull,
            probe_support::AgentLogError::CaptureBusy => ActionTraceError::CaptureBusy,
        })
    })
}

fn fail(error: ActionTraceError) -> ActionTraceError {
    TRACE_FAILED.store(true, Ordering::Release);
    error
}

fn store_changed_state(
    id: Option<ControllerId>,
    state: ControllerState,
) -> Result<bool, ActionTraceError> {
    // SAFETY: the manager calls this observer synchronously from the one app
    // thread that owns the controller manager.
    unsafe {
        let states = &mut *core::ptr::addr_of_mut!(LAST_STATES);
        if let Some(previous) = states
            .iter_mut()
            .find(|previous| previous.is_some_and(|(_, old)| old.source == state.source))
        {
            if *previous == Some((id, state)) {
                return Ok(false);
            }
            *previous = Some((id, state));
            return Ok(true);
        }
        if let Some(empty) = states.iter_mut().find(|entry| entry.is_none()) {
            *empty = Some((id, state));
            Ok(true)
        } else {
            Err(ActionTraceError::StateCapacity)
        }
    }
}

fn clear_state(source: ControllerSource) {
    // SAFETY: see `store_changed_state`.
    unsafe {
        let states = &mut *core::ptr::addr_of_mut!(LAST_STATES);
        if let Some(entry) = states
            .iter_mut()
            .find(|entry| entry.is_some_and(|(_, state)| state.source == source))
        {
            *entry = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_state_filter_keeps_edges_and_ignores_unchanged_samples() {
        let state = ControllerState {
            source: ControllerSource::GameCube(0),
            kind: ogc_controller::ControllerKind::GameCube,
            connected: true,
            buttons: ogc_controller::Buttons::A,
            buttons_down: ogc_controller::Buttons::A,
            buttons_up: ogc_controller::Buttons::empty(),
            axes: ogc_controller::Axes::default(),
        };
        clear_state(state.source);
        assert_eq!(store_changed_state(None, state), Ok(true));
        assert_eq!(store_changed_state(None, state), Ok(false));
        let held = ControllerState {
            buttons_down: ogc_controller::Buttons::empty(),
            ..state
        };
        assert_eq!(store_changed_state(None, held), Ok(true));
        clear_state(state.source);
        assert_eq!(store_changed_state(None, state), Ok(true));
    }

    #[test]
    fn trace_failure_is_sticky() {
        TRACE_FAILED.store(false, Ordering::Release);
        let error = fail(ActionTraceError::CaptureBusy);
        assert_eq!(error, ActionTraceError::CaptureBusy);
        assert!(TRACE_FAILED.load(Ordering::Acquire));
        TRACE_FAILED.store(true, Ordering::Release);
        assert!(TRACE_FAILED.load(Ordering::Acquire));
    }
}
