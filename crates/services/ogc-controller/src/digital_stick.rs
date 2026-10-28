//! Portable analog stick to digital direction mapping.
//!
//! The mapper uses the production `InputTransitions` connection gate and edge
//! accumulator. It does not identify sources or read hardware. Callers supply
//! each sample's physical `ControllerId` and threshold.

use crate::{
    Axes, Buttons, ControllerId, ControllerSource, ControllerState, InputTransitions,
    MAX_BLUETOOTH_HID_CONTROLLERS, MAX_GAMECUBE_CONTROLLERS, MAX_USB_CONTROLLERS,
    MAX_WPAD_CONTROLLERS, ResetReason, TransitionError,
};

/// One identified normalized stick sample. Positive Y means up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StickDirectionSample {
    pub id: ControllerId,
    pub x: i16,
    pub y: i16,
    pub threshold: i16,
}

/// Direction flags for a projected stick sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StickDirections(u8);

impl StickDirections {
    pub const EMPTY: Self = Self(0);
    pub const LEFT: Self = Self(1 << 0);
    pub const RIGHT: Self = Self(1 << 1);
    pub const DOWN: Self = Self(1 << 2);
    pub const UP: Self = Self(1 << 3);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, direction: Self) -> bool {
        self.0 & direction.0 == direction.0
    }

    /// Convert these directions to the matching controller button mask.
    pub const fn to_buttons(self) -> Buttons {
        let mut buttons = 0;
        if self.contains(Self::LEFT) {
            buttons |= Buttons::LEFT.bits();
        }
        if self.contains(Self::RIGHT) {
            buttons |= Buttons::RIGHT.bits();
        }
        if self.contains(Self::DOWN) {
            buttons |= Buttons::DOWN.bits();
        }
        if self.contains(Self::UP) {
            buttons |= Buttons::UP.bits();
        }
        Buttons::from_bits(buttons)
    }

    const fn from_buttons(buttons: Buttons) -> Self {
        let mut bits = 0;
        if buttons.contains(Buttons::LEFT) {
            bits |= Self::LEFT.0;
        }
        if buttons.contains(Buttons::RIGHT) {
            bits |= Self::RIGHT.0;
        }
        if buttons.contains(Buttons::DOWN) {
            bits |= Self::DOWN.0;
        }
        if buttons.contains(Buttons::UP) {
            bits |= Self::UP.0;
        }
        Self(bits)
    }
}

/// Project normalized stick axes to directions with strict dead-zone edges.
///
/// A threshold from zero through 32766 is valid. Values equal to either
/// threshold remain neutral. Widened comparisons keep `i16::MIN` safe.
pub fn project_stick_directions(
    x: i16,
    y: i16,
    threshold: i16,
) -> Result<StickDirections, StickDirectionError> {
    if !valid_threshold(threshold) {
        return Err(StickDirectionError::InvalidThreshold(threshold));
    }
    let threshold = i32::from(threshold);
    let x = i32::from(x);
    let y = i32::from(y);
    let mut directions = StickDirections::EMPTY;
    if x < -threshold {
        directions = directions | StickDirections::LEFT;
    }
    if x > threshold {
        directions = directions | StickDirections::RIGHT;
    }
    if y < -threshold {
        directions = directions | StickDirections::DOWN;
    }
    if y > threshold {
        directions = directions | StickDirections::UP;
    }
    Ok(directions)
}

const fn valid_threshold(threshold: i16) -> bool {
    let threshold = threshold as i32;
    threshold >= 0 && threshold <= 32766
}

impl core::ops::BitOr for StickDirections {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Held directions and accumulated edges for one sampled physical source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StickDirectionState {
    pub id: ControllerId,
    /// Physical directions held by the current sample, including on connect.
    pub held: StickDirections,
    pub down: StickDirections,
    pub up: StickDirections,
    /// Directions available to continuous app actions after the stick has
    /// returned to neutral once for this controller identity.
    pub action_held: StickDirections,
    /// New directions available to app actions after the neutral rearm point.
    pub action_down: StickDirections,
    /// Action directions released after the neutral rearm point.
    pub action_up: StickDirections,
}

/// Release observation for a removed or replaced source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StickDirectionReset {
    pub id: ControllerId,
    pub reason: ResetReason,
    pub released: StickDirections,
    /// Directions released from an action that was active before reset.
    pub action_released: StickDirections,
}

