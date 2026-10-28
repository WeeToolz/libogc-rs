use super::input_model::Input;
use ogc_controller::{Axes, Buttons, ControllerId, ControllerSnapshot};

pub fn project_snapshot(snapshot: ControllerSnapshot, _timestamp_ticks: u64) -> Input {
    project_snapshot_with_contributors(snapshot, _timestamp_ticks).0
}

pub(crate) fn project_snapshot_with_contributors(
    snapshot: ControllerSnapshot,
    _timestamp_ticks: u64,
) -> (Input, bool) {
    project_snapshot_with_identity(snapshot, _timestamp_ticks, |source| {
        snapshot.controller_id(source)
    })
}

pub(crate) fn project_snapshot_with_identity(
    snapshot: ControllerSnapshot,
    _timestamp_ticks: u64,
    mut identity_for: impl FnMut(ogc_controller::ControllerSource) -> Option<ControllerId>,
) -> (Input, bool) {
    let mut attribution = SourceAttribution::default();
    let selected_axes_source = snapshot
        .controllers()
        .iter()
        .position(|controller| controller.connected);
    for (index, controller) in snapshot.controllers().iter().enumerate() {
        attribution.observe_controller(
            controller,
            identity_for(controller.source),
            Some(index) == selected_axes_source,
        );
    }
    for reset in snapshot.resets() {
        attribution.observe_reset(reset.id, reset.state.buttons_down, reset.state.buttons_up);
    }
    let axes_source = snapshot
        .controllers()
        .iter()
        .find(|controller| controller.connected);
    let input = Input {
        held: snapshot.merged_buttons(),
        down: snapshot.merged_buttons_down(),
        up: snapshot.merged_buttons_up(),
        connected: !snapshot.is_empty(),
        axes: axes_source
            .map(|controller| controller.axes)
            .unwrap_or_default(),
        #[cfg(any(feature = "action-trace", test))]
        timestamp_ticks: _timestamp_ticks,
        #[cfg(any(feature = "action-trace", test))]
        timestamp_valid: true,
        source: attribution.identity(),
    };
    (input, attribution.has_contributors())
}

#[derive(Default)]
struct SourceAttribution {
    source: Option<ogc_controller::ControllerSource>,
    identity: Option<ControllerId>,
    ambiguous: bool,
    identity_unknown: bool,
    contributors: bool,
}

impl SourceAttribution {
    fn observe_controller(
        &mut self,
        controller: &ogc_controller::ControllerState,
        identity: Option<ControllerId>,
        selected_axes_source: bool,
    ) {
        let buttons_contribute = controller.connected
            && (!controller.buttons.is_empty()
                || !controller.buttons_down.is_empty()
                || !controller.buttons_up.is_empty());
        let axes_contribute =
            controller.connected && selected_axes_source && controller.axes != Axes::default();
        if buttons_contribute || axes_contribute {
            self.observe(controller.source, identity);
        }
    }

    fn observe_reset(&mut self, identity: ControllerId, down: Buttons, up: Buttons) {
        if !down.is_empty() || !up.is_empty() {
            self.observe(identity.source, Some(identity));
        }
    }

    fn observe(
        &mut self,
        source: ogc_controller::ControllerSource,
        identity: Option<ControllerId>,
    ) {
        self.contributors = true;
        match self.source {
            None => {
                self.source = Some(source);
                self.identity = identity;
                self.identity_unknown = identity.is_none();
            }
            Some(current) if current == source => {
                if let (Some(current), Some(next)) = (self.identity, identity) {
                    if current.generation != next.generation {
                        self.ambiguous = true;
                    }
                } else if self.identity.is_none() && identity.is_some() {
                    self.identity = identity;
                    self.ambiguous |= self.identity_unknown;
                } else if identity.is_none() {
                    self.identity_unknown = true;
                }
            }
            Some(_) => self.ambiguous = true,
        }
    }

    fn has_contributors(&self) -> bool {
        self.contributors
    }

