use crate::controller_input::{self, Input};
use ogc_controller::{Buttons, ControllerId, ControllerSnapshot};
use ogc_controller::{
    ControllerKind, ControllerSource, DigitalStickMapper, MAX_CONTROLLERS, StickDirectionError,
    StickDirectionSample,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalfControllerProfile {
    pub sr: Buttons,
    pub sl: Buttons,
    pub select: Buttons,
}

impl HalfControllerProfile {
    pub const fn new(sr: Buttons, sl: Buttons, select: Buttons) -> Self {
        Self { sr, sl, select }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HalfControllerInputError {
    MissingIdentity(ControllerSource),
    Stick(StickDirectionError),
}

/// Add Joy-Con half controls to the snapshot using the production stick
/// transition mapper. The profile only maps a connected half with a known ID.
pub fn project_wii_half_snapshot<const N: usize>(
    snapshot: ControllerSnapshot,
    timestamp_ticks: u64,
    profile: HalfControllerProfile,
    mapper: &mut DigitalStickMapper<N>,
) -> Result<Input, HalfControllerInputError> {
    project_wii_half_snapshot_with_identity(&snapshot, timestamp_ticks, profile, mapper, |source| {
        snapshot.controller_id(source)
    })
}

fn project_wii_half_snapshot_with_identity<const N: usize>(
    snapshot: &ControllerSnapshot,
    timestamp_ticks: u64,
    profile: HalfControllerProfile,
    mapper: &mut DigitalStickMapper<N>,
    mut identity_for: impl FnMut(ControllerSource) -> Option<ControllerId>,
) -> Result<Input, HalfControllerInputError> {
    let (mut input, base_has_contributors) =
        controller_input::action_input::project_snapshot_with_identity(
            *snapshot,
            timestamp_ticks,
            &mut identity_for,
        );
    let mut source = input.source;
    let mut source_ambiguous = base_has_contributors && source.is_none();
    for controller in snapshot.controllers() {
        if controller.kind != ControllerKind::JoyConLeft || !controller.connected {
            continue;
        }
        let identity = identity_for(controller.source);
        if identity.is_none()
            && !(controller.buttons | controller.buttons_down | controller.buttons_up).is_empty()
        {
            return Err(HalfControllerInputError::MissingIdentity(controller.source));
        }
        apply_left_half_actions(
            &mut input,
            &mut source,
            &mut source_ambiguous,
            base_has_contributors,
            LeftHalfEvent {
                kind: controller.kind,
                held: controller.buttons,
                down: controller.buttons_down,
                up: controller.buttons_up,
                identity,
            },
            profile,
        );
    }
    for reset in snapshot.resets() {
        if reset.state.kind == ControllerKind::JoyConLeft {
            apply_left_half_actions(
                &mut input,
                &mut source,
                &mut source_ambiguous,
                base_has_contributors,
                LeftHalfEvent {
                    kind: reset.state.kind,
                    held: Buttons::empty(),
                    down: reset.state.buttons_down,
                    up: reset.state.buttons_up,
                    identity: Some(reset.id),
                },
                profile,
            );
        }
    }

    let mut sample_storage =
        [const { core::mem::MaybeUninit::<StickDirectionSample>::uninit() }; MAX_CONTROLLERS];
    let mut sample_count = 0;
    for controller in snapshot.controllers() {
        if controller.kind != ControllerKind::JoyConRight || !controller.connected {
            continue;
        }
        let Some(id) = identity_for(controller.source) else {
            return Err(HalfControllerInputError::MissingIdentity(controller.source));
        };
        sample_storage[sample_count].write(StickDirectionSample {
            id,
            x: controller.axes.right_x,
            y: controller.axes.right_y,
            threshold: 8192,
        });
        sample_count += 1;
    }
    // SAFETY: every entry in `sample_storage[..sample_count]` was initialized
    // above, and the mapper reads only the supplied slice.
    let samples = unsafe {
        core::slice::from_raw_parts(
            sample_storage.as_ptr().cast::<StickDirectionSample>(),
            sample_count,
        )
    };
    let frame = mapper
        .map(samples)
        .map_err(HalfControllerInputError::Stick)?;

    for state in frame.states() {
        let held = state.action_held.to_buttons();
        let down = state.action_down.to_buttons();
        let up = state.action_up.to_buttons();
        input.held |= held;
        input.down |= down;
        input.up |= up;
        if !(held | down | up).is_empty() {
            merge_source(
                &mut source,
                &mut source_ambiguous,
                base_has_contributors,
                state.id,
            );
        }
    }
    for reset in frame.resets() {
        let released = reset.action_released.to_buttons();
        input.up |= released;
        if !released.is_empty() {
            merge_source(
                &mut source,
                &mut source_ambiguous,
                base_has_contributors,
                reset.id,
            );
        }
    }
    input.source = if source_ambiguous { None } else { source };
    Ok(input)
}

pub fn left_half_buttons(
    kind: ControllerKind,
    buttons: Buttons,
    profile: HalfControllerProfile,
) -> Buttons {
    if kind != ControllerKind::JoyConLeft {
        return Buttons::empty();
    }
    let mut mapped = Buttons::empty();
    if buttons.contains(Buttons::SR) {
        mapped |= profile.sr;
    }
    if buttons.contains(Buttons::SL) {
        mapped |= profile.sl;
    }
    if buttons.contains(Buttons::SELECT) {
        mapped |= profile.select;
    }
    mapped
}

#[derive(Clone, Copy)]
struct LeftHalfEvent {
    kind: ControllerKind,
    held: Buttons,
    down: Buttons,
    up: Buttons,
    identity: Option<ControllerId>,
}

fn apply_left_half_actions(
    input: &mut Input,
    source: &mut Option<ControllerId>,
    source_ambiguous: &mut bool,
    base_has_contributors: bool,
    event: LeftHalfEvent,
    profile: HalfControllerProfile,
) {
    let mapped_held = left_half_buttons(event.kind, event.held, profile);
    let mapped_down = left_half_buttons(event.kind, event.down, profile);
    let mapped_up = left_half_buttons(event.kind, event.up, profile);
    input.held |= mapped_held;
    input.down |= mapped_down;
    input.up |= mapped_up;
    if !(mapped_held | mapped_down | mapped_up).is_empty() {
        if let Some(identity) = event.identity {
            merge_source(source, source_ambiguous, base_has_contributors, identity);
        } else {
            *source_ambiguous = true;
        }
    }
    input.source = if *source_ambiguous { None } else { *source };
}

fn merge_source(
    source: &mut Option<ControllerId>,
    ambiguous: &mut bool,
    had_prior_contributor: bool,
    id: ControllerId,
) {
    if *ambiguous {
        return;
    }
    match *source {
        Some(previous) if previous == id => {}
        Some(_) => *ambiguous = true,
        None if had_prior_contributor => *ambiguous = true,
        None => *source = Some(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ogc_controller::{Axes, ControllerSource, ControllerState, StickDirections};

    fn right_snapshot(source: ControllerSource, x: i16) -> ControllerSnapshot {
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(ControllerState {
            source,
            kind: ControllerKind::JoyConRight,
            connected: true,
            buttons: Buttons::empty(),
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: Axes {
                right_x: x,
                ..Axes::default()
            },
        });
        snapshot
    }

    #[test]
    fn empty_snapshot_projection_keeps_timestamp_and_is_neutral() {
        let mut mapper = DigitalStickMapper::<MAX_CONTROLLERS>::new();
        let input = project_wii_half_snapshot(
            ControllerSnapshot::empty(),
            123,
            HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::empty()),
            &mut mapper,
        )
        .unwrap();
        assert_eq!(input.timestamp_ticks, 123);
        assert_eq!(input.held | input.down | input.up, Buttons::empty());
        assert!(!input.connected);
    }

    #[test]
    fn right_half_without_snapshot_identity_returns_an_error() {
        let source = ControllerSource::BluetoothHid(0);
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(ControllerState {
            source,
            kind: ControllerKind::JoyConRight,
            connected: true,
            buttons: Buttons::empty(),
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: Axes {
                right_x: 20_000,
                ..Axes::default()
            },
        });
        let mut mapper = DigitalStickMapper::<MAX_CONTROLLERS>::new();
        let result = project_wii_half_snapshot(
            snapshot,
            124,
            HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::empty()),
            &mut mapper,
        );
        assert!(matches!(
            result,
            Err(HalfControllerInputError::MissingIdentity(error_source))
                if error_source == source
        ));
    }

    #[test]
    fn right_half_projection_reports_gated_direction_edges_and_identity() {
        let source = ControllerSource::BluetoothHid(0);
        let identity = ControllerId {
            source,
            generation: 12,
        };
        let profile = HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::empty());
        let mut mapper = DigitalStickMapper::<MAX_CONTROLLERS>::new();
        let project = |x, mapper: &mut DigitalStickMapper<MAX_CONTROLLERS>| {
            project_wii_half_snapshot_with_identity(
                &right_snapshot(source, x),
                125,
                profile,
                mapper,
                |candidate| (candidate == source).then_some(identity),
            )
            .unwrap()
        };
        assert_eq!(project(20_000, &mut mapper).down, Buttons::empty());
        assert_eq!(project(20_000, &mut mapper).held, Buttons::empty());
        assert_eq!(project(0, &mut mapper).up, Buttons::empty());
        let pressed = project(20_000, &mut mapper);
        assert_eq!(pressed.down, Buttons::RIGHT);
        assert_eq!(pressed.held, Buttons::RIGHT);
        assert_eq!(pressed.source, Some(identity));
        let held = project(20_000, &mut mapper);
        assert_eq!(held.down, Buttons::empty());
        assert_eq!(held.held, Buttons::RIGHT);
        assert_eq!(held.source, Some(identity));
        let released = project(0, &mut mapper);
        assert_eq!(released.up, Buttons::RIGHT);
        assert_eq!(released.source, Some(identity));
    }

    #[test]
    fn left_half_reset_release_keeps_reset_identity_and_button_mapping() {
        let identity = ControllerId {
            source: ControllerSource::BluetoothHid(1),
            generation: 9,
        };
        let mut input = Input::default();
        let mut source = None;
        let mut ambiguous = false;
        apply_left_half_actions(
            &mut input,
            &mut source,
            &mut ambiguous,
            false,
            LeftHalfEvent {
                kind: ControllerKind::JoyConLeft,
                held: Buttons::empty(),
                down: Buttons::empty(),
                up: Buttons::SR,
                identity: Some(identity),
            },
            HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::empty()),
        );
        assert_eq!(input.up, Buttons::A);
        assert_eq!(input.source, Some(identity));
        assert!(!ambiguous);
    }

    #[test]
    fn left_half_profiles_map_only_approved_buttons() {
        let source = Buttons::SR | Buttons::SL | Buttons::SELECT;
        assert_eq!(
            left_half_buttons(
                ControllerKind::JoyConLeft,
                source,
                HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::empty())
            ),
            Buttons::A | Buttons::B
        );
        assert_eq!(
            left_half_buttons(
                ControllerKind::JoyConLeft,
                source,
                HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::START)
            ),
            Buttons::A | Buttons::B | Buttons::START
        );
        assert_eq!(
            left_half_buttons(
                ControllerKind::JoyConLeft,
                source,
                HalfControllerProfile::new(Buttons::A, Buttons::empty(), Buttons::empty())
            ),
            Buttons::A
        );
        assert_eq!(
            left_half_buttons(
                ControllerKind::JoyConRight,
                source,
                HalfControllerProfile::new(Buttons::A, Buttons::B, Buttons::START)
            ),
            Buttons::empty()
        );
    }

    #[test]
    fn right_half_requires_neutral_then_releases_on_disconnect_and_replacement() {
        let first_id = ControllerId {
            source: ControllerSource::BluetoothHid(0),
            generation: 4,
        };
        let replacement_id = ControllerId {
            source: first_id.source,
            generation: 5,
        };
        let sample = |id, x| StickDirectionSample {
            id,
            x,
            y: 0,
            threshold: 8192,
        };
        let mut mapper = DigitalStickMapper::<1>::new();
        let _ = mapper
            .map(core::slice::from_ref(&sample(first_id, 20_000)))
            .unwrap();
        let _ = mapper
            .map(core::slice::from_ref(&sample(first_id, 20_000)))
            .unwrap();
        let neutral = mapper
            .map(core::slice::from_ref(&sample(first_id, 0)))
            .unwrap();
        assert_eq!(
            neutral.states().next().unwrap().action_up,
            StickDirections::EMPTY
        );
        let frame = mapper
            .map(core::slice::from_ref(&sample(first_id, 20_000)))
            .unwrap();
        assert_eq!(
            frame.states().next().unwrap().action_held,
            StickDirections::RIGHT
        );
        let frame = mapper.map(&[]).unwrap();
        assert_eq!(
            frame.resets().next().unwrap().action_released,
            StickDirections::RIGHT
        );
        let _ = mapper
            .map(core::slice::from_ref(&sample(first_id, 0)))
            .unwrap();
        let _ = mapper
            .map(core::slice::from_ref(&sample(first_id, 20_000)))
            .unwrap();
        let frame = mapper
            .map(core::slice::from_ref(&sample(replacement_id, 20_000)))
            .unwrap();
        assert_eq!(frame.resets().next().unwrap().id, first_id);
        let current = frame.states().next().unwrap();
        assert_eq!(current.id, replacement_id);
        assert_eq!(current.action_held, StickDirections::EMPTY);
    }
}
