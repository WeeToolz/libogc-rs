use crate::xbox_report;
use crate::*;
use ogc_bt_sync as bt;

/// Result of parsing a controller HID input report.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MappedControllerReport {
    pub kind: ControllerKind,
    pub buttons: Buttons,
    pub axes: Axes,
}

/// Errors from calibrated Switch report mapping.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwitchReportError {
    UnsupportedKind,
    MalformedReport,
    MissingLeftCalibration,
    MissingRightCalibration,
    InvalidCalibration,
}

/// Stable text for visible startup and structured probe logs.
pub fn bluetooth_stage_text(stage: bt::HostStage) -> &'static str {
    match stage {
        bt::HostStage::Disabled => "BLUETOOTH DISABLED",
        bt::HostStage::AdapterOpen => "BLUETOOTH ADAPTER OPEN",
        bt::HostStage::Reset => "BLUETOOTH HCI RESET",
        bt::HostStage::BufferSize => "BLUETOOTH BUFFER SIZE",
        bt::HostStage::Configure => "BLUETOOTH ADAPTER CONFIGURATION",
        bt::HostStage::Features => "BLUETOOTH ADAPTER FEATURES",
        bt::HostStage::Pairing => "BLUETOOTH PAIRING SETUP",
        bt::HostStage::StoredKeys => "BLUETOOTH STORED KEYS",
        bt::HostStage::AdapterPatch => "BLUETOOTH ADAPTER PATCH",
        bt::HostStage::Address => "BLUETOOTH ADAPTER ADDRESS",
        bt::HostStage::ScanEnable => "BLUETOOTH SCAN ENABLE",
        bt::HostStage::EventFilter => "BLUETOOTH EVENT FILTER",
        bt::HostStage::Ready => "BLUETOOTH READY",
        bt::HostStage::Failed => "BLUETOOTH FAILED",
        bt::HostStage::Closing => "BLUETOOTH CLOSING",
    }
}

/// Detects a Bluetooth controller family from its advertised name and class.
pub fn detect_bluetooth_controller_kind(name: &[u8], class: bt::ClassOfDevice) -> ControllerKind {
    match bt::ControllerFamily::from_name(name) {
        bt::ControllerFamily::JoyConLeft => ControllerKind::JoyConLeft,
        bt::ControllerFamily::JoyConRight => ControllerKind::JoyConRight,
        bt::ControllerFamily::SwitchPro => ControllerKind::SwitchPro,
        bt::ControllerFamily::Xbox => ControllerKind::XboxOne,
        _ if class.likely_game_controller() => ControllerKind::GenericBluetoothHid,
        _ => ControllerKind::Unknown,
    }
}

/// Decode a known Bluetooth controller format without calibration.
///
/// Switch reports require [`map_switch_hid_report_calibrated`] and return
/// `None` here. Unknown families require a descriptor or explicit device
/// driver; this function does not guess one.
pub fn map_bluetooth_hid_report(
    kind: ControllerKind,
    report: &[u8],
) -> Option<MappedControllerReport> {
    match kind {
        ControllerKind::XboxOne => map_xbox_hid_report(report),
        _ => None,
    }
}

/// HORI Pokken Tournament pad (0f0d:0092): no report ID; buttons occupy
/// bytes 0/1, the hat byte 2, and four axis bytes 3..6.
/// Reference: https://github.com/FIX94/Nintendont/issues/240
pub fn map_pokken_hid_report(report: &[u8]) -> Option<MappedControllerReport> {
    if report.len() < 8 {
        return None;
    }
    let mut buttons = Buttons::empty();
    for (mask, button) in [
        (0x01, Buttons::Y),
        (0x02, Buttons::B),
        (0x04, Buttons::A),
        (0x08, Buttons::X),
        (0x10, Buttons::L),
        (0x20, Buttons::R),
        (0x40, Buttons::ZL),
        (0x80, Buttons::ZR),
    ] {
        map_if_u8(&mut buttons, report[0], mask, button);
    }
    map_if_u8(&mut buttons, report[1], 0x01, Buttons::SELECT);
    map_if_u8(&mut buttons, report[1], 0x02, Buttons::START);
    map_if_u8(&mut buttons, report[1], 0x04, Buttons::L_STICK);
    map_if_u8(&mut buttons, report[1], 0x08, Buttons::R_STICK);
    map_if_u8(&mut buttons, report[1], 0x10, Buttons::HOME);
    map_hat(&mut buttons, report[2] & 0x0f);
    Some(MappedControllerReport {
        kind: ControllerKind::GenericUsbHid,
        buttons,
        axes: Axes {
            left_x: normalize_u8_axis(report[3]),
            left_y: invert_axis(normalize_u8_axis(report[4])),
            right_x: normalize_u8_axis(report[5]),
            right_y: invert_axis(normalize_u8_axis(report[6])),
            ..Axes::default()
        },
    })
}

