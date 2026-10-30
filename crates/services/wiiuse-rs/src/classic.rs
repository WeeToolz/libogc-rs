//! Classic Controller calibration decoding.

use crate::ButtonState;

/// Active button bits in a Classic Controller report.
pub const CLASSIC_BUTTON_ALL_MASK: u16 = 0xfeff;
/// Wii U Pro R trigger button bit.
pub const CLASSIC_BUTTON_FULL_R: u16 = 0x0200;
/// Wii U Pro L trigger button bit.
pub const CLASSIC_BUTTON_FULL_L: u16 = 0x2000;

/// Stick calibration and controller type from one Classic Controller read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassicCalibration {
    /// Controller type: 0 for original, 1 for Pro, and 2 for Wii U Pro.
    pub type_: u8,
    /// Left stick maximum values, X then Y.
    pub left_max: [u8; 2],
    /// Left stick minimum values, X then Y.
    pub left_min: [u8; 2],
    /// Left stick center values, X then Y.
    pub left_center: [u8; 2],
    /// Right stick maximum values, X then Y.
    pub right_max: [u8; 2],
    /// Right stick minimum values, X then Y.
    pub right_min: [u8; 2],
    /// Right stick center values, X then Y.
    pub right_center: [u8; 2],
}

impl ClassicCalibration {
    /// Decode libogc Wiiuse's 224-byte Classic Controller calibration read.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let data = data.get(..224)?;
        if data[223] == 0x20 {
            return Some(Self {
                type_: 2,
                left_max: [208; 2],
                left_min: [48; 2],
                left_center: [128; 2],
                right_max: [208; 2],
                right_min: [48; 2],
                right_center: [128; 2],
            });
        }

        let offset = if data[0] == 0xff && data[16] != 0xff {
            16
        } else {
            0
        };
        let scaled = &data[offset..offset + 12];
        let left_max = [nonzero(scaled[0] / 4, 64), nonzero(scaled[3] / 4, 64)];
        let left_min = [scaled[1] / 4, scaled[4] / 4];
        let left_center = [nonzero(scaled[2] / 4, 32), nonzero(scaled[5] / 4, 32)];
        let right_max = [nonzero(scaled[6] / 8, 32), nonzero(scaled[9] / 8, 32)];
        let right_min = [scaled[7] / 8, scaled[10] / 8];
        let right_center = [nonzero(scaled[8] / 8, 16), nonzero(scaled[11] / 8, 16)];

        let (left_min, left_max, left_center) =
            fix_bad_calibration_values(left_min, left_max, left_center, 64, 32);
        let (right_min, right_max, right_center) =
            fix_bad_calibration_values(right_min, right_max, right_center, 32, 16);

        Some(Self {
            type_: u8::from(data[218] != 0),
            left_max,
            left_min,
            left_center,
            right_max,
            right_min,
            right_center,
        })
    }
}

/// Decoded values from a standard Classic Controller or Wii U Pro report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClassicReport {
    /// Current active button bits. Wii U Pro stick buttons use bits 16 and 17.
    pub buttons: u32,
    /// Previous active button bits.
    pub buttons_last: u32,
    /// Buttons held in both this and the previous report.
    pub buttons_held: u32,
    /// Buttons released in this report.
    pub buttons_released: u32,
    /// Left stick position, X then Y.
    pub left_stick: [u8; 2],
    /// Right stick position, X then Y.
    pub right_stick: [u8; 2],
    /// Left trigger value, from 0 to 31.
    pub left_trigger: u8,
    /// Right trigger value, from 0 to 31.
    pub right_trigger: u8,
    /// True when a Wii U Pro controller is charging.
    pub charging: bool,
    /// True when a Wii U Pro controller uses a wired connection.
    pub wired: bool,
    /// Wii U Pro battery level, from 0 to 7.
    pub battery: u8,
}

