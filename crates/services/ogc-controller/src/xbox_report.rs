//! Xbox One classic Bluetooth reports. Layout reference: SDL 2.30 xboxone HID driver.
//! https://github.com/libsdl-org/SDL/blob/release-2.30.0/src/joystick/hidapi/SDL_hidapi_xboxone.c
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum XboxInput {
    State {
        buttons: u16,
        hat: u8,
        sticks: [i16; 4],
        triggers: [u8; 2],
    },
    Guide(bool),
}
/// Button bits: A, B, X, Y, L, R, view, menu, left stick, right stick, guide.
pub fn decode(report: &[u8]) -> Option<XboxInput> {
    if report.first() == Some(&2) && report.len() == 2 {
        return Some(XboxInput::Guide(report[1] & 1 != 0));
    }
    if report.first() != Some(&1) || !matches!(report.len(), 16 | 17) {
        return None;
    }
    let mut buttons = 0u16;
    if report.len() == 16 {
        buttons = report[14] as u16 | (((report[15] & 3) as u16) << 8);
    } else {
        for (mask, bit) in [(1, 0), (2, 1), (8, 2), (16, 3), (64, 4), (128, 5)] {
            if report[14] & mask != 0 {
                buttons |= 1 << bit;
            }
        }
        for (mask, bit) in [(8, 7), (32, 8), (64, 9), (16, 10)] {
            if report[15] & mask != 0 {
                buttons |= 1 << bit;
            }
        }
        if report[15] & 4 != 0 || report[16] & 1 != 0 {
            buttons |= 1 << 6;
        }
    }
    let word = |offset| u16::from_le_bytes([report[offset], report[offset + 1]]);
    let x = |offset| (word(offset) as i32 - 32768) as i16;
    let y = |offset| (32768 - word(offset) as i32).clamp(-32768, 32767) as i16;
    let trigger = |offset| ((word(offset).min(1023) as u32 * 255) / 1023) as u8;
    Some(XboxInput::State {
        buttons,
        hat: report[13],
        sticks: [x(1), y(3), x(5), y(7)],
        triggers: [trigger(9), trigger(11)],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn neutral(length: usize) -> [u8; 17] {
        let mut report = [0; 17];
        report[0] = 1;
        for offset in [1, 3, 5, 7] {
            report[offset..offset + 2].copy_from_slice(&32768u16.to_le_bytes());
        }
        if length == 16 {
            report[16] = 0;
        }
        report
    }
    #[test]
    fn old_and_new_button_layouts_match_the_same_semantics() {
        let mut old = neutral(16);
        old[14] = 0xff;
        old[15] = 3;
        let mut new = neutral(17);
        new[14] = 0xdb;
        new[15] = 0x6c;
        new[16] = 1;
        assert!(matches!(
            decode(&old[..16]),
            Some(XboxInput::State {
                buttons: 0x3ff,
                sticks: [0, 0, 0, 0],
                ..
            })
        ));
        assert!(matches!(
            decode(&new),
            Some(XboxInput::State {
                buttons: 0x3ff,
                sticks: [0, 0, 0, 0],
                ..
            })
        ));
    }
    #[test]
    fn axes_triggers_and_hat_use_fixed_offsets() {
        let mut report = neutral(17);
        report[1..3].copy_from_slice(&65535u16.to_le_bytes());
        report[3..5].copy_from_slice(&0u16.to_le_bytes());
        report[9..11].copy_from_slice(&1023u16.to_le_bytes());
        report[13] = 8;
        assert!(matches!(
            decode(&report),
            Some(XboxInput::State {
                sticks: [32767, 32767, 0, 0],
                triggers: [255, 0],
                hat: 8,
                ..
            })
        ));
    }
    #[test]
    fn report_bounds_and_numeric_endpoints_are_checked() {
        for report in [&[][..], &[2][..], &[2, 1, 0][..], &[3, 1][..], &[0; 16][..]] {
            assert_eq!(decode(report), None);
        }
        let mut report = neutral(17);
        report[1..3].copy_from_slice(&[0, 0]);
        report[3..5].copy_from_slice(&[0xff, 0xff]);
        report[5..7].copy_from_slice(&[0xff, 0xff]);
        report[7..9].copy_from_slice(&[0, 0]);
        report[9..11].copy_from_slice(&[0xff, 0xff]);
        report[11..13].copy_from_slice(&[0, 2]);
        assert_eq!(
            decode(&report),
            Some(XboxInput::State {
                buttons: 0,
                hat: 0,
                sticks: [-32768, -32767, 32767, 32767],
                triggers: [255, 127],
            })
        );
    }

    #[test]
    fn guide_and_unknown_reports_do_not_replace_other_input() {
        assert_eq!(decode(&[2, 1]), Some(XboxInput::Guide(true)));
        assert_eq!(decode(&[2, 0]), Some(XboxInput::Guide(false)));
        assert_eq!(decode(&[1; 15]), None);
        assert_eq!(decode(&[1; 18]), None);
    }
}
