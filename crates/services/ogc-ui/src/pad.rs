//! Edge-triggered input adapter for one libogc PAD channel.

use crate::{Direction, UiInput};

/// Converts PAD status snapshots into toolkit input edges.
///
/// Use one adapter per controller channel. Reset it when the controller
/// changes or when the application starts a new input session.
#[derive(Clone, Copy, Debug, Default)]
pub struct PadInputAdapter {
    previous_buttons: u16,
}

impl PadInputAdapter {
    /// Convert one status sample. A disconnected or invalid sample clears the
    /// remembered buttons so the next valid press creates a fresh edge.
    pub fn update(&mut self, status: &ogc_input::PadStatus) -> UiInput {
        if status.err != ogc_input::PAD_ERR_NONE as i8 {
            self.previous_buttons = 0;
            return UiInput::default();
        }

        let pressed = status.button & !self.previous_buttons;
        self.previous_buttons = status.button;
        let direction = if pressed & ogc_input::PAD_BUTTON_UP != 0 {
            Some(Direction::Up)
        } else if pressed & ogc_input::PAD_BUTTON_DOWN != 0 {
            Some(Direction::Down)
        } else if pressed & ogc_input::PAD_BUTTON_LEFT != 0 {
            Some(Direction::Left)
        } else if pressed & ogc_input::PAD_BUTTON_RIGHT != 0 {
            Some(Direction::Right)
        } else {
            None
        };

        UiInput {
            direction,
            activate: pressed & ogc_input::PAD_BUTTON_A != 0,
            cancel: pressed & ogc_input::PAD_BUTTON_B != 0,
            ..UiInput::default()
        }
    }

    /// Forget held buttons. Call after a controller replacement or UI reset.
    pub fn reset(&mut self) {
        self.previous_buttons = 0;
    }
}
