//! Portable mapping from normalized controller snapshots to UI input.
//!
//! This module maps snapshots collected by `ogc-controller`; it does not poll
//! a device or create another button transition engine.

use alloc::vec::Vec;
use ogc_controller::{
    Axes, Buttons, ControllerId, ControllerKind, ControllerMetadata, ControllerSnapshot,
    ControllerSource,
};

use crate::{Direction, UiError, UiInput, repeat::KeyRepeat};

const MAX_AXIS: i32 = 32_767;
const STICK_PRESS: i32 = MAX_AXIS / 4;
const STICK_RELEASE: i32 = MAX_AXIS / 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stick {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInputBindings {
    /// Normalized buttons that accept or confirm.
    pub activate: u32,
    /// Normalized buttons that cancel or go back.
    pub cancel: u32,
    /// Button masks in up, down, left, right order.
    pub directions: [u32; 4],
    /// Stick used for digital direction input, if the kind has one.
    pub stick: Option<Stick>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInputLabels {
    pub activate: &'static str,
    pub cancel: &'static str,
    pub directions: &'static str,
    pub stick: &'static str,
}

/// Buttons and help labels for one controller kind or source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInputProfile {
    pub bindings: ControllerInputBindings,
    pub labels: ControllerInputLabels,
}

impl ControllerInputProfile {
    /// Return documented defaults for one normalized controller kind.
    pub const fn for_kind(kind: ControllerKind) -> Self {
        let (activate, cancel, labels, stick) = match kind {
            ControllerKind::GameCube => (
                Buttons::A.bits(),
                Buttons::B.bits(),
                ControllerInputLabels {
                    activate: "A",
                    cancel: "B",
                    directions: "D-Pad",
                    stick: "Control Stick",
                },
                Some(Stick::Left),
            ),
            ControllerKind::JoyConLeft => (
                Buttons::SR.bits(),
                Buttons::SL.bits(),
                ControllerInputLabels {
                    activate: "SR",
                    cancel: "SL",
                    directions: "Direction Buttons",
                    stick: "Left Stick",
                },
                Some(Stick::Left),
            ),
            ControllerKind::JoyConRight => (
                Buttons::A.bits(),
                Buttons::B.bits(),
                ControllerInputLabels {
                    activate: "A",
                    cancel: "B",
                    directions: "Right Stick",
                    stick: "Right Stick",
                },
                Some(Stick::Right),
            ),
            ControllerKind::JoyConPair | ControllerKind::SwitchPro => (
                Buttons::A.bits(),
                Buttons::B.bits(),
                ControllerInputLabels {
                    activate: "A",
                    cancel: "B",
                    directions: "D-Pad",
                    stick: "Left Stick",
                },
                Some(Stick::Left),
            ),
            _ => (
                Buttons::A.bits(),
                Buttons::B.bits(),
                ControllerInputLabels {
                    activate: "Confirm",
                    cancel: "Back",
                    directions: "Directions",
                    stick: "Stick",
                },
                Some(Stick::Left),
            ),
        };
        Self {
            bindings: ControllerInputBindings {
                activate,
                cancel,
                directions: [
                    Buttons::UP.bits(),
                    Buttons::DOWN.bits(),
                    Buttons::LEFT.bits(),
                    Buttons::RIGHT.bits(),
                ],
                stick,
            },
            labels,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileSelector {
    Kind(ControllerKind),
    Source(ControllerSource),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfileOverride {
    selector: ProfileSelector,
    profile: ControllerInputProfile,
}

/// Frame timing for UI key repeat. The caller must set the actual frame rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInputConfig {
    frame_rate_hz: u16,
}

impl ControllerInputConfig {
    pub const fn new(frame_rate_hz: u16) -> Option<Self> {
        if frame_rate_hz == 0 {
            None
        } else {
            Some(Self { frame_rate_hz })
        }
    }
}

impl Default for ControllerInputConfig {
    fn default() -> Self {
        Self { frame_rate_hz: 60 }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerInputReleases {
    pub activate: bool,
    pub cancel: bool,
    pub direction: Option<Direction>,
}

/// Mapped action and labels. Release details are observations; `UiInput` has
/// no release fields, so the widget tree does not receive release events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerInputFrame {
    pub input: UiInput,
    pub source: Option<ControllerSource>,
    pub labels: Option<ControllerInputLabels>,
    pub pointer_source: Option<ControllerSource>,
    pub released: ControllerInputReleases,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StickGate {
    id: ControllerId,
    direction: Option<Direction>,
    blocked: bool,
}

/// Stateful UI mapping over snapshots produced by `ControllerManager`.
pub struct ControllerInputAdapter {
    repeat: KeyRepeat<(ControllerId, Direction)>,
    profiles: Vec<ProfileOverride>,
    stick_gates: Vec<StickGate>,
    pointer_owner: Option<ControllerId>,
}

impl ControllerInputAdapter {
    pub fn new(config: ControllerInputConfig) -> Self {
        let (delay, interval) = repeat_frames(config.frame_rate_hz);
        Self {
            repeat: KeyRepeat::new(delay, interval),
            profiles: Vec::new(),
            stick_gates: Vec::new(),
            pointer_owner: None,
        }
    }

    pub fn set_kind_profile(
        &mut self,
        kind: ControllerKind,
        profile: ControllerInputProfile,
    ) -> Result<(), UiError> {
        self.set_profile(ProfileSelector::Kind(kind), profile)
    }

    pub fn set_source_profile(
        &mut self,
        source: ControllerSource,
        profile: ControllerInputProfile,
    ) -> Result<(), UiError> {
        self.set_profile(ProfileSelector::Source(source), profile)
    }

    fn set_profile(
        &mut self,
        selector: ProfileSelector,
        profile: ControllerInputProfile,
    ) -> Result<(), UiError> {
        if let Some(existing) = self
            .profiles
            .iter_mut()
            .find(|entry| entry.selector == selector)
        {
            existing.profile = profile;
        } else {
            self.profiles
                .try_reserve(1)
                .map_err(|_| UiError::AllocationFailed)?;
            self.profiles.push(ProfileOverride { selector, profile });
        }
        Ok(())
    }

    /// Reset repeat and stick state when the caller opens a new input scope.
    /// Held sticks remain blocked until they return to neutral.
    pub fn reset(&mut self) {
        self.repeat.reset();
        self.pointer_owner = None;
        for gate in &mut self.stick_gates {
            gate.direction = None;
            gate.blocked = true;
        }
    }

    /// Map one production snapshot and its stored metadata to `UiInput`.
    pub fn update(
        &mut self,
        snapshot: &ControllerSnapshot,
        viewport: [u16; 2],
    ) -> Result<ControllerInputFrame, UiError> {
        self.update_with_metadata(snapshot, viewport, |source| snapshot.metadata(source))
    }

    /// Map a snapshot with caller-supplied metadata lookup. This supports
    /// explicit metadata sources while production callers can use `update`.
    pub fn update_with_metadata(
        &mut self,
        snapshot: &ControllerSnapshot,
        viewport: [u16; 2],
        mut metadata_for: impl FnMut(ControllerSource) -> Option<ControllerMetadata>,
    ) -> Result<ControllerInputFrame, UiError> {
        // Reserve enough space for one stick gate per connected source before
        // changing repeat, pointer, or gate state.
        self.stick_gates
            .try_reserve(snapshot.controllers().len())
            .map_err(|_| UiError::AllocationFailed)?;
        let mut mapped = MappingState::default();
        for state in snapshot.controllers() {
            if !state.connected {
                continue;
            }
            let id = controller_id(snapshot, state.source);
            let profile = self.profile_for(state.kind, state.source);
            mapped.collect_buttons(
                state.buttons.bits(),
                state.buttons_down.bits(),
                state.buttons_up.bits(),
                profile,
                id,
                state.source,
            );

            let (axis_direction, axis_pressed, axis_released) =
                self.sample_stick(id, profile.bindings.stick, state.axes);
            if mapped.held_direction.is_none() {
                mapped.held_direction = axis_direction.map(|direction| DirectionKey {
                    id,
                    source: state.source,
                    direction,
                });
            }
            if mapped.pressed_direction.is_none() {
                mapped.pressed_direction = axis_pressed.map(|direction| DirectionKey {
                    id,
                    source: state.source,
                    direction,
                });
            }
            if mapped.released.direction.is_none() {
                mapped.released.direction = axis_released;
            }
        }

        for reset in snapshot.resets() {
            if mapped.released.direction.is_none() {
                mapped.released.direction = self.remove_stick_gate(reset.id);
            } else {
                self.remove_stick_gate(reset.id);
            }
            let profile = self.profile_for(reset.state.kind, reset.state.source);
            mapped.collect_buttons(
                reset.state.buttons.bits(),
                reset.state.buttons_down.bits(),
                reset.state.buttons_up.bits(),
                profile,
                reset.id,
                reset.state.source,
            );
        }

        self.stick_gates.retain(|gate| {
            snapshot
                .controllers()
                .iter()
                .any(|state| state.connected && controller_id(snapshot, state.source) == gate.id)
        });
        let owner = self.select_pointer(snapshot, viewport, &mut metadata_for);
        let owner_source = owner.map(|(_, source, _)| source);
        let owner_activate = owner_source.is_some_and(|source| {
            snapshot.controllers().iter().any(|state| {
                state.source == source
                    && state.buttons_down.bits()
                        & self.profile_for(state.kind, source).bindings.activate
                        != 0
            })
        });
        let cancel = mapped.pressed_cancel.is_some();
        let activate = mapped.pressed_activate.is_some() && !owner_activate && !cancel;
        let pointer_activate = owner_activate && !cancel;
        let direction = if cancel {
            self.repeat.reset();
            None
        } else {
            sample_direction_repeat(
                &mut self.repeat,
                mapped.held_direction,
                mapped.pressed_direction,
            )
        };

        let source = mapped
            .pressed_cancel
            .or_else(|| {
                mapped
                    .pressed_activate
                    .filter(|(_source, _)| !owner_activate || cancel)
            })
            .map(|(source, _)| source)
            .or_else(|| {
                mapped
                    .pressed_direction
                    .or(mapped.held_direction)
                    .map(|key| key.source)
            })
            .or(owner_source);
        let labels = source.map(|source| {
            let kind = snapshot
                .controllers()
                .iter()
                .find(|state| state.source == source)
                .map(|state| state.kind)
                .or_else(|| {
                    snapshot
                        .resets()
                        .find(|reset| reset.state.source == source)
                        .map(|reset| reset.state.kind)
                })
                .unwrap_or(ControllerKind::Unknown);
            self.profile_for(kind, source).labels
        });

        Ok(ControllerInputFrame {
            input: UiInput {
                direction: if cancel { None } else { direction },
                activate,
                cancel,
                pointer: if cancel {
                    None
                } else {
                    owner.map(|(_, _, point)| point)
                },
                pointer_activate,
            },
            source,
            labels,
            pointer_source: owner_source,
            released: mapped.released,
        })
    }

    fn profile_for(
        &self,
        kind: ControllerKind,
        source: ControllerSource,
    ) -> ControllerInputProfile {
        self.profiles
            .iter()
            .find(|entry| entry.selector == ProfileSelector::Source(source))
            .or_else(|| {
                self.profiles
                    .iter()
                    .find(|entry| entry.selector == ProfileSelector::Kind(kind))
            })
            .map_or_else(
                || ControllerInputProfile::for_kind(kind),
                |entry| entry.profile,
            )
    }

    fn sample_stick(
        &mut self,
        id: ControllerId,
        stick: Option<Stick>,
        axes: Axes,
    ) -> (Option<Direction>, Option<Direction>, Option<Direction>) {
        let Some(stick) = stick else {
            return (None, None, None);
        };
        let (x, y) = match stick {
            Stick::Left => (axes.left_x, axes.left_y),
            Stick::Right => (axes.right_x, axes.right_y),
        };
        let x = i32::from(x);
        let y = i32::from(y);
        let Some(index) = self.stick_gates.iter().position(|gate| gate.id == id) else {
            let blocked = x.abs().max(y.abs()) > STICK_RELEASE;
            self.stick_gates.push(StickGate {
                id,
                direction: None,
                blocked,
            });
            return (None, None, None);
        };
        let gate = &mut self.stick_gates[index];
        if gate.blocked {
            if x.abs().max(y.abs()) <= STICK_RELEASE {
                gate.blocked = false;
            }
            return (None, None, None);
        }
        let previous = gate.direction;
        let next = if previous.is_some_and(|direction| direction_is_active(direction, x, y)) {
            previous
        } else {
            strongest_direction(x, y)
                .filter(|direction| direction_magnitude(*direction, x, y) >= STICK_PRESS)
        };
        gate.direction = next;
        let pressed = (next != previous).then_some(next).flatten();
        let released = (next != previous).then_some(previous).flatten();
        (next, pressed, released)
    }

    fn remove_stick_gate(&mut self, id: ControllerId) -> Option<Direction> {
        let direction = self
            .stick_gates
            .iter()
            .find(|gate| gate.id == id)
            .and_then(|gate| gate.direction);
        self.stick_gates.retain(|gate| gate.id != id);
        direction
    }

    fn select_pointer(
        &mut self,
        snapshot: &ControllerSnapshot,
        viewport: [u16; 2],
        metadata_for: &mut impl FnMut(ControllerSource) -> Option<ControllerMetadata>,
    ) -> Option<(ControllerId, ControllerSource, (u16, u16))> {
        let find_owner = |source_id: Option<ControllerId>,
                          metadata_for: &mut dyn FnMut(
            ControllerSource,
        ) -> Option<ControllerMetadata>| {
            snapshot.controllers().iter().find_map(|state| {
                let id = controller_id(snapshot, state.source);
                if !state.connected || source_id.is_some_and(|wanted| wanted != id) {
                    return None;
                }
                let metadata = metadata_for(state.source)?;
                let pointer = valid_pointer(metadata, viewport)?;
                Some((id, state.source, pointer))
            })
        };
        let owner = self
            .pointer_owner
            .and_then(|previous| find_owner(Some(previous), metadata_for))
            .or_else(|| find_owner(None, metadata_for));
        self.pointer_owner = owner.map(|candidate| candidate.0);
        owner
    }
}

impl Default for ControllerInputAdapter {
    fn default() -> Self {
        Self::new(ControllerInputConfig::default())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectionKey {
    id: ControllerId,
    source: ControllerSource,
    direction: Direction,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MappingState {
    pressed_activate: Option<(ControllerSource, ControllerInputLabels)>,
    pressed_cancel: Option<(ControllerSource, ControllerInputLabels)>,
    released: ControllerInputReleases,
    held_direction: Option<DirectionKey>,
    pressed_direction: Option<DirectionKey>,
}

impl MappingState {
    fn collect_buttons(
        &mut self,
        held: u32,
        down: u32,
        up: u32,
        profile: ControllerInputProfile,
        id: ControllerId,
        source: ControllerSource,
    ) {
        if self.pressed_activate.is_none() && down & profile.bindings.activate != 0 {
            self.pressed_activate = Some((source, profile.labels));
        }
        if self.pressed_cancel.is_none() && down & profile.bindings.cancel != 0 {
            self.pressed_cancel = Some((source, profile.labels));
        }
        self.released.activate |= up & profile.bindings.activate != 0;
        self.released.cancel |= up & profile.bindings.cancel != 0;
        let direction_for = |mask| {
            profile
                .bindings
                .directions
                .iter()
                .zip([
                    Direction::Up,
                    Direction::Down,
                    Direction::Left,
                    Direction::Right,
                ])
                .find_map(|(button, direction)| (mask & button != 0).then_some(direction))
        };
        if self.held_direction.is_none() {
            self.held_direction = direction_for(held).map(|direction| DirectionKey {
                id,
                source,
                direction,
            });
        }
        if self.pressed_direction.is_none() {
            self.pressed_direction = direction_for(down).map(|direction| DirectionKey {
                id,
                source,
                direction,
            });
        }
        if self.released.direction.is_none() {
            self.released.direction = direction_for(up);
        }
    }
}

fn sample_direction_repeat(
    repeat: &mut KeyRepeat<(ControllerId, Direction)>,
    held: Option<DirectionKey>,
    pressed: Option<DirectionKey>,
) -> Option<Direction> {
    let key = pressed.or(held).map(|key| (key.id, key.direction));
    let pressed_key = pressed.map(|key| (key.id, key.direction));
    let repeated = repeat.sample(key, pressed_key);
    if repeated {
        key.map(|(_, direction)| direction)
    } else if key.is_none() {
        // A quick directional tap still emits its press edge.
        pressed.map(|key| key.direction)
    } else {
        None
    }
}

fn controller_id(snapshot: &ControllerSnapshot, source: ControllerSource) -> ControllerId {
    snapshot.controller_id(source).unwrap_or(ControllerId {
        source,
        generation: 0,
    })
}

fn valid_pointer(metadata: ControllerMetadata, viewport: [u16; 2]) -> Option<(u16, u16)> {
    if !metadata.capabilities.pointer || viewport[0] == 0 || viewport[1] == 0 {
        return None;
    }
    let pointer = metadata.pointer?;
    let (x, y) = pointer.position();
    let [source_width, source_height] = pointer.resolution;
    if x < 0.0 || y < 0.0 || x >= source_width as f32 || y >= source_height as f32 {
        return None;
    }
    let x = (x * f32::from(viewport[0]) / source_width as f32) as u16;
    let y = (y * f32::from(viewport[1]) / source_height as f32) as u16;
    Some((x, y))
}

fn strongest_direction(x: i32, y: i32) -> Option<Direction> {
    let mut candidates = [
        (
            y.abs(),
            (y > 0)
                .then_some(Direction::Up)
                .or_else(|| (y < 0).then_some(Direction::Down)),
        ),
        (
            x.abs(),
            (x > 0)
                .then_some(Direction::Right)
                .or_else(|| (x < 0).then_some(Direction::Left)),
        ),
    ];
    candidates.sort_by_key(|candidate| core::cmp::Reverse(candidate.0));
    candidates[0].1
}

fn direction_magnitude(direction: Direction, x: i32, y: i32) -> i32 {
    match direction {
        Direction::Up | Direction::Down => y.abs(),
        Direction::Left | Direction::Right => x.abs(),
        Direction::Previous | Direction::Next => 0,
    }
}

fn direction_is_active(direction: Direction, x: i32, y: i32) -> bool {
    match direction {
        Direction::Up => y > STICK_RELEASE,
        Direction::Down => y < -STICK_RELEASE,
        Direction::Left => x < -STICK_RELEASE,
        Direction::Right => x > STICK_RELEASE,
        Direction::Previous | Direction::Next => false,
    }
}

fn repeat_frames(frame_rate_hz: u16) -> (u16, u16) {
    let frames = |milliseconds: u32| {
        (milliseconds * u32::from(frame_rate_hz))
            .div_ceil(1_000)
            .max(1) as u16
    };
    (frames(350), frames(80))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Size, Ui, UiEvent};

    #[test]
    fn controller_input_config_rejects_zero_frame_rate_and_accepts_positive_rate() {
        assert_eq!(ControllerInputConfig::new(0), None);
        assert_eq!(
            ControllerInputConfig::new(50),
            Some(ControllerInputConfig { frame_rate_hz: 50 })
        );
    }

    fn map_update(
        adapter: &mut ControllerInputAdapter,
        snapshot: &ControllerSnapshot,
        viewport: [u16; 2],
    ) -> ControllerInputFrame {
        adapter.update(snapshot, viewport).unwrap()
    }

    fn map_update_with_metadata(
        adapter: &mut ControllerInputAdapter,
        snapshot: &ControllerSnapshot,
        viewport: [u16; 2],
        metadata_for: impl FnMut(ControllerSource) -> Option<ControllerMetadata>,
    ) -> ControllerInputFrame {
        adapter
            .update_with_metadata(snapshot, viewport, metadata_for)
            .unwrap()
    }

    fn state(
        source: ControllerSource,
        kind: ControllerKind,
        buttons: u32,
        buttons_down: u32,
        buttons_up: u32,
        axes: Axes,
    ) -> ogc_controller::ControllerState {
        ogc_controller::ControllerState {
            source,
            kind,
            connected: true,
            buttons: Buttons::from_bits(buttons),
            buttons_down: Buttons::from_bits(buttons_down),
            buttons_up: Buttons::from_bits(buttons_up),
            axes,
        }
    }

    fn snapshot(state: ogc_controller::ControllerState) -> ControllerSnapshot {
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state);
        snapshot
    }

    fn metadata(x: f32, y: f32, resolution: [u32; 2]) -> ControllerMetadata {
        ControllerMetadata {
            capabilities: ogc_controller::ControllerCapabilities {
                pointer: true,
                ..Default::default()
            },
            pointer: ogc_controller::PointerMetadata::new(x, y, resolution),
        }
    }

    #[test]
    fn each_switch_and_gamecube_kind_uses_its_own_action_profile() {
        let cases = [
            (
                ControllerSource::GameCube(0),
                ControllerKind::GameCube,
                Buttons::A.bits(),
                true,
                "A",
            ),
            (
                ControllerSource::BluetoothHid(0),
                ControllerKind::SwitchPro,
                Buttons::A.bits(),
                true,
                "A",
            ),
            (
                ControllerSource::BluetoothHidPair(0, 1),
                ControllerKind::JoyConPair,
                Buttons::B.bits(),
                false,
                "B",
            ),
            (
                ControllerSource::BluetoothHid(1),
                ControllerKind::JoyConRight,
                Buttons::B.bits(),
                false,
                "B",
            ),
        ];
        for (source, kind, press, activating, label) in cases {
            let mut adapter = ControllerInputAdapter::default();
            let frame = map_update(
                &mut adapter,
                &snapshot(state(source, kind, press, press, 0, Axes::default())),
                [640, 480],
            );
            assert_eq!(frame.input.activate, activating);
            assert_eq!(frame.input.cancel, !activating);
            let labels = frame.labels.unwrap();
            assert_eq!(
                if activating {
                    labels.activate
                } else {
                    labels.cancel
                },
                label
            );
            assert_eq!(frame.source, Some(source));
        }

        assert_eq!(
            ControllerInputProfile::for_kind(ControllerKind::JoyConRight)
                .labels
                .directions,
            "Right Stick"
        );
        assert_eq!(
            ControllerInputProfile::for_kind(ControllerKind::Unknown)
                .labels
                .directions,
            "Directions"
        );

        let source = ControllerSource::BluetoothHid(2);
        let mut left = ControllerInputAdapter::default();
        let frame = map_update(
            &mut left,
            &snapshot(state(
                source,
                ControllerKind::JoyConLeft,
                Buttons::A.bits() | Buttons::B.bits() | Buttons::SR.bits(),
                Buttons::A.bits() | Buttons::B.bits() | Buttons::SR.bits(),
                0,
                Axes::default(),
            )),
            [640, 480],
        );
        assert!(frame.input.activate);
        assert_eq!(frame.labels.unwrap().activate, "SR");
    }

    #[test]
    fn cancel_wins_over_confirm_and_pointer_activation_in_the_real_tree() {
        let source = ControllerSource::BluetoothHidPair(0, 1);
        let state = state(
            source,
            ControllerKind::JoyConPair,
            Buttons::A.bits() | Buttons::B.bits(),
            Buttons::A.bits() | Buttons::B.bits(),
            0,
            Axes::default(),
        );
        let mut adapter = ControllerInputAdapter::default();
        let frame =
            map_update_with_metadata(&mut adapter, &snapshot(state), [100, 60], |candidate| {
                (candidate == source).then_some(metadata(50.0, 40.0, [100, 60]))
            });
        assert!(frame.input.cancel);
        assert!(!frame.input.activate);
        assert!(!frame.input.pointer_activate);
        assert_eq!(frame.input.pointer, None);

        let mut ui = Ui::new(100, 60).unwrap();
        let first = ui
            .add_button(ui.root(), "FIRST", Size::fixed(100, 24))
            .unwrap();
        ui.add_button(ui.root(), "SECOND", Size::fixed(100, 24))
            .unwrap();
        assert_eq!(ui.update(frame.input), Some(UiEvent::Cancelled(first)));
    }

    #[test]
    fn pointer_owner_activates_only_the_hit_widget_once() {
        let source = ControllerSource::WiiBluetooth(0);
        let state = state(
            source,
            ControllerKind::WiiRemote,
            Buttons::A.bits(),
            Buttons::A.bits(),
            0,
            Axes::default(),
        );
        let mut adapter = ControllerInputAdapter::default();
        let frame = map_update_with_metadata(&mut adapter, &snapshot(state), [100, 60], |_| {
            Some(metadata(50.0, 30.0, [100, 60]))
        });
        assert!(frame.input.pointer_activate);
        assert!(!frame.input.activate);
        assert_eq!(frame.pointer_source, Some(source));

        let mut ui = Ui::new(100, 60).unwrap();
        ui.add_button(ui.root(), "FIRST", Size::fixed(100, 24))
            .unwrap();
        let second = ui
            .add_button(ui.root(), "SECOND", Size::fixed(100, 24))
            .unwrap();
        assert_eq!(ui.update(frame.input), Some(UiEvent::Activated(second)));
    }

    #[test]
    fn pointer_metadata_scales_valid_coordinates_and_rejects_offscreen_samples() {
        let source = ControllerSource::WiiBluetooth(0);
        let snapshot = snapshot(state(
            source,
            ControllerKind::WiiRemote,
            0,
            0,
            0,
            Axes::default(),
        ));
        let mut adapter = ControllerInputAdapter::default();
        let frame = adapter
            .update_with_metadata(&snapshot, [200, 80], |_| {
                Some(metadata(25.0, 10.0, [100, 40]))
            })
            .unwrap();
        assert_eq!(frame.input.pointer, Some((50, 20)));
        let offscreen = adapter
            .update_with_metadata(&snapshot, [200, 80], |_| {
                Some(metadata(100.0, 10.0, [100, 40]))
            })
            .unwrap();
        assert_eq!(offscreen.input.pointer, None);

        assert_eq!(
            valid_pointer(metadata(25.0, 10.0, [100, 40]), [200, 80]),
            Some((50, 20))
        );
        assert_eq!(
            valid_pointer(metadata(100.0, 10.0, [100, 40]), [200, 80]),
            None
        );
        assert_eq!(
            valid_pointer(metadata(-0.1, 10.0, [100, 40]), [200, 80]),
            None
        );
        assert_eq!(valid_pointer(metadata(1.0, 1.0, [100, 40]), [0, 80]), None);
    }

    #[test]
    fn pointer_owner_stays_stable_then_replaces_invalid_pointer_source() {
        let first = ControllerSource::WiiBluetooth(0);
        let second = ControllerSource::WiiBluetooth(1);
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            first,
            ControllerKind::WiiRemote,
            0,
            0,
            0,
            Axes::default(),
        ));
        snapshot.push(state(
            second,
            ControllerKind::WiiRemote,
            0,
            0,
            0,
            Axes::default(),
        ));
        let mut adapter = ControllerInputAdapter::default();
        let metadata_for = |source, first_x| {
            if source == first {
                Some(metadata(first_x, 10.0, [100, 40]))
            } else if source == second {
                Some(metadata(20.0, 10.0, [100, 40]))
            } else {
                None
            }
        };
        let frame = map_update_with_metadata(&mut adapter, &snapshot, [200, 80], |source| {
            metadata_for(source, 10.0)
        });
        assert_eq!(frame.pointer_source, Some(first));
        let frame = map_update_with_metadata(&mut adapter, &snapshot, [200, 80], |source| {
            metadata_for(source, 15.0)
        });
        assert_eq!(frame.pointer_source, Some(first));
        assert_eq!(frame.input.pointer, Some((30, 20)));
        let frame = map_update_with_metadata(&mut adapter, &snapshot, [200, 80], |source| {
            metadata_for(source, 100.0)
        });
        assert_eq!(frame.pointer_source, Some(second));
        assert_eq!(frame.input.pointer, Some((40, 20)));
    }

    #[test]
    fn other_controller_confirm_activates_focus_and_source_override_wins() {
        let pointer = ControllerSource::WiiBluetooth(0);
        let button = ControllerSource::BluetoothHid(0);
        let mut pointer_snapshot = ControllerSnapshot::empty();
        pointer_snapshot.push(state(
            pointer,
            ControllerKind::WiiRemote,
            0,
            0,
            0,
            Axes::default(),
        ));
        pointer_snapshot.push(state(
            button,
            ControllerKind::SwitchPro,
            Buttons::X.bits(),
            Buttons::X.bits(),
            0,
            Axes::default(),
        ));
        let mut adapter = ControllerInputAdapter::default();
        let mut kind_profile = ControllerInputProfile::for_kind(ControllerKind::SwitchPro);
        kind_profile.bindings.activate = Buttons::Y.bits();
        kind_profile.labels.activate = "Kind override";
        adapter
            .set_kind_profile(ControllerKind::SwitchPro, kind_profile)
            .unwrap();
        let mut profile = ControllerInputProfile::for_kind(ControllerKind::SwitchPro);
        profile.bindings.activate = Buttons::X.bits();
        profile.labels.activate = "Options key";
        adapter.set_source_profile(button, profile).unwrap();
        let frame =
            map_update_with_metadata(&mut adapter, &pointer_snapshot, [100, 60], |source| {
                (source == pointer).then_some(metadata(10.0, 10.0, [100, 60]))
            });
        assert!(frame.input.activate);
        assert!(!frame.input.pointer_activate);
        assert_eq!(frame.source, Some(button));
        assert_eq!(frame.labels.unwrap().activate, "Options key");

        let kind_source = ControllerSource::BluetoothHid(1);
        let kind_frame = map_update(
            &mut adapter,
            &snapshot(state(
                kind_source,
                ControllerKind::SwitchPro,
                Buttons::Y.bits(),
                Buttons::Y.bits(),
                0,
                Axes::default(),
            )),
            [100, 60],
        );
        assert!(kind_frame.input.activate);
        assert_eq!(kind_frame.labels.unwrap().activate, "Kind override");

        let mut simultaneous = ControllerSnapshot::empty();
        simultaneous.push(state(
            pointer,
            ControllerKind::WiiRemote,
            Buttons::A.bits(),
            Buttons::A.bits(),
            0,
            Axes::default(),
        ));
        simultaneous.push(state(
            button,
            ControllerKind::SwitchPro,
            Buttons::X.bits(),
            Buttons::X.bits(),
            0,
            Axes::default(),
        ));
        let frame = map_update_with_metadata(&mut adapter, &simultaneous, [100, 60], |source| {
            (source == pointer).then_some(metadata(10.0, 10.0, [100, 60]))
        });
        assert!(frame.input.pointer_activate);
        assert!(!frame.input.activate);

        let mut ui = Ui::new(100, 60).unwrap();
        let first = ui
            .add_button(ui.root(), "FIRST", Size::fixed(100, 24))
            .unwrap();
        let second = ui
            .add_button(ui.root(), "SECOND", Size::fixed(100, 24))
            .unwrap();
        assert_eq!(
            ui.update(crate::UiInput {
                direction: Some(Direction::Next),
                ..Default::default()
            }),
            None
        );
        assert_eq!(ui.focused(), Some(second));
        assert_eq!(ui.update(frame.input), Some(UiEvent::Activated(first)));
        assert_eq!(ui.focused(), Some(first));
    }

    #[test]
    fn held_axis_is_blocked_until_neutral_then_uses_hysteresis() {
        let source = ControllerSource::BluetoothHid(0);
        let mut adapter = ControllerInputAdapter::default();
        let held = |y| {
            snapshot(state(
                source,
                ControllerKind::SwitchPro,
                0,
                0,
                0,
                Axes {
                    left_y: y,
                    ..Default::default()
                },
            ))
        };
        assert_eq!(
            map_update(&mut adapter, &held(10_000), [100, 60])
                .input
                .direction,
            None
        );
        assert_eq!(
            map_update(&mut adapter, &held(0), [100, 60])
                .input
                .direction,
            None
        );
        assert_eq!(
            map_update(&mut adapter, &held(9_000), [100, 60])
                .input
                .direction,
            Some(Direction::Up)
        );
        assert_eq!(
            map_update(&mut adapter, &held(7_000), [100, 60])
                .input
                .direction,
            None
        );
        assert_eq!(
            map_update(&mut adapter, &held(6_000), [100, 60])
                .released
                .direction,
            Some(Direction::Up)
        );
        assert_eq!(
            map_update(&mut adapter, &held(-9_000), [100, 60])
                .input
                .direction,
            Some(Direction::Down)
        );
    }

    #[test]
    fn direction_repeat_uses_ceil_converted_350ms_and_80ms_intervals() {
        let source = ControllerSource::BluetoothHid(0);
        let mut adapter = ControllerInputAdapter::new(ControllerInputConfig::new(60).unwrap());
        let held = |down| {
            snapshot(state(
                source,
                ControllerKind::SwitchPro,
                Buttons::UP.bits(),
                if down { Buttons::UP.bits() } else { 0 },
                0,
                Axes::default(),
            ))
        };
        assert_eq!(
            map_update(&mut adapter, &held(true), [100, 60])
                .input
                .direction,
            Some(Direction::Up)
        );
        for _ in 0..20 {
            assert_eq!(
                map_update(&mut adapter, &held(false), [100, 60])
                    .input
                    .direction,
                None
            );
        }
        assert_eq!(
            map_update(&mut adapter, &held(false), [100, 60])
                .input
                .direction,
            Some(Direction::Up)
        );
        for _ in 0..4 {
            assert_eq!(
                map_update(&mut adapter, &held(false), [100, 60])
                    .input
                    .direction,
                None
            );
        }
        assert_eq!(
            map_update(&mut adapter, &held(false), [100, 60])
                .input
                .direction,
            Some(Direction::Up)
        );
    }

    #[test]
    fn direction_repeat_does_not_transfer_to_a_reconnected_generation() {
        let source = ControllerSource::BluetoothHid(0);
        let old_id = ControllerId {
            source,
            generation: 4,
        };
        let new_id = ControllerId {
            source,
            generation: 5,
        };
        let key = |id| DirectionKey {
            id,
            source,
            direction: Direction::Up,
        };
        let mut repeat = KeyRepeat::new(3, 2);
        assert_eq!(
            sample_direction_repeat(&mut repeat, None, Some(key(old_id))),
            Some(Direction::Up)
        );
        assert_eq!(
            sample_direction_repeat(&mut repeat, Some(key(old_id)), None),
            None
        );
        assert_eq!(
            sample_direction_repeat(&mut repeat, Some(key(old_id)), None),
            None
        );
        assert_eq!(
            sample_direction_repeat(&mut repeat, Some(key(old_id)), None),
            Some(Direction::Up)
        );

        // A same-source reconnect has a new generation. Its held direction
        // cannot resume the previous controller's repeat sequence.
        assert_eq!(
            sample_direction_repeat(&mut repeat, Some(key(new_id)), None),
            None
        );
        for _ in 0..5 {
            assert_eq!(
                sample_direction_repeat(&mut repeat, Some(key(new_id)), None),
                None
            );
        }
        assert_eq!(sample_direction_repeat(&mut repeat, None, None), None);
        assert_eq!(
            sample_direction_repeat(&mut repeat, None, Some(key(new_id))),
            Some(Direction::Up)
        );
    }

    #[test]
    fn reset_blocks_held_direction_until_release_and_new_press() {
        let source = ControllerSource::BluetoothHid(0);
        let mut adapter = ControllerInputAdapter::default();
        let frame = |buttons, down| {
            snapshot(state(
                source,
                ControllerKind::SwitchPro,
                buttons,
                down,
                0,
                Axes::default(),
            ))
        };
        assert_eq!(
            map_update(
                &mut adapter,
                &frame(Buttons::UP.bits(), Buttons::UP.bits()),
                [100, 60]
            )
            .input
            .direction,
            Some(Direction::Up)
        );
        adapter.reset();
        assert_eq!(
            map_update(&mut adapter, &frame(Buttons::UP.bits(), 0), [100, 60])
                .input
                .direction,
            None
        );
        assert_eq!(
            map_update(&mut adapter, &frame(0, 0), [100, 60])
                .input
                .direction,
            None
        );
        assert_eq!(
            map_update(
                &mut adapter,
                &frame(Buttons::UP.bits(), Buttons::UP.bits()),
                [100, 60]
            )
            .input
            .direction,
            Some(Direction::Up)
        );
    }

    #[test]
    fn quick_press_release_edges_are_visible_without_rebuilding_transitions() {
        let source = ControllerSource::BluetoothHid(0);
        let snapshot = snapshot(state(
            source,
            ControllerKind::SwitchPro,
            0,
            Buttons::A.bits(),
            Buttons::A.bits(),
            Axes::default(),
        ));
        let frame = map_update(&mut ControllerInputAdapter::default(), &snapshot, [100, 60]);
        assert!(frame.input.activate);
        assert!(frame.released.activate);
        assert_eq!(frame.source, Some(source));
    }
}