impl ClassicReport {
    /// Decode a Classic Controller report and its button transitions.
    ///
    /// `wii_u_pro` selects the eleven-byte Wii U Pro layout. The standard
    /// Classic Controller layout uses six bytes.
    pub fn decode(data: &[u8], wii_u_pro: bool, previous_buttons: u32) -> Option<Self> {
        let button_mask =
            u32::from(CLASSIC_BUTTON_ALL_MASK) | if wii_u_pro { 0x0003_0000 } else { 0 };
        let previous_buttons = previous_buttons & button_mask;
        let (
            encoded_buttons,
            encoded_extra_buttons,
            left_stick,
            right_stick,
            left_trigger,
            right_trigger,
            charging,
            wired,
            battery,
        ) = if wii_u_pro {
            let data = data.get(..11)?;
            let encoded_buttons = u16::from_be_bytes([data[8], data[9]]);
            let active_classic_buttons = !encoded_buttons & CLASSIC_BUTTON_ALL_MASK;
            (
                encoded_buttons,
                data[10],
                [
                    ((data[0] >> 4) | (data[1] << 4)),
                    ((data[4] >> 4) | (data[5] << 4)),
                ],
                [
                    ((data[2] >> 4) | (data[3] << 4)),
                    ((data[6] >> 4) | (data[7] << 4)),
                ],
                if active_classic_buttons & CLASSIC_BUTTON_FULL_L != 0 {
                    0x1f
                } else {
                    0
                },
                if active_classic_buttons & CLASSIC_BUTTON_FULL_R != 0 {
                    0x1f
                } else {
                    0
                },
                data[10] & 0x04 == 0,
                data[10] & 0x08 == 0,
                (data[10] & 0x70) >> 4,
            )
        } else {
            let data = data.get(..6)?;
            (
                u16::from_be_bytes([data[4], data[5]]),
                0xff,
                [data[0] & 0x3f, data[1] & 0x3f],
                [
                    ((data[0] & 0xc0) >> 3) | ((data[1] & 0xc0) >> 5) | ((data[2] & 0x80) >> 7),
                    data[2] & 0x1f,
                ],
                ((data[2] & 0x60) >> 2) | ((data[3] & 0xe0) >> 5),
                data[3] & 0x1f,
                false,
                false,
                0,
            )
        };

        let classic = ButtonState::decode_active_low_word(
            encoded_buttons,
            CLASSIC_BUTTON_ALL_MASK,
            previous_buttons as u16,
        );
        let extra = if wii_u_pro {
            ButtonState::decode_active_low(
                encoded_extra_buttons,
                0x03,
                (previous_buttons >> 16) as u8,
            )
        } else {
            ButtonState::default()
        };
        Some(Self {
            buttons: u32::from(classic.current) | (u32::from(extra.current) << 16),
            buttons_last: previous_buttons,
            buttons_held: u32::from(classic.held) | (u32::from(extra.held) << 16),
            buttons_released: u32::from(classic.released) | (u32::from(extra.released) << 16),
            left_stick,
            right_stick,
            left_trigger,
            right_trigger,
            charging,
            wired,
            battery,
        })
    }

    /// Return combined WPAD button bits for a decoded report.
    pub const fn wpad_buttons(self, wii_u_pro: bool) -> u32 {
        if wii_u_pro {
            ((self.buttons >> 16) & 0x0003)
                | ((self.buttons & CLASSIC_BUTTON_ALL_MASK as u32) << 16)
        } else {
            (self.buttons & CLASSIC_BUTTON_ALL_MASK as u32) << 16
        }
    }
}

fn nonzero(value: u8, fallback: u8) -> u8 {
    if value == 0 { fallback } else { value }
}

