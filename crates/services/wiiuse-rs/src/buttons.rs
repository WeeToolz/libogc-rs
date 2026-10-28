//! Wii Remote button report decoding and transition tracking.

/// Mask of the button bits defined by Wiiuse's `WIIMOTE_BUTTON_ALL`.
pub const WIIMOTE_BUTTON_ALL: u16 = 0x1f9f;

/// Button state decoded from one Wii Remote input report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ButtonState {
    /// Buttons held in the previous report.
    pub previous: u16,
    /// Buttons held in this report.
    pub current: u16,
    /// Buttons held in both reports.
    pub held: u16,
    /// Buttons pressed in this report.
    pub pressed: u16,
    /// Buttons released in this report.
    pub released: u16,
}

impl ButtonState {
    /// Decode the two big-endian button bytes from a Wii Remote report.
    ///
    /// `button_bytes` starts at the first button byte, as it does in the
    /// Wiiuse `wiiuse_pressed_buttons` event handler.
    pub fn decode(button_bytes: &[u8], previous: u16) -> Option<Self> {
        let bytes = button_bytes.get(..2)?;
        let current = u16::from_be_bytes([bytes[0], bytes[1]]) & WIIMOTE_BUTTON_ALL;
        Some(Self::from_current(current, previous, WIIMOTE_BUTTON_ALL))
    }

    /// Decode one active-low expansion button byte, such as Nunchuk buttons.
    pub fn decode_active_low(value: u8, mask: u8, previous: u8) -> Self {
        let current = u16::from(!value & mask);
        Self::from_current(current, u16::from(previous), u16::from(mask))
    }

    /// Decode an active-low, big-endian expansion button word.
    pub fn decode_active_low_word(value: u16, mask: u16, previous: u16) -> Self {
        Self::from_current(!value & mask, previous, mask)
    }

    fn from_current(current: u16, previous: u16, mask: u16) -> Self {
        let previous = previous & mask;
        let held = current & previous;
        Self {
            previous,
            current,
            held,
            pressed: current & !previous,
            released: (previous | held) & !current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_big_endian_buttons_and_tracks_pressed_held_and_released() {
        let state = ButtonState::decode(&[0x00, 0x0c], 0x010c).unwrap();
        assert_eq!(
            state,
            ButtonState {
                previous: 0x010c,
                current: 0x000c,
                held: 0x000c,
                pressed: 0,
                released: 0x0100,
            }
        );
    }

    #[test]
    fn masks_non_button_bits_and_rejects_truncated_data() {
        let state = ButtonState::decode(&[0xff, 0xff], 0).unwrap();
        assert_eq!(state.current, WIIMOTE_BUTTON_ALL);
        assert_eq!(state.pressed, WIIMOTE_BUTTON_ALL);
        assert!(ButtonState::decode(&[0x00], 0).is_none());

        let released = ButtonState::decode(&[0, 0], u16::MAX).unwrap();
        assert_eq!(released.previous, WIIMOTE_BUTTON_ALL);
        assert_eq!(released.released, WIIMOTE_BUTTON_ALL);
    }

    #[test]
    fn decodes_active_low_nunchuk_button_transitions() {
        let both_pressed = ButtonState::decode_active_low(0xfc, 0x03, 0x01);
        assert_eq!(both_pressed.current, 0x03);
        assert_eq!(both_pressed.held, 0x01);
        assert_eq!(both_pressed.pressed, 0x02);
        assert_eq!(both_pressed.released, 0);

        let both_released = ButtonState::decode_active_low(0xff, 0x03, 0x03);
        assert_eq!(both_released.current, 0);
        assert_eq!(both_released.released, 0x03);
    }

    #[test]
    fn decodes_active_low_classic_and_guitar_button_word_transitions() {
        let previous = 0x0060;
        let encoded = !(0x0021u16);
        let classic = ButtonState::decode_active_low_word(encoded, 0xfeff, previous);
        let guitar = ButtonState::decode_active_low_word(encoded, 0xfeff, previous);
        let expected = ButtonState {
            previous,
            current: 0x0021,
            held: 0x0020,
            pressed: 0x0001,
            released: 0x0040,
        };
        assert_eq!(classic, expected);
        assert_eq!(guitar, expected);
    }
}
