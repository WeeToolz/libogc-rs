//! Guitar Hero 3 touch bar and whammy report decoding.

use crate::ButtonState;

/// Raw fields, button transitions, and analog values from one guitar report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuitarHero3Report {
    /// Current, previous, pressed, held, and released button masks.
    pub buttons: ButtonState,
    /// Six-bit stick position, X then Y.
    pub stick: [u8; 2],
    /// Five-bit touch-bar reading.
    pub touch_raw: u8,
    /// Five-bit whammy-bar reading.
    pub whammy_raw: u8,
    /// Converted touch-bar and whammy-bar values.
    pub analog: GuitarHero3AnalogState,
}

impl GuitarHero3Report {
    /// Decode the six-byte libogc Wiiuse guitar expansion payload.
    ///
    /// Reject a short payload before reading any fields. Previous button bits
    /// are restricted to the guitar mask, as in the shared button decoder.
    pub fn decode(data: &[u8], previous_buttons: u16) -> Option<Self> {
        let data = data.get(..6)?;
        let touch_raw = data[2] & 0x1f;
        let whammy_raw = data[3] & 0x1f;
        Some(Self {
            buttons: ButtonState::decode_active_low_word(
                u16::from_be_bytes([data[4], data[5]]),
                0xfeff,
                previous_buttons,
            ),
            stick: [data[0] & 0x3f, data[1] & 0x3f],
            touch_raw,
            whammy_raw,
            analog: GuitarHero3AnalogState::decode(touch_raw, whammy_raw),
        })
    }
}

/// Decoded analog state from one Guitar Hero 3 expansion report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GuitarHero3AnalogState {
    /// Wiiuse touch bar code, including its `0x1000` availability marker.
    pub touch_bar: u16,
    /// Wiiuse whammy value. This value is not clamped to the 0 to 1 range.
    pub whammy_bar: f32,
}

impl GuitarHero3AnalogState {
    /// Decode the touch bar and whammy bytes from their five-bit report values.
    pub fn decode(touch: u8, whammy: u8) -> Self {
        let touch = touch & 0x1f;
        let whammy = whammy & 0x1f;
        let touch_bar = if touch > 0x1b {
            0x1010
        } else if touch > 0x18 {
            0x1018
        } else if touch > 0x15 {
            0x1008
        } else if touch > 0x13 {
            0x100c
        } else if touch > 0x10 {
            0x1004
        } else if touch > 0x0d {
            0x1000
        } else if touch > 0x0b {
            0x1006
        } else if touch > 0x08 {
            0x1002
        } else if touch > 0x05 {
            0x1003
        } else if touch > 0x02 {
            0x1001
        } else {
            0
        };
        Self {
            touch_bar,
            whammy_bar: (f32::from(whammy) - 0x0f as f32) / (0x1a - 0x0f) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_complete_guitar_report_with_independent_expected_values() {
        let report = GuitarHero3Report::decode(&[0xc5, 0xd6, 0xee, 0xee, 0xff, 0xde], 0x0060)
            .expect("complete guitar payload");
        assert_eq!(report.stick, [5, 22]);
        assert_eq!(report.touch_raw, 14);
        assert_eq!(report.whammy_raw, 14);
        assert_eq!(report.buttons.previous, 0x0060);
        assert_eq!(report.buttons.current, 0x0021);
        assert_eq!(report.buttons.held, 0x0020);
        assert_eq!(report.buttons.pressed, 0x0001);
        assert_eq!(report.buttons.released, 0x0040);
        assert_eq!(report.analog.touch_bar, 0x1000);
        assert_eq!(report.analog.whammy_bar, -1.0 / 11.0);
    }

    #[test]
    fn guitar_report_rejects_short_payloads_and_masks_unused_button_bits() {
        for length in 0..6 {
            assert!(GuitarHero3Report::decode(&[0xff; 6][..length], 0).is_none());
        }
        let released = GuitarHero3Report::decode(&[0xff; 6], 0xffff).unwrap();
        assert_eq!(released.buttons.previous, 0xfeff);
        assert_eq!(released.buttons.current, 0);
        assert_eq!(released.buttons.released, 0xfeff);
        let unused = GuitarHero3Report::decode(&[0, 0, 0, 0, 0xfe, 0xff], 0).unwrap();
        assert_eq!(unused.buttons.current, 0);
    }

    #[test]
    fn maps_touch_bar_ranges_to_wiiuse_codes() {
        for (raw, expected) in [
            (0, 0x0000),
            (3, 0x1001),
            (6, 0x1003),
            (9, 0x1002),
            (12, 0x1006),
            (14, 0x1000),
            (17, 0x1004),
            (20, 0x100c),
            (22, 0x1008),
            (25, 0x1018),
            (28, 0x1010),
        ] {
            assert_eq!(GuitarHero3AnalogState::decode(raw, 15).touch_bar, expected);
        }
    }

    #[test]
    fn preserves_c_whammy_arithmetic_below_and_above_calibration_range() {
        let step = 1.0 / 11.0;
        assert_eq!(GuitarHero3AnalogState::decode(0, 14).whammy_bar, -step);
        assert_eq!(GuitarHero3AnalogState::decode(0, 15).whammy_bar, 0.0);
        assert_eq!(GuitarHero3AnalogState::decode(0, 26).whammy_bar, 1.0);
        assert_eq!(
            GuitarHero3AnalogState::decode(0, 31).whammy_bar,
            16.0 / 11.0
        );
    }
}