fn map_xbox_hid_report(report: &[u8]) -> Option<MappedControllerReport> {
    let xbox_report::XboxInput::State {
        buttons: raw,
        hat,
        sticks,
        triggers,
    } = xbox_report::decode(report)?
    else {
        return None;
    };
    let mut buttons = Buttons::empty();
    for (bit, button) in [
        Buttons::A,
        Buttons::B,
        Buttons::X,
        Buttons::Y,
        Buttons::L,
        Buttons::R,
        Buttons::SELECT,
        Buttons::START,
        Buttons::L_STICK,
        Buttons::R_STICK,
        Buttons::HOME,
    ]
    .into_iter()
    .enumerate()
    {
        if raw & (1 << bit) != 0 {
            buttons.insert(button);
        }
    }
    if (1..=8).contains(&hat) {
        map_hat(&mut buttons, hat - 1);
    }
    if triggers[0] > 127 {
        buttons.insert(Buttons::ZL);
    }
    if triggers[1] > 127 {
        buttons.insert(Buttons::ZR);
    }
    Some(MappedControllerReport {
        kind: ControllerKind::XboxOne,
        buttons,
        axes: Axes {
            left_x: sticks[0],
            left_y: sticks[1],
            right_x: sticks[2],
            right_y: sticks[3],
            left_trigger: triggers[0],
            right_trigger: triggers[1],
        }
        .clamp_normalized(),
    })
}

/// Map a Switch-family report with validated stick calibration from the host.
/// Single Joy-Con reports use only that Joy-Con's stick and physical controls.
pub fn map_switch_hid_report_calibrated(
    kind: ControllerKind,
    report: &[u8],
    calibration: bt::switch::SwitchCalibration,
) -> Result<MappedControllerReport, SwitchReportError> {
    let (needs_left, needs_right) = match kind {
        ControllerKind::SwitchPro => (true, true),
        ControllerKind::JoyConLeft => (true, false),
        ControllerKind::JoyConRight => (false, true),
        _ => return Err(SwitchReportError::UnsupportedKind),
    };
    let parsed = parse_switch_standard_report(report).ok_or(SwitchReportError::MalformedReport)?;
    let mut axes = Axes::default();
    if needs_left {
        let stick = calibration
            .left
            .ok_or(SwitchReportError::MissingLeftCalibration)?;
        axes.left_x = stick
            .x
            .normalize(parsed.left_stick_x)
            .ok_or(SwitchReportError::InvalidCalibration)?;
        axes.left_y = stick
            .y
            .normalize(parsed.left_stick_y)
            .ok_or(SwitchReportError::InvalidCalibration)?;
    }
    if needs_right {
        let stick = calibration
            .right
            .ok_or(SwitchReportError::MissingRightCalibration)?;
        axes.right_x = stick
            .x
            .normalize(parsed.right_stick_x)
            .ok_or(SwitchReportError::InvalidCalibration)?;
        axes.right_y = stick
            .y
            .normalize(parsed.right_stick_y)
            .ok_or(SwitchReportError::InvalidCalibration)?;
    }
    Ok(map_switch_standard_report(kind, parsed, axes, true))
}

