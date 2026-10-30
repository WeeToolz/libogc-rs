//! Button transitions and reset events shared by hardware polling and host tests.
use crate::{Axes, Buttons, ControllerSource, ControllerState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerId {
    pub source: ControllerSource,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetReason {
    Disconnected,
    Replaced,
    InvalidReport,
    SourceError(i32),
    Timeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerReset {
    pub id: ControllerId,
    pub reason: ResetReason,
    /// Neutral state with releases for all buttons held before the reset.
    pub state: ControllerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionError {
    InvalidSlot,
    GenerationExhausted,
    ResetPending,
}

/// Raw hardware button masks after the same connection and edge gate used by
/// normalized controller input. The bit positions remain the source API's.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RawButtonFrame {
    pub held: u32,
    pub down: u32,
    pub up: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawButtonSample {
    pub source: crate::ControllerSource,
    pub connected: bool,
    pub held: u32,
    pub down: u32,
    pub up: u32,
}

/// Applies `InputTransitions` connection gating to source-specific button bits.
/// This preserves each bus's raw mask values while preventing a held button at
/// connect from producing an activation edge.
pub struct RawButtonTransitions<const N: usize> {
    transitions: InputTransitions<N>,
}

impl<const N: usize> Default for RawButtonTransitions<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RawButtonTransitions<N> {
    pub const fn new() -> Self {
        Self {
            transitions: InputTransitions::new(),
        }
    }

    pub fn sample(
        &mut self,
        index: usize,
        sample: RawButtonSample,
    ) -> Result<RawButtonFrame, TransitionError> {
        let state = crate::ControllerState {
            source: sample.source,
            connected: sample.connected,
            buttons: Buttons::from_bits(sample.held),
            buttons_down: Buttons::from_bits(sample.down),
            buttons_up: Buttons::from_bits(sample.up),
            ..crate::ControllerState::empty()
        };
        self.transitions.sample(index, state)?;
        if let Some(state) = self.transitions.take(index) {
            return Ok(RawButtonFrame {
                held: state.buttons.bits(),
                down: state.buttons_down.bits(),
                up: state.buttons_up.bits(),
            });
        }
        Ok(self
            .transitions
            .take_reset(index)
            .map(|reset| RawButtonFrame {
                held: 0,
                down: reset.state.buttons_down.bits(),
                up: reset.state.buttons_up.bits(),
            })
            .unwrap_or_default())
    }
}

#[derive(Clone, Copy)]
struct Slot {
    state: ControllerState,
    generation: u32,
    blocked: Buttons,
    down: Buttons,
    up: Buttons,
}

impl Slot {
    const EMPTY: Self = Self {
        state: ControllerState::empty(),
        generation: 0,
        blocked: Buttons::empty(),
        down: Buttons::empty(),
        up: Buttons::empty(),
    };
}

/// Accumulate every sample before taking one frame. Slots retain connection
/// generations; a reset must be consumed before a second reset in that slot.
#[derive(Clone)]
pub struct InputTransitions<const N: usize> {
    slots: [Slot; N],
    resets: [Option<ControllerReset>; N],
}

impl<const N: usize> Default for InputTransitions<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InputTransitions<N> {
    pub const fn new() -> Self {
        Self {
            slots: [Slot::EMPTY; N],
            resets: [None; N],
        }
    }

    #[cfg(test)]
    pub(crate) fn set_generation_for_test(
        &mut self,
        index: usize,
        generation: u32,
    ) -> Result<(), TransitionError> {
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(TransitionError::InvalidSlot)?;
        slot.generation = generation;
        Ok(())
    }

    pub fn sample(
        &mut self,
        index: usize,
        mut state: ControllerState,
    ) -> Result<ControllerId, TransitionError> {
        state.axes = state.axes.clamp_normalized();
        let slot = self.slots.get(index).ok_or(TransitionError::InvalidSlot)?;
        if !state.connected {
            let id = ControllerId {
                source: slot.state.source,
                generation: slot.generation,
            };
            self.reset(index, ResetReason::Disconnected)?;
            return Ok(id);
        }
        if slot.state.connected && slot.state.source != state.source {
            self.reset(index, ResetReason::Replaced)?;
        }
        let slot = &mut self.slots[index];
        if !slot.state.connected {
            slot.generation = slot
                .generation
                .checked_add(1)
                .ok_or(TransitionError::GenerationExhausted)?;
            // A button already held at connection cannot trigger an action.
            slot.blocked = state.buttons;
        }
        slot.blocked = slot.blocked & state.buttons;
        slot.down |= Buttons::from_bits(
            state.buttons.bits() & !slot.state.buttons.bits() & !slot.blocked.bits(),
        );
        slot.up |= Buttons::from_bits(slot.state.buttons.bits() & !state.buttons.bits());
        // Preserve source edges, including a tap coalesced by WPAD polling.
        slot.down |= Buttons::from_bits(state.buttons_down.bits() & !slot.blocked.bits());
        slot.up |= state.buttons_up;
        slot.state = state;
        Ok(ControllerId {
            source: state.source,
            generation: slot.generation,
        })
    }

    /// Take accumulated edges once. Held input remains available next frame.
    pub fn take(&mut self, index: usize) -> Option<ControllerState> {
        let slot = self.slots.get_mut(index)?;
        if !slot.state.connected {
            return None;
        }
        let mut state = slot.state;
        state.buttons_down = slot.down;
        state.buttons_up = slot.up;
        slot.down = Buttons::empty();
        slot.up = Buttons::empty();
        // Source edges are consumed once, even if the sample remains unchanged.
        slot.state.buttons_down = Buttons::empty();
        slot.state.buttons_up = Buttons::empty();
        Some(state)
    }

    pub fn id(&self, index: usize) -> Option<ControllerId> {
        let slot = self.slots.get(index)?;
        slot.state.connected.then_some(ControllerId {
            source: slot.state.source,
            generation: slot.generation,
        })
    }

    pub fn reset(&mut self, index: usize, reason: ResetReason) -> Result<(), TransitionError> {
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(TransitionError::InvalidSlot)?;
        if !slot.state.connected {
            return Ok(());
        }
        if self.resets[index].is_some() {
            return Err(TransitionError::ResetPending);
        }
        let mut state = slot.state;
        state.connected = false;
        state.buttons = Buttons::empty();
        // BUG FIX: keep every edge collected before disconnect in the reset event.
        // A quick tap can have both edges pending when the source disappears.
        state.buttons_down = slot.down;
        state.buttons_up = slot.up | slot.state.buttons;
        state.axes = Axes::default();
        self.resets[index] = Some(ControllerReset {
            id: ControllerId {
                source: state.source,
                generation: slot.generation,
            },
            reason,
            state,
        });
        let generation = slot.generation;
        *slot = Slot::EMPTY;
        slot.generation = generation;
        Ok(())
    }

    pub fn take_reset(&mut self, index: usize) -> Option<ControllerReset> {
        self.resets.get_mut(index)?.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(buttons: Buttons) -> ControllerState {
        ControllerState {
            source: ControllerSource::BluetoothHid(0),
            connected: true,
            buttons,
            ..ControllerState::empty()
        }
    }
    #[test]
    fn identity_tracks_disconnect_reconnect_and_invalid_slots() {
        let mut input = InputTransitions::<1>::new();
        assert_eq!(input.id(0), None);
        assert_eq!(input.id(1), None);
        let first = input.sample(0, state(Buttons::empty())).unwrap();
        assert_eq!(first.generation, 1);
        assert_eq!(input.id(0), Some(first));
        input.reset(0, ResetReason::Disconnected).unwrap();
        assert_eq!(input.id(0), None);
        assert_eq!(input.take_reset(0).unwrap().id, first);
        let second = input.sample(0, state(Buttons::empty())).unwrap();
        assert_eq!(second.generation, 2);
        assert_eq!(input.id(0), Some(second));
    }

    #[test]
    fn held_at_connect_does_not_activate_and_rearms_after_release() {
        let mut input = InputTransitions::<1>::new();
        input.sample(0, state(Buttons::A)).unwrap();
        assert_eq!(input.take(0).unwrap().buttons_down, Buttons::empty());
        input.sample(0, state(Buttons::empty())).unwrap();
        assert_eq!(input.take(0).unwrap().buttons_up, Buttons::A);
        input.sample(0, state(Buttons::A)).unwrap();
        assert_eq!(input.take(0).unwrap().buttons_down, Buttons::A);
        assert_eq!(input.take(0).unwrap().buttons_down, Buttons::empty());
    }
    #[test]
    fn a_quick_tap_between_frames_keeps_both_edges() {
        let mut input = InputTransitions::<1>::new();
        input.sample(0, state(Buttons::empty())).unwrap();
        input.sample(0, state(Buttons::A)).unwrap();
        input.sample(0, state(Buttons::empty())).unwrap();
        let frame = input.take(0).unwrap();
        assert_eq!(frame.buttons, Buttons::empty());
        assert_eq!(frame.buttons_down, Buttons::A);
        assert_eq!(frame.buttons_up, Buttons::A);
    }
    #[test]
    fn disconnect_releases_and_reconnect_has_a_new_identity() {
        let mut input = InputTransitions::<1>::new();
        let first = input.sample(0, state(Buttons::empty())).unwrap();
        input.sample(0, state(Buttons::A | Buttons::B)).unwrap();
        input.reset(0, ResetReason::SourceError(-3)).unwrap();
        assert_eq!(input.take(0), None);
        let reset = input.take_reset(0).unwrap();
        assert_eq!(reset.id, first);
        assert_eq!(reset.reason, ResetReason::SourceError(-3));
        assert_eq!(reset.state.buttons_up, Buttons::A | Buttons::B);
        assert_eq!(reset.state.axes, Axes::default());
        assert_eq!(input.take_reset(0), None);
        let second = input.sample(0, state(Buttons::A)).unwrap();
        assert_eq!(second.generation, first.generation + 1);
        assert_eq!(input.take(0).unwrap().buttons_down, Buttons::empty());
    }

    #[test]
    fn disconnect_preserves_accumulated_press_and_release_edges() {
        let mut input = InputTransitions::<1>::new();
        input.sample(0, state(Buttons::empty())).unwrap();
        input.sample(0, state(Buttons::A)).unwrap();
        input.sample(0, state(Buttons::empty())).unwrap();
        input.reset(0, ResetReason::Disconnected).unwrap();

        let reset = input.take_reset(0).unwrap();
        assert_eq!(reset.state.buttons, Buttons::empty());
        assert_eq!(reset.state.buttons_down, Buttons::A);
        assert_eq!(reset.state.buttons_up, Buttons::A);
        assert_eq!(reset.state.axes, Axes::default());
    }

    #[test]
    fn raw_masks_use_the_same_connect_gate_and_disconnect_release() {
        let mut raw = RawButtonTransitions::<1>::new();
        let source = ControllerSource::WiiBluetooth(0);
        assert_eq!(
            raw.sample(
                0,
                RawButtonSample {
                    source,
                    connected: true,
                    held: 0x10,
                    down: 0x10,
                    up: 0,
                },
            )
            .unwrap(),
            RawButtonFrame {
                held: 0x10,
                down: 0,
                up: 0,
            }
        );
        assert_eq!(
            raw.sample(
                0,
                RawButtonSample {
                    source,
                    connected: true,
                    held: 0,
                    down: 0,
                    up: 0x10,
                },
            )
            .unwrap(),
            RawButtonFrame {
                held: 0,
                down: 0,
                up: 0x10,
            }
        );
        assert_eq!(
            raw.sample(
                0,
                RawButtonSample {
                    source,
                    connected: true,
                    held: 0x10,
                    down: 0x10,
                    up: 0,
                },
            )
            .unwrap(),
            RawButtonFrame {
                held: 0x10,
                down: 0x10,
                up: 0,
            }
        );
        assert_eq!(
            raw.sample(
                0,
                RawButtonSample {
                    source,
                    connected: false,
                    held: 0,
                    down: 0,
                    up: 0,
                },
            )
            .unwrap(),
            RawButtonFrame {
                held: 0,
                down: 0,
                up: 0x10,
            }
        );
    }
    #[test]
    fn replacement_preserves_the_old_release_and_bounds_are_checked() {
        let mut input = InputTransitions::<1>::new();
        input.sample(0, state(Buttons::A)).unwrap();
        let mut replacement = state(Buttons::B);
        replacement.source = ControllerSource::UsbHid(0);
        input.sample(0, replacement).unwrap();
        assert_eq!(input.take_reset(0).unwrap().state.buttons_up, Buttons::A);
        assert_eq!(input.take(0).unwrap().buttons_down, Buttons::empty());
        assert_eq!(
            input.sample(1, replacement),
            Err(TransitionError::InvalidSlot)
        );
        assert_eq!(
            input.reset(1, ResetReason::Disconnected),
            Err(TransitionError::InvalidSlot)
        );
    }
}
