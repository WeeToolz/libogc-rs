//! Shared UI probe engine. The PowerPC monitor feeds it real manager snapshots;
//! host tests call the same mapper and retained UI update path.

extern crate alloc;

use alloc::string::ToString;
use core::fmt::{self, Write};

use ogc_controller::ControllerSource;
#[cfg(test)]
use ogc_ui::Rect;
use ogc_ui::{Canvas, ControllerInputFrame, Layout, Size, Ui, UiError, UiEvent, UiInput, WidgetId};

use crate::event_line::{EventLineBuf as JsonWriter, source_slots, source_text};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiProbeObservation {
    pub mapped: UiInput,
    pub source: Option<ControllerSource>,
    pub labels: Option<ogc_ui::ControllerInputLabels>,
    pub pointer_source: Option<ControllerSource>,
    pub target: Option<WidgetId>,
    pub focus_before: Option<WidgetId>,
    pub event: Option<UiEvent>,
    pub focus_after: Option<WidgetId>,
    pub dialog_open: bool,
    pub scope_changed: bool,
    pub finished: bool,
}

/// Format one UI observation as JSON using the same writer as the live probe.
pub fn write_observation_json(
    output: &mut JsonWriter,
    frame_number: u32,
    observation: &UiProbeObservation,
    identity: Option<ogc_controller::ControllerId>,
    pointer_identity: Option<ogc_controller::ControllerId>,
    metadata: Option<ogc_controller::ControllerMetadata>,
    pointer_metadata: Option<ogc_controller::ControllerMetadata>,
) -> fmt::Result {
    let source_name = observation.source.map_or("unobserved", source_text);
    let pointer_source = observation.pointer_source.map_or("none", source_text);
    let identity_text = identity.map_or_else(
        || "unobserved".to_string(),
        |identity| alloc::format!("{identity:?}"),
    );
    let pointer_identity_text = pointer_identity.map_or_else(
        || "unobserved".to_string(),
        |identity| alloc::format!("{identity:?}"),
    );
    write!(
        output,
        "{{\"type\":\"ui_frame\",\"frame\":{frame_number},\"source\":"
    )?;
    output.write_json_string(source_name)?;
    output.write_str(",\"generation\":")?;
    output.write_json_string(&identity_text)?;
    write!(
        output,
        ",\"source_observed\":{},\"pointer_source\":",
        observation.source.is_some()
    )?;
    output.write_json_string(pointer_source)?;
    output.write_str(",\"pointer_owner\":")?;
    output.write_json_string(pointer_source)?;
    let source_slot_pair = observation.source.map_or((u8::MAX, u8::MAX), source_slots);
    let pointer_slot_pair = observation
        .pointer_source
        .map_or((u8::MAX, u8::MAX), source_slots);
    write!(
        output,
        ",\"source_slots\":[{},{}],\"pointer_slots\":[{},{}],\"pointer_generation\":",
        source_slot_pair.0, source_slot_pair.1, pointer_slot_pair.0, pointer_slot_pair.1
    )?;
    output.write_json_string(&pointer_identity_text)?;
    write!(
        output,
        ",\"pointer_valid_for_frame\":{},\"metadata\":",
        observation.pointer_source.is_some() && observation.mapped.pointer.is_some()
    )?;
    if metadata.is_some() || pointer_metadata.is_some() {
        output.write_str("{\"capabilities\":")?;
        if let Some(capabilities) = metadata.map(|metadata| metadata.capabilities) {
            write!(
                output,
                "{{\"buttons\":{},\"left_stick\":{},\"right_stick\":{},\"triggers\":{},\"pointer\":{},\"motion\":{},\"expansion\":{}}}",
                capabilities.buttons,
                capabilities.left_stick,
                capabilities.right_stick,
                capabilities.triggers,
                capabilities.pointer,
                capabilities.motion,
                capabilities.expansion,
            )?;
        } else {
            output.write_str("null")?;
        }
        output.write_str(",\"pointer_sample\":")?;
        if let Some(pointer) = pointer_metadata.and_then(|metadata| metadata.pointer) {
            let (x, y) = pointer.position();
            write!(
                output,
                "{{\"x\":{x},\"y\":{y},\"resolution\":[{},{}]}}",
                pointer.resolution[0], pointer.resolution[1]
            )?;
        } else {
            output.write_str("null")?;
        }
        output.write_char('}')?;
    } else {
        output.write_str("null")?;
    }
    output.write_str(",\"labels\":")?;
    if let Some(labels) = observation.labels {
        output.write_str("{\"confirm\":")?;
        output.write_json_string(labels.activate)?;
        output.write_str(",\"back\":")?;
        output.write_json_string(labels.cancel)?;
        output.write_str(",\"directions\":")?;
        output.write_json_string(labels.directions)?;
        output.write_str(",\"stick\":")?;
        output.write_json_string(labels.stick)?;
        output.write_char('}')?;
    } else {
        output.write_str("null")?;
    }
    output.write_str(",\"target\":")?;
    write_widget_json(output, observation.target)?;
    output.write_str(",\"focus_before\":")?;
    write_widget_json(output, observation.focus_before)?;
    output.write_str(",\"event\":")?;
    output.write_json_string(
        &observation
            .event
            .map_or_else(|| "none".to_string(), |event| alloc::format!("{event:?}")),
    )?;
    output.write_str(",\"focus_after\":")?;
    write_widget_json(output, observation.focus_after)?;
    output.write_str(",\"input\":")?;
    output.write_json_string(&alloc::format!("{:?}", observation.mapped))?;
    write!(
        output,
        ",\"dialog_open\":{},\"scope_changed\":{}}}",
        observation.dialog_open, observation.scope_changed
    )
}