fn map_switch_standard_report(
    kind: ControllerKind,
    parsed: bt::switch::SwitchStandardReport,
    mut axes: Axes,
    filter_single_joycon_controls: bool,
) -> MappedControllerReport {
    let mut right = parsed.right_buttons;
    let mut shared = parsed.shared_buttons;
    let mut left = parsed.left_buttons;
    if filter_single_joycon_controls {
        match kind {
            ControllerKind::JoyConLeft => {
                right = 0;
                shared &= !0x16; // Ignore Plus, right-stick, and Home on a left Joy-Con.
            }
            ControllerKind::JoyConRight => {
                left = 0;
                shared &= !0x29; // Ignore Minus, left-stick, and Capture on a right Joy-Con.
            }
            _ => {}
        }
    }
    let mut buttons = Buttons::empty();

    map_if_u8(&mut buttons, right, 0x01, Buttons::Y);
    map_if_u8(&mut buttons, right, 0x02, Buttons::X);
    map_if_u8(&mut buttons, right, 0x04, Buttons::B);
    map_if_u8(&mut buttons, right, 0x08, Buttons::A);
    map_if_u8(&mut buttons, right, 0x10, Buttons::SR);
    map_if_u8(&mut buttons, right, 0x20, Buttons::SL);
    map_if_u8(&mut buttons, right, 0x40, Buttons::R);
    map_if_u8(&mut buttons, right, 0x80, Buttons::ZR);

    map_if_u8(&mut buttons, shared, 0x01, Buttons::SELECT);
    map_if_u8(&mut buttons, shared, 0x02, Buttons::START);
    map_if_u8(&mut buttons, shared, 0x04, Buttons::R_STICK);
    map_if_u8(&mut buttons, shared, 0x08, Buttons::L_STICK);
    map_if_u8(&mut buttons, shared, 0x10, Buttons::HOME);
    map_if_u8(&mut buttons, shared, 0x20, Buttons::CAPTURE);

    map_if_u8(&mut buttons, left, 0x01, Buttons::DOWN);
    map_if_u8(&mut buttons, left, 0x02, Buttons::UP);
    map_if_u8(&mut buttons, left, 0x04, Buttons::RIGHT);
    map_if_u8(&mut buttons, left, 0x08, Buttons::LEFT);
    map_if_u8(&mut buttons, left, 0x10, Buttons::SR);
    map_if_u8(&mut buttons, left, 0x20, Buttons::SL);
    map_if_u8(&mut buttons, left, 0x40, Buttons::L);
    map_if_u8(&mut buttons, left, 0x80, Buttons::ZL);

    axes.left_trigger = if buttons.contains(Buttons::ZL) {
        255
    } else {
        0
    };
    axes.right_trigger = if buttons.contains(Buttons::ZR) {
        255
    } else {
        0
    };
    MappedControllerReport {
        kind,
        buttons,
        axes,
    }
}

pub(crate) fn map_hat(buttons: &mut Buttons, hat: u8) {
    match hat {
        0 => buttons.insert(Buttons::UP),
        1 => {
            buttons.insert(Buttons::UP);
            buttons.insert(Buttons::RIGHT);
        }
        2 => buttons.insert(Buttons::RIGHT),
        3 => {
            buttons.insert(Buttons::DOWN);
            buttons.insert(Buttons::RIGHT);
        }
        4 => buttons.insert(Buttons::DOWN),
        5 => {
            buttons.insert(Buttons::DOWN);
            buttons.insert(Buttons::LEFT);
        }
        6 => buttons.insert(Buttons::LEFT),
        7 => {
            buttons.insert(Buttons::UP);
            buttons.insert(Buttons::LEFT);
        }
        _ => {}
    }
}

#[cfg(target_arch = "powerpc")]
pub(crate) fn normalize_i8(value: i8) -> i16 {
    (value as i16 * 256).clamp(-32767, 32767)
}

fn normalize_u8_axis(value: u8) -> i16 {
    ((value as i16 - 128) * 256).clamp(-32767, 32767)
}

/// Normalize one byte axis against its reported asymmetric calibration range.
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) fn normalize_calibrated_axis(value: u8, min: u8, center: u8, max: u8) -> i16 {
    let value = i32::from(value);
    let center = i32::from(center);
    let delta = value - center;
    let range = if delta < 0 {
        center - i32::from(min)
    } else {
        i32::from(max) - center
    };
    if range <= 0 {
        return 0;
    }
    (delta * 32767 / range).clamp(-32767, 32767) as i16
}

pub(crate) fn invert_axis(value: i16) -> i16 {
    if value == i16::MIN { i16::MAX } else { -value }
}

pub(crate) fn map_if_u8(buttons: &mut Buttons, raw: u8, mask: u8, button: Buttons) {
    if raw & mask != 0 {
        buttons.insert(button);
    }
}

#[cfg(target_arch = "powerpc")]
pub(crate) fn map_if_u16(buttons: &mut Buttons, raw: u16, mask: u16, button: Buttons) {
    if raw & mask != 0 {
        buttons.insert(button);
    }
}

