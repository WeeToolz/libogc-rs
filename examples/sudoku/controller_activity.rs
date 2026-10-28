use crate::controller_input::Input;

pub fn active(input: Input, last_input_frame: &mut u32, frame: u32) -> bool {
    if !input.held.is_empty() || !input.down.is_empty() {
        *last_input_frame = frame;
    }
    *last_input_frame != u32::MAX && frame.wrapping_sub(*last_input_frame) < 180
}