/// Format a source error with valid JSON null/number cleanup values.
pub fn write_controller_error_json(
    output: &mut JsonWriter,
    error: &ogc_controller::ControllerError,
) -> fmt::Result {
    output.write_str("{\"type\":\"controller_error\",\"source\":")?;
    output.write_json_string(&alloc::format!("{:?}", error.source))?;
    output.write_str(",\"operation\":")?;
    output.write_json_string(error.operation)?;
    output.write_str(",\"failure\":")?;
    output.write_json_string(&alloc::format!("{:?}", error.failure))?;
    output.write_str(",\"cleanup_code\":")?;
    match error.cleanup_code {
        Some(code) => write!(output, "{code}")?,
        None => output.write_str("null")?,
    }
    output.write_char('}')
}

fn write_widget_json(output: &mut impl Write, widget: Option<WidgetId>) -> fmt::Result {
    match widget {
        Some(widget) => write!(output, "{}", widget.index()),
        None => output.write_str("null"),
    }
}

/// Real retained UI tree used by the live probe and host action tests.
pub struct UiInputProbe {
    ui: Ui,
    status: WidgetId,
    list: WidgetId,
    grid_a: WidgetId,
    grid_b: WidgetId,
    disabled: WidgetId,
    open_dialog: WidgetId,
    dialog: WidgetId,
    dialog_accept: WidgetId,
    dialog_cancel: WidgetId,
    finish: WidgetId,
    dialog_open: bool,
    finished: bool,
    last_status: (Option<ogc_controller::ControllerSource>, Option<UiEvent>),
}

impl UiInputProbe {
    pub fn new(viewport: [u16; 2]) -> Result<Self, UiError> {
        let mut ui = Ui::new(viewport[0], viewport[1])?;
        ui.add_label(ui.root(), "CONTROLLER UI INPUT PROBE", Size::fill_width(28))?;
        let status = ui.add_label(
            ui.root(),
            "INPUT: waiting for controller",
            Size::fill_width(32),
        )?;
        ui.add_label(
            ui.root(),
            "MOVE / CONFIRM / BACK / POINTER",
            Size::fill_width(24),
        )?;
        let list = ui.add_list(
            ui.root(),
            &["LIST ONE", "LIST TWO", "LIST THREE"],
            0,
            Size::fill_width(60),
        )?;
        let grid = ui.add_container(ui.root(), Layout::row(8, 0), Size::fill_width(40))?;
        let grid_a = ui.add_button(grid, "GRID A", Size::fill())?;
        let grid_b = ui.add_button(grid, "GRID B", Size::fill())?;
        let disabled = ui.add_button(ui.root(), "DISABLED CONTROL", Size::fill_width(30))?;
        ui.set_enabled(disabled, false)?;
        let open_dialog = ui.add_button(ui.root(), "OPEN DIALOG", Size::fill_width(30))?;
        let finish = ui.add_button(ui.root(), "FINISH COLLECTION", Size::fill_width(30))?;
        let dialog = ui.add_container(ui.root(), Layout::column(4, 4), Size::fill_width(76))?;
        ui.add_label(dialog, "MODAL FOCUS SCOPE", Size::fill_width(20))?;
        let dialog_accept = ui.add_button(dialog, "DIALOG ACCEPT", Size::fill_width(24))?;
        let dialog_cancel = ui.add_button(dialog, "DIALOG BACK", Size::fill_width(24))?;
        ui.set_visible(dialog, false)?;
        Ok(Self {
            ui,
            status,
            list,
            grid_a,
            grid_b,
            disabled,
            open_dialog,
            dialog,
            dialog_accept,
            dialog_cancel,
            finish,
            dialog_open: false,
            finished: false,
            last_status: (None, None),
        })
    }