#[cfg(target_arch = "powerpc")]
pub(crate) fn map_if_u32(buttons: &mut Buttons, raw: u32, mask: u32, button: Buttons) {
    if raw & mask != 0 {
        buttons.insert(button);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_stage_text_identifies_each_lifecycle_stage() {
        for (stage, expected) in [
            (bt::HostStage::Disabled, "BLUETOOTH DISABLED"),
            (bt::HostStage::AdapterOpen, "BLUETOOTH ADAPTER OPEN"),
            (bt::HostStage::Reset, "BLUETOOTH HCI RESET"),
            (bt::HostStage::BufferSize, "BLUETOOTH BUFFER SIZE"),
            (bt::HostStage::Configure, "BLUETOOTH ADAPTER CONFIGURATION"),
            (bt::HostStage::Features, "BLUETOOTH ADAPTER FEATURES"),
            (bt::HostStage::Pairing, "BLUETOOTH PAIRING SETUP"),
            (bt::HostStage::StoredKeys, "BLUETOOTH STORED KEYS"),
            (bt::HostStage::AdapterPatch, "BLUETOOTH ADAPTER PATCH"),
            (bt::HostStage::Address, "BLUETOOTH ADAPTER ADDRESS"),
            (bt::HostStage::ScanEnable, "BLUETOOTH SCAN ENABLE"),
            (bt::HostStage::EventFilter, "BLUETOOTH EVENT FILTER"),
            (bt::HostStage::Ready, "BLUETOOTH READY"),
            (bt::HostStage::Failed, "BLUETOOTH FAILED"),
            (bt::HostStage::Closing, "BLUETOOTH CLOSING"),
        ] {
            assert_eq!(bluetooth_stage_text(stage), expected);
        }
    }

    #[test]
    fn calibrated_axes_use_symmetric_endpoints_from_fixed_samples() {
        assert_eq!(normalize_calibrated_axis(30, 30, 131, 222), -32767);
        assert_eq!(normalize_calibrated_axis(131, 30, 131, 222), 0);
        assert_eq!(normalize_calibrated_axis(222, 30, 131, 222), 32767);
        assert_eq!(normalize_calibrated_axis(0, 126, 126, 126), 0);
    }

    #[test]
    fn pokken_face_buttons_and_hat_use_fixed_offsets() {
        let neutral = [0, 0, 15, 128, 128, 128, 128, 0];
        assert!(map_pokken_hid_report(&neutral).unwrap().buttons.is_empty());
        for (raw, expected) in [
            (1, Buttons::Y),
            (2, Buttons::B),
            (4, Buttons::A),
            (8, Buttons::X),
        ] {
            let mut report = neutral;
            report[0] = raw;
            assert_eq!(map_pokken_hid_report(&report).unwrap().buttons, expected);
        }
        let mut report = neutral;
        report[1] = 2;
        report[2] = 2;
        assert_eq!(
            map_pokken_hid_report(&report).unwrap().buttons,
            Buttons::START | Buttons::RIGHT
        );
        assert!(map_pokken_hid_report(&neutral[..7]).is_none());
    }

    #[test]
    fn uncalibrated_bluetooth_switch_reports_are_rejected() {
        let report = [
            0x30, 0, 0, 0x08, 0x02, 0x02, 0x00, 0x08, 0x80, 0x00, 0x08, 0x80,
        ];
        for kind in [
            ControllerKind::JoyConLeft,
            ControllerKind::JoyConRight,
            ControllerKind::SwitchPro,
        ] {
            assert_eq!(map_bluetooth_hid_report(kind, &report), None);
        }
    }

    #[test]
    fn parses_switch_standard_report_fields() {
        let report = [
            0x30, 0x44, 0x91, 0x08, 0x02, 0x80, 0x34, 0x12, 0xab, 0xcd, 0x3f, 0x12,
        ];
        let parsed = parse_switch_standard_report(&report).unwrap();

        assert_eq!(parsed.report_id, SWITCH_HID_INPUT_STANDARD);
        assert_eq!(parsed.timer, 0x44);
        assert_eq!(parsed.battery_connection, 0x91);
        assert_eq!(parsed.right_buttons, 0x08);
        assert_eq!(parsed.shared_buttons, 0x02);
        assert_eq!(parsed.left_buttons, 0x80);
        assert_eq!(parsed.left_stick_x, 0x234);
        assert_eq!(parsed.left_stick_y, 0xab1);
        assert_eq!(parsed.right_stick_x, 0xfcd);
        assert_eq!(parsed.right_stick_y, 0x123);
    }

    #[test]
    fn parses_switch_subcommand_ack() {
        let mut report = [0u8; 18];
        report[0] = SWITCH_HID_INPUT_SUBCOMMAND_ACK;
        report[13] = 0x80;
        report[14] = SWITCH_SUBCMD_SET_REPORT_MODE;
        report[15..18].copy_from_slice(&[1, 2, 3]);

        let ack = parse_switch_subcommand_ack(&report).unwrap();
        assert!(ack.ack);
        assert_eq!(ack.ack_type, 0);
        assert_eq!(ack.subcommand, SWITCH_SUBCMD_SET_REPORT_MODE);
        assert_eq!(ack.data, &[1, 2, 3]);
    }

    #[test]
    fn calibrated_switch_report_preserves_trigger_mapping() {
        let report = [
            0x30, 0, 0, 0x80, 0x00, 0x80, 0x00, 0x05, 0xa0, 0x00, 0x03, 0x90,
        ];
        let mapped = map_switch_hid_report_calibrated(
            ControllerKind::SwitchPro,
            &report,
            asymmetric_switch_calibration(),
        )
        .unwrap();
        assert!(mapped.buttons.contains(Buttons::ZR));
        assert!(mapped.buttons.contains(Buttons::ZL));
        assert_eq!(mapped.axes.left_trigger, 255);
        assert_eq!(mapped.axes.right_trigger, 255);
    }

    #[test]
    fn known_xbox_bluetooth_report_keeps_its_explicit_mapping() {
        let mut report = [0u8; 17];
        report[0] = 1;
        report[1..3].copy_from_slice(&u16::MAX.to_le_bytes());
        report[3..5].copy_from_slice(&0u16.to_le_bytes());
        report[9..11].copy_from_slice(&1023u16.to_le_bytes());
        report[13] = 8;
        report[14] = 1;
        let mapped = map_bluetooth_hid_report(ControllerKind::XboxOne, &report).unwrap();

        assert_eq!(mapped.kind, ControllerKind::XboxOne);
        assert!(mapped.buttons.contains(Buttons::A));
        assert!(mapped.buttons.contains(Buttons::UP));
        assert_eq!(mapped.axes.left_x, 32767);
        assert_eq!(mapped.axes.left_y, 32767);
        assert_eq!(mapped.axes.left_trigger, 255);
    }

    fn asymmetric_switch_calibration() -> bt::switch::SwitchCalibration {
        use bt::switch::{SwitchAxisCalibration as Axis, SwitchCalibration as Calibration};
        use bt::switch::{SwitchCalibrationSource as Source, SwitchStickCalibration as Stick};
        Calibration {
            left: Some(Stick {
                x: Axis {
                    min: 1000,
                    center: 1200,
                    max: 1800,
                },
                y: Axis {
                    min: 2000,
                    center: 2500,
                    max: 3000,
                },
                source: Source::User,
            }),
            right: Some(Stick {
                x: Axis {
                    min: 500,
                    center: 700,
                    max: 1000,
                },
                y: Axis {
                    min: 2000,
                    center: 2200,
                    max: 2600,
                },
                source: Source::Factory,
            }),
        }
    }

    #[test]
    fn calibrated_switch_report_uses_asymmetric_axes_and_fixed_raw_fixture() {
        // Stick bytes encode L=(0x500,0xa00), R=(0x300,0x900).
        let report = [0x30, 0, 0, 0, 0, 0, 0x00, 0x05, 0xa0, 0x00, 0x03, 0x90];
        let mapped = map_switch_hid_report_calibrated(
            ControllerKind::SwitchPro,
            &report,
            asymmetric_switch_calibration(),
        )
        .unwrap();
        assert_eq!(mapped.axes.left_x, 4368);
        assert_eq!(mapped.axes.left_y, 3932);
        assert_eq!(mapped.axes.right_x, 7427);
        assert_eq!(mapped.axes.right_y, 8519);
    }

    #[test]
    fn calibrated_switch_report_clamps_to_calibrated_endpoints() {
        // L=(0x000,0xfff), R=(0x000,0xfff), outside both configured ranges.
        let report = [0x30, 0, 0, 0, 0, 0, 0x00, 0xf0, 0xff, 0x00, 0xf0, 0xff];
        let mapped = map_switch_hid_report_calibrated(
            ControllerKind::SwitchPro,
            &report,
            asymmetric_switch_calibration(),
        )
        .unwrap();
        assert_eq!(mapped.axes.left_x, -32768);
        assert_eq!(mapped.axes.left_y, 32767);
        assert_eq!(mapped.axes.right_x, -32768);
        assert_eq!(mapped.axes.right_y, 32767);
    }

    #[test]
    fn calibrated_mapping_rejects_missing_or_invalid_required_calibration() {
        let report = [0x30, 0, 0, 0, 0, 0, 0x00, 0x05, 0xa0, 0x00, 0x03, 0x90];
        let mut calibration = asymmetric_switch_calibration();
        calibration.left = None;
        assert_eq!(
            map_switch_hid_report_calibrated(ControllerKind::SwitchPro, &report, calibration),
            Err(SwitchReportError::MissingLeftCalibration)
        );

        let mut calibration = asymmetric_switch_calibration();
        calibration.left.as_mut().unwrap().x.center = 1000;
        assert_eq!(
            map_switch_hid_report_calibrated(ControllerKind::SwitchPro, &report, calibration),
            Err(SwitchReportError::InvalidCalibration)
        );
    }

    #[test]
    fn calibrated_single_joycon_uses_only_its_axes_and_controls() {
        // Both sides report face buttons and shared controls. A left Joy-Con
        // has no right stick, face buttons, Plus, right stick button, or Home.
        let report = [
            0x30, 0, 0, 0x08, 0x1f, 0x02, 0x00, 0x05, 0xa0, 0x00, 0x03, 0x90,
        ];
        let mapped = map_switch_hid_report_calibrated(
            ControllerKind::JoyConLeft,
            &report,
            asymmetric_switch_calibration(),
        )
        .unwrap();
        assert!(mapped.buttons.contains(Buttons::UP));
        assert!(mapped.buttons.contains(Buttons::SELECT));
        assert!(!mapped.buttons.contains(Buttons::A));
        assert!(!mapped.buttons.contains(Buttons::START));
        assert!(!mapped.buttons.contains(Buttons::R_STICK));
        assert!(!mapped.buttons.contains(Buttons::HOME));
        assert_eq!(mapped.axes.left_x, 4368);
        assert_eq!(mapped.axes.right_x, 0);
        assert_eq!(mapped.axes.right_y, 0);

        let mut calibration = asymmetric_switch_calibration();
        calibration.right = None;
        assert!(
            map_switch_hid_report_calibrated(ControllerKind::JoyConLeft, &report, calibration,)
                .is_ok()
        );
    }

    #[test]
    fn rejects_malformed_reports() {
        assert_eq!(
            map_bluetooth_hid_report(ControllerKind::JoyConLeft, &[0x30, 0]),
            None
        );
    }

    #[test]
    fn detects_switch_controller_names() {
        let class = bt::ClassOfDevice::new([0x08, bt::CLASS_MAJOR_PERIPHERAL, 0x00]);
        assert_eq!(
            detect_bluetooth_controller_kind(b"Nintendo Joy-Con (L)", class),
            ControllerKind::JoyConLeft
        );
        assert_eq!(
            detect_bluetooth_controller_kind(b"Pro Controller", class),
            ControllerKind::SwitchPro
        );
        assert_eq!(
            detect_bluetooth_controller_kind(b"8BitDo Pad", class),
            ControllerKind::GenericBluetoothHid
        );
    }

    #[test]
    fn third_party_names_do_not_select_nintendo_protocols() {
        let class = bt::ClassOfDevice::new([0x08, bt::CLASS_MAJOR_PERIPHERAL, 0x00]);
        for name in [
            b"PDP Pro Controller".as_slice(),
            b"Third Party Joy-Con (L)".as_slice(),
            b"Joy-Con (R) clone".as_slice(),
            b"Xbox compatible gamepad".as_slice(),
        ] {
            assert_eq!(
                detect_bluetooth_controller_kind(name, class),
                ControllerKind::GenericBluetoothHid
            );
        }
        assert_eq!(
            detect_bluetooth_controller_kind(b"Joy-Con (R)\0\0", class),
            ControllerKind::JoyConRight
        );
    }

    #[test]
    fn unknown_bluetooth_and_switch_reports_are_not_guessed() {
        assert!(
            map_bluetooth_hid_report(
                ControllerKind::GenericBluetoothHid,
                &[0x83, 0x20, 0, 128, 255, 128]
            )
            .is_none()
        );
        assert!(
            map_bluetooth_hid_report(
                ControllerKind::SwitchPro,
                &[0x3f, 0x41, 0x10, 8, 0, 128, 0, 128, 0, 128, 0, 128]
            )
            .is_none()
        );
    }
}