/// Fixed-capacity result. Active states follow the input sample order. Resets
/// retain their old source IDs and are returned separately from new states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StickDirectionFrame<const N: usize> {
    states: [Option<StickDirectionState>; N],
    state_count: usize,
    resets: [Option<StickDirectionReset>; N],
    reset_count: usize,
}

impl<const N: usize> StickDirectionFrame<N> {
    const fn empty() -> Self {
        Self {
            states: [None; N],
            state_count: 0,
            resets: [None; N],
            reset_count: 0,
        }
    }

    pub fn states(&self) -> impl Iterator<Item = &StickDirectionState> {
        self.states[..self.state_count].iter().flatten()
    }

    pub fn resets(&self) -> impl Iterator<Item = &StickDirectionReset> {
        self.resets[..self.reset_count].iter().flatten()
    }
}

/// Input validation or transition failure. Validation and transition errors
/// leave the mapper's connection and edge state unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StickDirectionError {
    Capacity { samples: usize, capacity: usize },
    InvalidSource(ControllerSource),
    InvalidIdentity(ControllerId),
    InvalidThreshold(i16),
    DuplicateSource(ControllerSource),
    Transition(TransitionError),
}

/// Stateful, allocation-free stick projection for up to `N` physical sources.
#[derive(Clone)]
pub struct DigitalStickMapper<const N: usize> {
    ids: [Option<ControllerId>; N],
    action_armed: [bool; N],
    transitions: InputTransitions<N>,
}

impl<const N: usize> Default for DigitalStickMapper<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> DigitalStickMapper<N> {
    pub const fn new() -> Self {
        Self {
            ids: [None; N],
            action_armed: [false; N],
            transitions: InputTransitions::new(),
        }
    }

    /// Map samples by physical identity. Input order only controls output order.
    /// Threshold checks use strict comparisons: an axis equal to the threshold
    /// remains neutral on that axis. State changes commit only after every
    /// transition succeeds.
    pub fn map(
        &mut self,
        samples: &[StickDirectionSample],
    ) -> Result<StickDirectionFrame<N>, StickDirectionError> {
        self.validate(samples)?;
        let mut staged = self.clone();
        let frame = staged.map_validated(samples)?;
        *self = staged;
        Ok(frame)
    }

    fn map_validated(
        &mut self,
        samples: &[StickDirectionSample],
    ) -> Result<StickDirectionFrame<N>, StickDirectionError> {
        let mut frame = StickDirectionFrame::empty();
        for slot in 0..N {
            let Some(previous) = self.ids[slot] else {
                continue;
            };
            let current = samples
                .iter()
                .find(|sample| sample.id.source == previous.source);
            if current.is_some_and(|sample| sample.id == previous) {
                continue;
            }
            let reason = if current.is_some() {
                ResetReason::Replaced
            } else {
                ResetReason::Disconnected
            };
            self.transitions
                .reset(slot, reason)
                .map_err(StickDirectionError::Transition)?;
            let reset =
                self.transitions
                    .take_reset(slot)
                    .ok_or(StickDirectionError::Transition(
                        TransitionError::ResetPending,
                    ))?;
            let released = StickDirections::from_buttons(reset.state.buttons_up);
            frame.resets[frame.reset_count] = Some(StickDirectionReset {
                id: previous,
                reason,
                released,
                action_released: if self.action_armed[slot] {
                    released
                } else {
                    StickDirections::EMPTY
                },
            });
            frame.reset_count += 1;
            self.ids[slot] = None;
            self.action_armed[slot] = false;
        }

        for sample in samples {
            let slot = self
                .ids
                .iter()
                .position(|id| *id == Some(sample.id))
                .or_else(|| self.ids.iter().position(Option::is_none))
                .ok_or(StickDirectionError::Capacity {
                    samples: samples.len(),
                    capacity: N,
                })?;
            let directions = project_stick_directions(sample.x, sample.y, sample.threshold)?;
            let controller = ControllerState {
                source: sample.id.source,
                connected: true,
                buttons: directions.to_buttons(),
                axes: Axes {
                    left_x: sample.x,
                    left_y: sample.y,
                    ..Axes::default()
                },
                ..ControllerState::empty()
            };
            self.transitions
                .sample(slot, controller)
                .map_err(StickDirectionError::Transition)?;
            let action_was_armed = self.ids[slot] == Some(sample.id) && self.action_armed[slot];
            if self.ids[slot] != Some(sample.id) {
                self.action_armed[slot] = directions == StickDirections::EMPTY;
            } else if !self.action_armed[slot] && directions == StickDirections::EMPTY {
                self.action_armed[slot] = true;
            }
            self.ids[slot] = Some(sample.id);
            let state = self
                .transitions
                .take(slot)
                .ok_or(StickDirectionError::Transition(
                    TransitionError::InvalidSlot,
                ))?;
            let held = StickDirections::from_buttons(state.buttons);
            let down = StickDirections::from_buttons(state.buttons_down);
            let action_armed = self.action_armed[slot];
            frame.states[frame.state_count] = Some(StickDirectionState {
                id: sample.id,
                held,
                down,
                up: StickDirections::from_buttons(state.buttons_up),
                action_held: if action_armed {
                    held
                } else {
                    StickDirections::EMPTY
                },
                action_down: if action_armed {
                    down
                } else {
                    StickDirections::EMPTY
                },
                action_up: if action_was_armed {
                    StickDirections::from_buttons(state.buttons_up)
                } else {
                    StickDirections::EMPTY
                },
            });
            frame.state_count += 1;
        }
        Ok(frame)
    }