    /// Apply one mapper output to the real widget tree and report its result.
    pub fn update(&mut self, frame: ControllerInputFrame) -> Result<UiProbeObservation, UiError> {
        let target = frame
            .input
            .pointer
            .and_then(|(x, y)| self.ui.pointer_target(x, y));
        let focus_before = self.ui.focused();
        let event = self.ui.update(frame.input);
        let status = (frame.source, event);
        if status != self.last_status {
            let source = status
                .0
                .map_or("none".to_string(), |source| alloc::format!("{source:?}"));
            let action = status
                .1
                .map_or("none".to_string(), |event| alloc::format!("{event:?}"));
            self.ui.set_text(
                self.status,
                &alloc::format!("SOURCE: {source} ACTION: {action}"),
            )?;
            self.last_status = status;
        }
        let mut scope_changed = false;
        match event {
            Some(UiEvent::Activated(widget)) if widget == self.open_dialog => {
                self.ui.set_visible(self.dialog, true)?;
                self.ui.set_focus_scope(self.dialog)?;
                self.dialog_open = true;
                scope_changed = true;
            }
            Some(UiEvent::Activated(widget))
                if self.dialog_open
                    && (widget == self.dialog_accept || widget == self.dialog_cancel) =>
            {
                self.ui.set_visible(self.dialog, false)?;
                self.ui.set_focus_scope(self.ui.root())?;
                self.dialog_open = false;
                scope_changed = true;
            }
            Some(UiEvent::Cancelled(_)) if self.dialog_open => {
                self.ui.set_visible(self.dialog, false)?;
                self.ui.set_focus_scope(self.ui.root())?;
                self.dialog_open = false;
                scope_changed = true;
            }
            Some(UiEvent::Activated(widget)) if widget == self.finish => {
                self.finished = true;
            }
            _ => {}
        }
        Ok(UiProbeObservation {
            mapped: frame.input,
            source: frame.source,
            labels: frame.labels,
            pointer_source: frame.pointer_source,
            target,
            focus_before,
            event,
            focus_after: self.ui.focused(),
            dialog_open: self.dialog_open,
            scope_changed,
            finished: self.finished,
        })
    }

    #[cfg(test)]
    pub const fn focused(&self) -> Option<WidgetId> {
        self.ui.focused()
    }

    #[cfg(test)]
    pub const fn dialog_open(&self) -> bool {
        self.dialog_open
    }

    pub const fn finished(&self) -> bool {
        self.finished
    }

    /// Draw the same retained tree that receives mapped hardware input.
    pub fn draw(&mut self, canvas: &mut dyn Canvas) -> Result<(), ogc_ui::CanvasError> {
        self.ui.draw(canvas)
    }

    /// Return the current layout bounds for a fixture widget.
    #[cfg(test)]
    pub fn widget_bounds(&mut self, widget: WidgetId) -> Result<Rect, UiError> {
        self.ui.bounds(widget)
    }

    /// Check the production tree hit test at the center of its disabled item.
    pub fn disabled_pointer_is_skipped(&mut self) -> Result<bool, UiError> {
        let bounds = self.ui.bounds(self.disabled)?;
        let x = bounds.x.saturating_add(bounds.width / 2);
        let y = bounds.y.saturating_add(bounds.height / 2);
        Ok(self.ui.pointer_target(x, y) != Some(self.disabled))
    }

    #[cfg(test)]
    pub fn widget_id(&self, name: &str) -> Option<WidgetId> {
        [
            ("list", self.list),
            ("grid_a", self.grid_a),
            ("grid_b", self.grid_b),
            ("disabled", self.disabled),
            ("open_dialog", self.open_dialog),
            ("dialog_accept", self.dialog_accept),
            ("dialog_cancel", self.dialog_cancel),
            ("finish", self.finish),
        ]
        .into_iter()
        .find_map(|(candidate, widget)| (candidate == name).then_some(widget))
    }