    fn identity(&self) -> Option<ControllerId> {
        (!self.ambiguous && !self.identity_unknown)
            .then_some(self.identity)
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ogc_controller::{ControllerKind, ControllerSource, ControllerState};

    fn state(
        source: ControllerSource,
        buttons: Buttons,
        down: Buttons,
        up: Buttons,
    ) -> ControllerState {
        ControllerState {
            source,
            kind: ControllerKind::GameCube,
            connected: true,
            buttons,
            buttons_down: down,
            buttons_up: up,
            axes: Axes::default(),
        }
    }

    #[test]
    fn projection_keeps_scan_timestamp_edges_and_does_not_guess_missing_identity() {
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            ControllerSource::GameCube(0),
            Buttons::A,
            Buttons::A,
            Buttons::B,
        ));
        let input = project_snapshot(snapshot, 91);
        assert_eq!(input.timestamp_ticks, 91);
        assert_eq!(input.held, Buttons::A);
        assert_eq!(input.down, Buttons::A);
        assert_eq!(input.up, Buttons::B);
        assert_eq!(input.source, None);
        assert!(input.connected);
    }

    #[test]
    fn merged_actions_do_not_claim_one_source() {
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            ControllerSource::GameCube(0),
            Buttons::A,
            Buttons::A,
            Buttons::empty(),
        ));
        snapshot.push(state(
            ControllerSource::UsbHid(0),
            Buttons::B,
            Buttons::empty(),
            Buttons::B,
        ));
        let input = project_snapshot(snapshot, 92);
        assert_eq!(input.held, Buttons::A | Buttons::B);
        assert_eq!(input.down, Buttons::A);
        assert_eq!(input.up, Buttons::B);
        assert_eq!(input.source, None);
    }

    #[test]
    fn unused_axes_from_another_controller_do_not_change_attribution() {
        let mut buttons = state(
            ControllerSource::GameCube(0),
            Buttons::A,
            Buttons::A,
            Buttons::empty(),
        );
        let button_id = ControllerId {
            source: buttons.source,
            generation: 7,
        };
        let mut other_axes = state(
            ControllerSource::UsbHid(0),
            Buttons::empty(),
            Buttons::empty(),
            Buttons::empty(),
        );
        other_axes.axes.left_x = 100;
        let other_id = ControllerId {
            source: other_axes.source,
            generation: 2,
        };
        let mut attribution = SourceAttribution::default();
        attribution.observe_controller(&buttons, Some(button_id), true);
        attribution.observe_controller(&other_axes, Some(other_id), false);
        assert_eq!(attribution.identity(), Some(button_id));
        buttons.axes.left_x = 200;
        let mut axis_attribution = SourceAttribution::default();
        axis_attribution.observe_controller(&buttons, Some(button_id), true);
        axis_attribution.observe_controller(&other_axes, Some(other_id), false);
        assert_eq!(axis_attribution.identity(), Some(button_id));
    }

    #[test]
    fn reset_source_stays_attributed_when_an_unrelated_live_controller_is_neutral() {
        let reset_source = ControllerId {
            source: ControllerSource::GameCube(0),
            generation: 3,
        };
        let live_source = ControllerId {
            source: ControllerSource::UsbHid(0),
            generation: 1,
        };
        let reset_state = state(
            reset_source.source,
            Buttons::empty(),
            Buttons::A,
            Buttons::empty(),
        );
        let neutral_live = state(
            live_source.source,
            Buttons::empty(),
            Buttons::empty(),
            Buttons::empty(),
        );
        let mut attribution = SourceAttribution::default();
        attribution.observe_reset(
            reset_source,
            reset_state.buttons_down,
            reset_state.buttons_up,
        );
        attribution.observe_controller(&neutral_live, Some(live_source), true);
        assert_eq!(attribution.identity(), Some(reset_source));
    }

    #[test]
    fn a_missing_identity_contributor_makes_source_attribution_unknown() {
        let source = ControllerSource::GameCube(0);
        let identity = ControllerId {
            source,
            generation: 3,
        };
        let mut attribution = SourceAttribution::default();
        attribution.observe(source, None);
        attribution.observe(source, Some(identity));
        assert_eq!(attribution.identity(), None);
    }
}