fn fix_bad_calibration_values(
    mut min: [u8; 2],
    mut max: [u8; 2],
    mut center: [u8; 2],
    default_max: u8,
    default_center: u8,
) -> ([u8; 2], [u8; 2], [u8; 2]) {
    for axis in 0..2 {
        if min[axis] >= center[axis] || max[axis] <= center[axis] {
            min[axis] = 0;
            max[axis] = default_max;
            center[axis] = default_center;
        }
    }
    (min, max, center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_scaled_classic_stick_calibration() {
        let mut data = [0; 224];
        data[..12].copy_from_slice(&[252, 24, 128, 252, 24, 128, 248, 8, 128, 248, 8, 128]);
        data[218] = 1;
        let calibration = ClassicCalibration::decode(&data).unwrap();
        assert_eq!(calibration.type_, 1);
        assert_eq!(calibration.left_max, [63, 63]);
        assert_eq!(calibration.left_min, [6, 6]);
        assert_eq!(calibration.left_center, [32, 32]);
        assert_eq!(calibration.right_max, [31, 31]);
        assert_eq!(calibration.right_min, [1, 1]);
        assert_eq!(calibration.right_center, [16, 16]);
    }

    #[test]
    fn uses_offset_sixteen_for_prefixed_classic_calibration() {
        let mut data = [0; 224];
        data[0] = 0xff;
        data[16..28].copy_from_slice(&[252, 24, 128, 252, 24, 128, 248, 8, 128, 248, 8, 128]);
        let calibration = ClassicCalibration::decode(&data).unwrap();
        assert_eq!(calibration.left_max, [63, 63]);
        assert_eq!(calibration.right_max, [31, 31]);
    }

    #[test]
    fn applies_invalid_range_defaults_and_wii_u_pro_calibration() {
        let invalid = ClassicCalibration::decode(&[0; 224]).unwrap();
        assert_eq!(invalid.left_min, [0, 0]);
        assert_eq!(invalid.left_max, [64, 64]);
        assert_eq!(invalid.left_center, [32, 32]);
        assert_eq!(invalid.right_min, [0, 0]);
        assert_eq!(invalid.right_max, [32, 32]);
        assert_eq!(invalid.right_center, [16, 16]);

        let mut pro = [0; 224];
        pro[223] = 0x20;
        let calibration = ClassicCalibration::decode(&pro).unwrap();
        assert_eq!(calibration.type_, 2);
        assert_eq!(calibration.left_max, [208, 208]);
        assert_eq!(calibration.left_min, [48, 48]);
        assert_eq!(calibration.left_center, [128, 128]);
        assert_eq!(calibration.right_max, [208, 208]);
        assert_eq!(calibration.right_min, [48, 48]);
        assert_eq!(calibration.right_center, [128, 128]);
    }

    #[test]
    fn rejects_short_classic_calibration_reads() {
        assert!(ClassicCalibration::decode(&[0; 223]).is_none());
    }

    #[test]
    fn decodes_standard_classic_sticks_triggers_and_buttons() {
        let data = [0xc5, 0xd6, 0xa7, 0xe3, 0xf3, 0x7f];
        let report = ClassicReport::decode(&data, false, 0).unwrap();
        assert_eq!(report.left_stick, [5, 22]);
        assert_eq!(report.right_stick, [31, 7]);
        assert_eq!(report.left_trigger, 15);
        assert_eq!(report.right_trigger, 3);
        assert_eq!(report.charging, false);
        assert_eq!(report.wired, false);
        assert_eq!(report.battery, 0);
        assert_eq!(report.buttons, 0x0c80);
        assert_eq!(report.wpad_buttons(false), report.buttons << 16);
        let with_invalid_previous = ClassicReport::decode(&data, false, u32::MAX).unwrap();
        assert_eq!(with_invalid_previous.buttons_last, 0x0000_feff);
        assert!(ClassicReport::decode(&data[..5], false, 0).is_none());
    }

    #[test]
    fn decodes_wii_u_pro_sticks_buttons_and_status() {
        let data = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xff, 0xff, 0x50,
        ];
        let report = ClassicReport::decode(&data, true, 0).unwrap();
        assert_eq!(report.left_stick, [0x41, 0xc9]);
        assert_eq!(report.right_stick, [0x85, 0x0d]);
        assert_eq!(report.charging, true);
        assert_eq!(report.wired, true);
        assert_eq!(report.battery, 5);
        assert_eq!(report.buttons & 0x0003_0000, 0x0003_0000);
        assert_eq!(report.wpad_buttons(true) & 3, 3);
        let with_invalid_previous = ClassicReport::decode(&data, true, u32::MAX).unwrap();
        assert_eq!(with_invalid_previous.buttons_last, 0x0003_feff);
        assert!(ClassicReport::decode(&data[..10], true, 0).is_none());
    }

    #[test]
    fn wpad_buttons_pack_classic_and_pro_bits_with_fixed_values() {
        let classic = ClassicReport::decode(&[0; 6], false, 0).unwrap();
        assert_eq!(classic.wpad_buttons(false), 0xfeff_0000);
        let pro = ClassicReport::decode(&[0; 11], true, 0).unwrap();
        assert_eq!(pro.wpad_buttons(true), 0xfeff_0003);
        assert_eq!(pro.wpad_buttons(false), 0xfeff_0000);
        let released = ClassicReport::decode(&[0xff; 11], true, 0).unwrap();
        assert_eq!(released.wpad_buttons(true), 0);
    }

    #[test]
    fn tracks_wii_u_pro_extra_button_edges() {
        let mut data = [0xff; 11];
        data[8..10].copy_from_slice(&u16::MAX.to_be_bytes());
        data[10] = 0xfc;
        let first = ClassicReport::decode(&data, true, 0).unwrap();
        assert_eq!(first.buttons & 0x0003_0000, 0x0003_0000);
        assert_eq!(first.buttons_held, 0);
        assert_eq!(first.buttons_released, 0);

        data[10] = 0xfd;
        let next = ClassicReport::decode(&data, true, first.buttons).unwrap();
        assert_eq!(next.buttons & 0x0003_0000, 0x0002_0000);
        assert_eq!(next.buttons_held & 0x0003_0000, 0x0002_0000);
        assert_eq!(next.buttons_released & 0x0003_0000, 0x0001_0000);
    }
}