    pub fn widget_name(&self, widget: WidgetId) -> Option<&'static str> {
        if widget == self.list {
            Some("list")
        } else if widget == self.grid_a {
            Some("grid_a")
        } else if widget == self.grid_b {
            Some("grid_b")
        } else if widget == self.disabled {
            Some("disabled")
        } else if widget == self.open_dialog {
            Some("open_dialog")
        } else if widget == self.dialog_accept {
            Some("dialog_accept")
        } else if widget == self.dialog_cancel {
            Some("dialog_cancel")
        } else if widget == self.finish {
            Some("finish")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ogc_controller::{Axes, Buttons, ControllerKind, ControllerMetadata, ControllerSnapshot};
    use ogc_ui::{ControllerInputAdapter, Direction};

    fn state(
        source: ControllerSource,
        kind: ControllerKind,
        buttons: u32,
        down: u32,
        axes: Axes,
    ) -> ogc_controller::ControllerState {
        ogc_controller::ControllerState {
            source,
            kind,
            connected: true,
            buttons: Buttons::from_bits(buttons),
            buttons_down: Buttons::from_bits(down),
            buttons_up: Buttons::empty(),
            axes,
        }
    }

    fn frame(source: ControllerSource, buttons: u32, down: u32) -> ControllerSnapshot {
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            source,
            ControllerKind::GameCube,
            buttons,
            down,
            Axes::default(),
        ));
        snapshot
    }

    fn focus_named_widget(probe: &mut UiInputProbe, name: &str) {
        probe.update(ControllerInputFrame::default()).unwrap();
        for _ in 0..16 {
            if probe
                .focused()
                .is_some_and(|widget| probe.widget_name(widget) == Some(name))
            {
                return;
            }
            probe.ui.update(UiInput {
                direction: Some(Direction::Next),
                ..UiInput::default()
            });
        }
        assert_eq!(
            probe.focused().and_then(|widget| probe.widget_name(widget)),
            Some(name)
        );
    }

    #[test]
    fn real_ui_update_opens_and_closes_modal_scope_on_controller_actions() {
        let source = ControllerSource::GameCube(0);
        let mut mapper = ControllerInputAdapter::default();
        let mut probe = UiInputProbe::new([640, 480]).unwrap();
        focus_named_widget(&mut probe, "open_dialog");

        let opening = mapper
            .update(
                &frame(source, Buttons::A.bits(), Buttons::A.bits()),
                [640, 480],
            )
            .unwrap();
        let opened = probe.update(opening).unwrap();
        assert!(matches!(
            opened.event,
            Some(UiEvent::Activated(widget))
                if probe.widget_name(widget) == Some("open_dialog")
        ));
        assert!(opened.scope_changed);
        assert!(opened.dialog_open);
        assert_eq!(opened.focus_after, None);
        mapper.reset();

        let neutral = mapper.update(&frame(source, 0, 0), [640, 480]).unwrap();
        let focused_dialog = probe.update(neutral).unwrap();
        assert_eq!(
            focused_dialog
                .focus_after
                .and_then(|widget| probe.widget_name(widget)),
            Some("dialog_accept")
        );
        let back = mapper
            .update(
                &frame(source, Buttons::B.bits(), Buttons::B.bits()),
                [640, 480],
            )
            .unwrap();
        let closed = probe.update(back).unwrap();
        assert!(matches!(closed.event, Some(UiEvent::Cancelled(_))));
        assert!(closed.scope_changed);
        assert!(!closed.dialog_open);
    }

    #[test]
    fn pointer_action_reports_target_and_modal_focus_after_scope_change() {
        let source = ControllerSource::WiiBluetooth(0);
        let mut probe = UiInputProbe::new([640, 480]).unwrap();
        let bounds = probe
            .widget_bounds(probe.widget_id("open_dialog").unwrap())
            .unwrap();
        let (x, y) = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            source,
            ControllerKind::WiiRemote,
            Buttons::A.bits(),
            Buttons::A.bits(),
            Axes::default(),
        ));
        let metadata = ControllerMetadata {
            capabilities: ogc_controller::ControllerCapabilities {
                pointer: true,
                ..Default::default()
            },
            pointer: ogc_controller::PointerMetadata::new(f32::from(x), f32::from(y), [640, 480]),
        };
        let mut mapper = ControllerInputAdapter::default();
        let mapped = mapper
            .update_with_metadata(&snapshot, [640, 480], |_| Some(metadata))
            .unwrap();
        let target = probe
            .ui
            .pointer_target(x, y)
            .expect("coordinate must hit the open dialog control");
        let observation = probe.update(mapped).unwrap();
        assert_eq!(observation.target, Some(target));
        assert_eq!(observation.event, Some(UiEvent::Activated(target)));
        assert_eq!(observation.focus_after, None);
        assert!(observation.dialog_open);

        let neutral = mapper.update(&snapshot, [640, 480]).unwrap();
        let next = probe.update(neutral).unwrap();
        assert_eq!(
            next.focus_after
                .and_then(|widget| probe.widget_name(widget)),
            Some("dialog_accept")
        );
    }

    #[test]
    fn modal_buttons_remain_visible_and_hittable_at_selected_video_sizes() {
        for viewport in [[640, 480], [640, 400]] {
            let mut probe = UiInputProbe::new(viewport).unwrap();
            let open = probe.widget_id("open_dialog").unwrap();
            let open_bounds = probe.widget_bounds(open).unwrap();
            assert!(open_bounds.y + open_bounds.height <= viewport[1]);
            let open_point = (
                open_bounds.x + open_bounds.width / 2,
                open_bounds.y + open_bounds.height / 2,
            );
            let open_frame = ControllerInputFrame {
                input: UiInput {
                    pointer: Some(open_point),
                    pointer_activate: true,
                    ..UiInput::default()
                },
                source: Some(ControllerSource::WiiBluetooth(0)),
                labels: None,
                pointer_source: Some(ControllerSource::WiiBluetooth(0)),
                released: Default::default(),
            };
            assert_eq!(
                probe.update(open_frame).unwrap().event,
                Some(UiEvent::Activated(open))
            );

            for name in ["dialog_accept", "dialog_cancel"] {
                let widget = probe.widget_id(name).unwrap();
                let bounds = probe.widget_bounds(widget).unwrap();
                assert!(bounds.x + bounds.width <= viewport[0], "{name}: {bounds:?}");
                assert!(
                    bounds.y + bounds.height <= viewport[1],
                    "{name}: {bounds:?}"
                );
                let point = (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
                assert_eq!(probe.ui.pointer_target(point.0, point.1), Some(widget));
            }
        }
    }

    #[test]
    fn directional_input_uses_the_same_real_focus_path() {
        let mut probe = UiInputProbe::new([640, 480]).unwrap();
        let source = ControllerSource::GameCube(0);
        let mut mapper = ControllerInputAdapter::default();
        let initial = mapper.update(&frame(source, 0, 0), [640, 480]).unwrap();
        let first = probe.update(initial).unwrap();
        assert_eq!(
            first
                .focus_after
                .and_then(|widget| probe.widget_name(widget)),
            Some("list")
        );
        let next = mapper
            .update(
                &frame(source, Buttons::DOWN.bits(), Buttons::DOWN.bits()),
                [640, 480],
            )
            .unwrap();
        let observation = probe.update(next).unwrap();
        assert_eq!(observation.focus_after, first.focus_after);
        assert!(matches!(
            observation.event,
            Some(UiEvent::SelectionChanged { .. })
        ));
        assert_eq!(observation.mapped.direction, Some(Direction::Down));
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;
    use ogc_controller::{
        ControllerCapabilities, ControllerId, ControllerMetadata, InputFailure, PointerMetadata,
    };
    use ogc_ui::ControllerInputLabels;
    use serde_json::Value;

    #[test]
    fn shared_trace_source_names_and_slots_match_source_identity() {
        let source = ControllerSource::UsbGameCube {
            adapter_slot: 2,
            port: 3,
        };
        assert_eq!(crate::event_line::source_text(source), "usb_gamecube");
        assert_eq!(crate::event_line::source_slots(source), (11, 3));
    }

    fn fixture_observation(event: Option<UiEvent>, target: WidgetId) -> UiProbeObservation {
        UiProbeObservation {
            mapped: UiInput {
                pointer: Some((17, 23)),
                pointer_activate: true,
                ..UiInput::default()
            },
            source: Some(ControllerSource::WiiBluetooth(0)),
            labels: Some(ControllerInputLabels {
                activate: "A \"confirm\"",
                cancel: "B\\back",
                directions: "D-Pad",
                stick: "Control Stick",
            }),
            pointer_source: Some(ControllerSource::WiiBluetooth(0)),
            target: Some(target),
            focus_before: Some(target),
            event,
            focus_after: Some(target),
            dialog_open: true,
            scope_changed: event.is_some(),
            finished: false,
        }
    }

    fn parse_observation(
        observation: &UiProbeObservation,
        metadata: Option<ControllerMetadata>,
        pointer_metadata: Option<ControllerMetadata>,
    ) -> Value {
        let mut line = crate::event_line::EventLineBuf::new();
        write_observation_json(
            &mut line,
            7,
            observation,
            Some(ControllerId {
                source: ControllerSource::WiiBluetooth(0),
                generation: 9,
            }),
            Some(ControllerId {
                source: ControllerSource::WiiBluetooth(0),
                generation: 9,
            }),
            metadata,
            pointer_metadata,
        )
        .unwrap();
        serde_json::from_str(line.as_str().unwrap()).unwrap()
    }

    #[test]
    fn observation_json_parses_for_every_ui_event_and_optional_metadata() {
        let probe = UiInputProbe::new([640, 480]).unwrap();
        let target = probe.list;
        let observation = |event| fixture_observation(event, target);
        let events = [
            UiEvent::Activated(target),
            UiEvent::Cancelled(target),
            UiEvent::CustomUpdated(target),
            UiEvent::Toggled {
                widget: target,
                checked: true,
            },
            UiEvent::ValueChanged {
                widget: target,
                value: 12,
            },
            UiEvent::SelectionChanged {
                widget: target,
                selected: 3,
            },
            UiEvent::RadioSelected(target),
        ];
        for event in events {
            let value = parse_observation(&observation(Some(event)), None, None);
            assert_eq!(value["type"], "ui_frame");
            assert_eq!(value["source_observed"], true);
            assert_eq!(
                value["generation"],
                "ControllerId { source: WiiBluetooth(0), generation: 9 }"
            );
            assert_eq!(value["metadata"], Value::Null);
            assert_eq!(value["labels"]["confirm"], "A \"confirm\"");
            assert_eq!(value["labels"]["back"], "B\\back");
            assert_eq!(value["target"], target.index());
            assert!(value["event"].as_str().is_some());
            assert!(
                value["input"]
                    .as_str()
                    .unwrap()
                    .contains("pointer_activate: true")
            );
        }

        let metadata = ControllerMetadata {
            capabilities: ControllerCapabilities {
                pointer: true,
                left_stick: true,
                ..ControllerCapabilities::default()
            },
            pointer: PointerMetadata::new(10.5, 20.25, [640, 480]),
        };
        let source_metadata = ControllerMetadata {
            capabilities: ControllerCapabilities {
                buttons: true,
                ..ControllerCapabilities::default()
            },
            pointer: None,
        };
        let value = parse_observation(&observation(None), Some(source_metadata), Some(metadata));
        assert_eq!(value["metadata"]["capabilities"]["buttons"], true);
        assert_eq!(value["metadata"]["capabilities"]["pointer"], false);
        assert_eq!(value["metadata"]["pointer_sample"]["x"], 10.5);
        assert_eq!(value["metadata"]["pointer_sample"]["y"], 20.25);
        assert_eq!(value["metadata"]["pointer_sample"]["resolution"][0], 640);
        assert_eq!(value["metadata"]["pointer_sample"]["resolution"][1], 480);
    }

    #[test]
    fn controller_error_json_escapes_strings_and_uses_null_or_numeric_cleanup() {
        let failure = InputFailure::Source(-7);
        for cleanup_code in [None, Some(-2)] {
            let error = ogc_controller::ControllerError {
                source: ControllerSource::WiiBluetooth(0),
                operation: "operation \"failed\"\n",
                failure,
                cleanup_code,
            };
            let mut line = crate::event_line::EventLineBuf::new();
            write_controller_error_json(&mut line, &error).unwrap();
            let value: Value = serde_json::from_str(line.as_str().unwrap()).unwrap();
            assert_eq!(value["operation"], "operation \"failed\"\n");
            assert_eq!(
                value["cleanup_code"],
                cleanup_code.map_or(Value::Null, Value::from)
            );
            assert!(value["failure"].as_str().unwrap().contains("Source(-7)"));
        }
    }
}
