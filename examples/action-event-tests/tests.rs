#[path = "../shared/controller_input_types.rs"]
mod input_model;
mod controller_input {
    pub use crate::input_model::Input;
}

#[path = "../action_event/action_event.rs"]
mod action_event;
#[path = "../../probes/ogc-system-probe/src/event_line.rs"]
mod event_line;

#[cfg(test)]
mod tests {
    use super::{
        action_event::{ActionEvent, write_action_event},
        controller_input::Input,
        event_line::{EventLineBuf, source_slots, source_text},
    };
    use ogc_controller::ControllerSource;

    #[test]
    fn action_event_is_valid_json_with_escaped_app_strings_and_unknown_identity() {
        let mut line = EventLineBuf::new();
        let source = ControllerSource::Unknown;
        write_action_event(
            &mut line,
            ActionEvent {
                game: "G\"ame",
                screen: "menu\\screen",
                action: "press\naction",
                frame: 7,
            },
            Input::default(),
            (
                source_text(source),
                source_slots(source).0,
                source_slots(source).1,
            ),
            EventLineBuf::write_json_string,
        )
        .unwrap();

        assert_eq!(
            line.as_str().unwrap(),
            "{\"type\":\"app_action\",\"frame\":7,\"timestamp_ticks\":0,\"timestamp_ticks_valid\":false,\"source\":\"unknown\",\"source_valid\":false,\"slot_a\":255,\"slot_b\":255,\"generation\":0,\"generation_valid\":false,\"down\":0,\"held\":0,\"up\":0,\"left_x\":0,\"left_y\":0,\"right_x\":0,\"right_y\":0,\"connected\":false,\"game\":\"G\\\"ame\",\"screen\":\"menu\\\\screen\",\"action\":\"press\\naction\"}"
        );
    }
}
