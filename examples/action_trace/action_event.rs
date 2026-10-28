use core::fmt::{self, Display, Write};

use crate::controller_input::Input;

pub struct ActionEvent<'a, F> {
    pub game: &'a str,
    pub screen: &'a str,
    pub action: &'a str,
    pub frame: F,
}

/// Write one application action record into the production bounded JSON line.
pub fn write_action_event<W: Write, F: Display>(
    output: &mut W,
    event: ActionEvent<'_, F>,
    input: Input,
    source: (&'static str, u8, u8),
    write_json_string: fn(&mut W, &str) -> fmt::Result,
) -> fmt::Result {
    let (source_text, slot_a, slot_b) = source;
    let source_valid = input.source.is_some();
    write!(
        output,
        "{{\"type\":\"app_action\",\"frame\":{},\"timestamp_ticks\":{},\"timestamp_ticks_valid\":{},\"source\":\"{source_text}\",\"source_valid\":{source_valid},\"slot_a\":{slot_a},\"slot_b\":{slot_b},\"generation\":{},\"generation_valid\":{},\"down\":{},\"held\":{},\"up\":{},\"left_x\":{},\"left_y\":{},\"right_x\":{},\"right_y\":{},\"connected\":{},\"game\":",
        event.frame,
        input.timestamp_ticks,
        input.timestamp_valid,
        input
            .source
            .map(|identity| identity.generation)
            .unwrap_or(0),
        input.source.is_some(),
        input.down.bits(),
        input.held.bits(),
        input.up.bits(),
        input.axes.left_x,
        input.axes.left_y,
        input.axes.right_x,
        input.axes.right_y,
        input.connected,
    )?;
    write_json_string(output, event.game)?;
    output.write_str(",\"screen\":")?;
    write_json_string(output, event.screen)?;
    output.write_str(",\"action\":")?;
    write_json_string(output, event.action)?;
    output.write_char('}')
}