    fn validate(&self, samples: &[StickDirectionSample]) -> Result<(), StickDirectionError> {
        if samples.len() > N {
            return Err(StickDirectionError::Capacity {
                samples: samples.len(),
                capacity: N,
            });
        }
        for (index, sample) in samples.iter().enumerate() {
            if !valid_source(sample.id.source) {
                return Err(StickDirectionError::InvalidSource(sample.id.source));
            }
            if sample.id.generation == 0 {
                return Err(StickDirectionError::InvalidIdentity(sample.id));
            }
            if !valid_threshold(sample.threshold) {
                return Err(StickDirectionError::InvalidThreshold(sample.threshold));
            }
            if samples[..index]
                .iter()
                .any(|prior| prior.id.source == sample.id.source)
            {
                return Err(StickDirectionError::DuplicateSource(sample.id.source));
            }
        }
        Ok(())
    }
}

const fn valid_source(source: ControllerSource) -> bool {
    match source {
        ControllerSource::Unknown => false,
        ControllerSource::GameCube(slot) => slot < MAX_GAMECUBE_CONTROLLERS as u8,
        ControllerSource::WiiBluetooth(slot) => slot < MAX_WPAD_CONTROLLERS as u8,
        ControllerSource::UsbHid(slot) => slot < MAX_USB_CONTROLLERS as u8,
        ControllerSource::UsbGameCube { adapter_slot, port } => {
            adapter_slot < MAX_USB_CONTROLLERS as u8 && port < MAX_GAMECUBE_CONTROLLERS as u8
        }
        ControllerSource::BluetoothHid(slot) => slot < MAX_BLUETOOTH_HID_CONTROLLERS as u8,
        ControllerSource::BluetoothHidPair(left, right) => {
            left < MAX_BLUETOOTH_HID_CONTROLLERS as u8
                && right < MAX_BLUETOOTH_HID_CONTROLLERS as u8
                && left != right
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(source: ControllerSource, generation: u32, x: i16, y: i16) -> StickDirectionSample {
        StickDirectionSample {
            id: ControllerId { source, generation },
            x,
            y,
            threshold: 8192,
        }
    }

    #[test]
    fn strict_thresholds_endpoints_and_diagonals_match_direction_contract() {
        let mut mapper = DigitalStickMapper::<1>::new();
        let id = ControllerSource::BluetoothHid(0);
        let frame = mapper.map(&[sample(id, 4, 8192, -8192)]).unwrap();
        assert_eq!(frame.states().next().unwrap().held, StickDirections::EMPTY);

        let frame = mapper.map(&[sample(id, 4, 32767, -32768)]).unwrap();
        assert_eq!(
            frame.states().next().unwrap().held,
            StickDirections::RIGHT | StickDirections::DOWN
        );
        assert_eq!(
            project_stick_directions(i16::MIN, i16::MAX, 32766),
            Ok(StickDirections::LEFT | StickDirections::UP)
        );
        assert_eq!(
            project_stick_directions(32766, -32766, 32766),
            Ok(StickDirections::EMPTY)
        );
        assert_eq!(
            project_stick_directions(i16::MIN, 0, 32767),
            Err(StickDirectionError::InvalidThreshold(32767))
        );
        assert_eq!(
            project_stick_directions(i16::MIN, 0, -1),
            Err(StickDirectionError::InvalidThreshold(-1))
        );

        let frame = mapper
            .map(&[StickDirectionSample {
                threshold: 9599,
                ..sample(id, 4, 9599, -9599)
            }])
            .unwrap();
        assert_eq!(frame.states().next().unwrap().held, StickDirections::EMPTY);
        let frame = mapper
            .map(&[StickDirectionSample {
                threshold: 9599,
                ..sample(id, 4, 9600, -9600)
            }])
            .unwrap();
        assert_eq!(
            frame.states().next().unwrap().held,
            StickDirections::RIGHT | StickDirections::DOWN
        );
    }

    #[test]
    fn direction_edges_hold_and_release_across_samples() {
        let source = ControllerSource::BluetoothHid(1);
        let mut mapper = DigitalStickMapper::<1>::new();
        let frame = mapper.map(&[sample(source, 1, 0, 0)]).unwrap();
        assert_eq!(frame.states().next().unwrap().down, StickDirections::EMPTY);

        let frame = mapper.map(&[sample(source, 1, 0, 9000)]).unwrap();
        assert_eq!(frame.states().next().unwrap().down, StickDirections::UP);
        let frame = mapper.map(&[sample(source, 1, 0, 10000)]).unwrap();
        assert_eq!(frame.states().next().unwrap().held, StickDirections::UP);
        assert_eq!(frame.states().next().unwrap().down, StickDirections::EMPTY);
        let frame = mapper.map(&[sample(source, 1, 0, 0)]).unwrap();
        assert_eq!(frame.states().next().unwrap().up, StickDirections::UP);
    }

    #[test]
    fn held_direction_is_gated_until_release_after_connect() {
        let source = ControllerSource::UsbGameCube {
            adapter_slot: 1,
            port: 2,
        };
        let mut mapper = DigitalStickMapper::<1>::new();
        let first = mapper.map(&[sample(source, 7, 10000, 0)]).unwrap();
        assert_eq!(first.states().next().unwrap().held, StickDirections::RIGHT);
        assert_eq!(first.states().next().unwrap().down, StickDirections::EMPTY);
        assert_eq!(
            first.states().next().unwrap().action_held,
            StickDirections::EMPTY
        );
        let changed_without_neutral = mapper.map(&[sample(source, 7, 0, 10000)]).unwrap();
        let state = changed_without_neutral.states().next().unwrap();
        assert_eq!(state.held, StickDirections::UP);
        assert_eq!(state.action_held, StickDirections::EMPTY);
        assert_eq!(state.action_down, StickDirections::EMPTY);
        assert_eq!(state.action_up, StickDirections::EMPTY);

        let neutral = mapper.map(&[sample(source, 7, 0, 0)]).unwrap();
        let state = neutral.states().next().unwrap();
        assert_eq!(state.up, StickDirections::UP);
        assert_eq!(state.action_held, StickDirections::EMPTY);
        assert_eq!(state.action_up, StickDirections::EMPTY);
        let armed = mapper.map(&[sample(source, 7, 10000, 0)]).unwrap();
        assert_eq!(
            armed.states().next().unwrap().action_held,
            StickDirections::RIGHT
        );
        assert_eq!(
            armed.states().next().unwrap().action_down,
            StickDirections::RIGHT
        );
        let released = mapper.map(&[sample(source, 7, 0, 0)]).unwrap();
        assert_eq!(
            released.states().next().unwrap().action_up,
            StickDirections::RIGHT
        );
    }

    #[test]
    fn removal_and_reconnect_emit_old_release_and_gate_new_generation() {
        let source = ControllerSource::BluetoothHidPair(1, 4);
        let mut mapper = DigitalStickMapper::<1>::new();
        mapper.map(&[sample(source, 3, -9000, 0)]).unwrap();
        let empty = mapper.map(&[]).unwrap();
        let reset = empty.resets().next().unwrap();
        assert_eq!(reset.id.generation, 3);
        assert_eq!(reset.released, StickDirections::LEFT);
        assert_eq!(reset.action_released, StickDirections::EMPTY);
        assert_eq!(reset.reason, ResetReason::Disconnected);

        let reconnected = mapper.map(&[sample(source, 4, -9000, 0)]).unwrap();
        assert_eq!(reconnected.resets().count(), 0);
        assert_eq!(
            reconnected.states().next().unwrap().held,
            StickDirections::LEFT
        );
        assert_eq!(
            reconnected.states().next().unwrap().down,
            StickDirections::EMPTY
        );
        assert_eq!(
            reconnected.states().next().unwrap().action_held,
            StickDirections::EMPTY
        );
    }

    #[test]
    fn replacement_emits_release_then_connection_gate_for_new_generation() {
        let source = ControllerSource::BluetoothHid(2);
        let mut mapper = DigitalStickMapper::<1>::new();
        mapper.map(&[sample(source, 2, 0, 9000)]).unwrap();
        let frame = mapper.map(&[sample(source, 3, 0, -9000)]).unwrap();
        let reset = frame.resets().next().unwrap();
        assert_eq!(reset.reason, ResetReason::Replaced);
        assert_eq!(reset.released, StickDirections::UP);
        assert_eq!(reset.action_released, StickDirections::EMPTY);
        let state = frame.states().next().unwrap();
        assert_eq!(state.id.generation, 3);
        assert_eq!(state.held, StickDirections::DOWN);
        assert_eq!(state.down, StickDirections::EMPTY);
        assert_eq!(state.action_held, StickDirections::EMPTY);
    }

    #[test]
    fn reordered_sources_keep_identity_and_edges() {
        let left = ControllerSource::BluetoothHid(0);
        let right = ControllerSource::BluetoothHid(1);
        let mut mapper = DigitalStickMapper::<2>::new();
        mapper
            .map(&[sample(left, 1, 0, 0), sample(right, 1, 0, 0)])
            .unwrap();
        mapper
            .map(&[sample(left, 1, 0, 9000), sample(right, 1, -9000, 0)])
            .unwrap();
        let frame = mapper
            .map(&[sample(right, 1, -10000, 0), sample(left, 1, 0, 10000)])
            .unwrap();
        let mut states = frame.states();
        let right_state = states.next().unwrap();
        let left_state = states.next().unwrap();
        assert_eq!(right_state.id.source, right);
        assert_eq!(right_state.down, StickDirections::EMPTY);
        assert_eq!(left_state.id.source, left);
        assert_eq!(left_state.down, StickDirections::EMPTY);
        assert_eq!(right_state.held, StickDirections::LEFT);
        assert_eq!(left_state.held, StickDirections::UP);
    }

    #[test]
    fn multiple_sources_have_independent_directions() {
        let mut mapper = DigitalStickMapper::<2>::new();
        let frame = mapper
            .map(&[
                sample(ControllerSource::GameCube(0), 1, 9000, 9000),
                sample(ControllerSource::WiiBluetooth(1), 2, -9000, -9000),
            ])
            .unwrap();
        let mut states = frame.states();
        assert_eq!(
            states.next().unwrap().held,
            StickDirections::RIGHT | StickDirections::UP
        );
        assert_eq!(
            states.next().unwrap().held,
            StickDirections::LEFT | StickDirections::DOWN
        );
    }

    #[test]
    fn invalid_inputs_fail_before_changing_mapper_state() {
        let source = ControllerSource::GameCube(0);
        let mut mapper = DigitalStickMapper::<2>::new();
        mapper.map(&[sample(source, 1, 0, 0)]).unwrap();
        mapper.map(&[sample(source, 1, 0, 9000)]).unwrap();

        let duplicate = [sample(source, 1, 0, 0), sample(source, 2, 0, 0)];
        assert_eq!(
            mapper.map(&duplicate).unwrap_err(),
            StickDirectionError::DuplicateSource(source)
        );
        let invalid_threshold = [StickDirectionSample {
            threshold: -1,
            ..sample(source, 1, 0, 9000)
        }];
        assert_eq!(
            mapper.map(&invalid_threshold).unwrap_err(),
            StickDirectionError::InvalidThreshold(-1)
        );
        let unknown = [sample(ControllerSource::Unknown, 1, 0, 0)];
        assert_eq!(
            mapper.map(&unknown).unwrap_err(),
            StickDirectionError::InvalidSource(ControllerSource::Unknown)
        );
        let invalid_slot = [sample(ControllerSource::GameCube(4), 1, 0, 0)];
        assert_eq!(
            mapper.map(&invalid_slot).unwrap_err(),
            StickDirectionError::InvalidSource(ControllerSource::GameCube(4))
        );
        for source in [
            ControllerSource::UsbGameCube {
                adapter_slot: 4,
                port: 0,
            },
            ControllerSource::UsbGameCube {
                adapter_slot: 0,
                port: 4,
            },
            ControllerSource::BluetoothHidPair(0, 0),
            ControllerSource::BluetoothHidPair(0, 6),
        ] {
            assert_eq!(
                mapper.map(&[sample(source, 1, 0, 0)]).unwrap_err(),
                StickDirectionError::InvalidSource(source)
            );
        }
        let invalid_identity = [sample(source, 0, 0, 0)];
        assert_eq!(
            mapper.map(&invalid_identity).unwrap_err(),
            StickDirectionError::InvalidIdentity(ControllerId {
                source,
                generation: 0
            })
        );
        let too_many = [sample(source, 1, 0, 9000); 3];
        assert_eq!(
            mapper.map(&too_many).unwrap_err(),
            StickDirectionError::Capacity {
                samples: 3,
                capacity: 2
            }
        );

        let unchanged = mapper.map(&[sample(source, 1, 0, 10000)]).unwrap();
        assert_eq!(unchanged.states().next().unwrap().held, StickDirections::UP);
        assert_eq!(
            unchanged.states().next().unwrap().down,
            StickDirections::EMPTY
        );
        assert_eq!(unchanged.resets().count(), 0);
    }

    #[test]
    fn generation_exhaustion_keeps_old_state_and_release_observable() {
        let source = ControllerSource::WiiBluetooth(0);
        let old_id = ControllerId {
            source,
            generation: 12,
        };
        let mut mapper = DigitalStickMapper::<1>::new();
        let old = StickDirectionSample {
            id: old_id,
            x: 0,
            y: 9000,
            threshold: 8192,
        };
        mapper.map(&[old]).unwrap();
        mapper.map(&[StickDirectionSample { y: 0, ..old }]).unwrap();
        mapper
            .map(&[StickDirectionSample { y: 9000, ..old }])
            .unwrap();
        mapper
            .transitions
            .set_generation_for_test(0, u32::MAX)
            .unwrap();

        let replacement = StickDirectionSample {
            id: ControllerId {
                generation: 13,
                ..old_id
            },
            x: -9000,
            y: 0,
            threshold: 8192,
        };
        assert_eq!(
            mapper.map(&[replacement]).unwrap_err(),
            StickDirectionError::Transition(TransitionError::GenerationExhausted)
        );

        let after_error = mapper.map(&[old]).unwrap();
        assert_eq!(after_error.resets().count(), 0);
        let state = after_error.states().next().unwrap();
        assert_eq!(state.id, old_id);
        assert_eq!(state.held, StickDirections::UP);
        assert_eq!(state.down, StickDirections::EMPTY);
        assert_eq!(state.up, StickDirections::EMPTY);
        assert_eq!(state.action_held, StickDirections::UP);
        assert_eq!(state.action_down, StickDirections::EMPTY);

        let removed = mapper.map(&[]).unwrap();
        let reset = removed.resets().next().unwrap();
        assert_eq!(reset.id, old_id);
        assert_eq!(reset.released, StickDirections::UP);
        assert_eq!(reset.action_released, StickDirections::UP);
    }

    #[test]
    fn direction_bits_and_button_projection_are_stable() {
        let directions = project_stick_directions(9000, -9000, 8192).unwrap();
        assert_eq!(directions.bits(), 0b0110);
        assert!(directions.contains(StickDirections::RIGHT));
        assert!(directions.contains(StickDirections::DOWN));
        assert!(!directions.contains(StickDirections::LEFT));
        assert_eq!(
            directions.to_buttons().bits(),
            Buttons::RIGHT.bits() | Buttons::DOWN.bits()
        );
    }
}
