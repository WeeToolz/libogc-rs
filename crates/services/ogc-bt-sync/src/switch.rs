//! Shared Switch HID packet formats. All callers use these bytes.
use crate as bt;

pub const SWITCH_HID_INPUT_SUBCOMMAND_ACK: u8 = 0x21;
pub const SWITCH_HID_INPUT_STANDARD: u8 = 0x30;
pub const SWITCH_HID_OUTPUT_SUBCOMMAND: u8 = 0x01;
pub const SWITCH_HID_OUTPUT_RUMBLE_ONLY: u8 = 0x10;
pub const SWITCH_SUBCMD_SET_REPORT_MODE: u8 = 0x03;
pub const SWITCH_SUBCMD_LOW_POWER_MODE: u8 = 0x08;
pub const SWITCH_SUBCMD_SET_PLAYER_LIGHTS: u8 = 0x30;
pub const SWITCH_SUBCMD_ENABLE_IMU: u8 = 0x40;
pub const SWITCH_SUBCMD_ENABLE_RUMBLE: u8 = 0x48;
/// SPI layout and 12-bit stick packing follow the Linux driver at
/// <https://github.com/torvalds/linux/blob/master/drivers/hid/hid-nintendo.c>,
/// with reverse-engineering notes at
/// <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/spi_flash_notes.md>.
/// libogc has no Switch protocol C source; these are implementation references,
/// not Nintendo specifications.
pub const SWITCH_SUBCMD_SPI_FLASH_READ: u8 = 0x10;
pub const SWITCH_REPORT_MODE_STANDARD_FULL: u8 = 0x30;
pub const SWITCH_USER_CALIBRATION_ADDRESS: u32 = 0x8010;
pub const SWITCH_USER_CALIBRATION_LENGTH: u8 = 22;
pub const SWITCH_FACTORY_CALIBRATION_ADDRESS: u32 = 0x603d;
pub const SWITCH_FACTORY_CALIBRATION_LENGTH: u8 = 18;
pub const SWITCH_USER_CALIBRATION_MAGIC: [u8; 2] = [0xb2, 0xa1];
pub const SWITCH_NEUTRAL_RUMBLE: [u8; 8] = [0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40];
pub const SWITCH_SUBCOMMAND_OUTPUT_BYTES: usize = 64;
pub const SWITCH_RUMBLE_OUTPUT_BYTES: usize = 10;
pub const SWITCH_HIDP_OUTPUT_HEADER: u8 = bt::HIDP_TRANS_DATA | bt::HIDP_DATA_RTYPE_OUTPUT;
pub const SWITCH_HIDP_INPUT_HEADER: u8 = bt::HIDP_TRANS_DATA | bt::HIDP_DATA_RTYPE_INPUT;

/// Parsed raw Switch standard input report fields.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchStandardReport {
    pub report_id: u8,
    pub timer: u8,
    pub battery_connection: u8,
    pub right_buttons: u8,
    pub shared_buttons: u8,
    pub left_buttons: u8,
    pub left_stick_x: u16,
    pub left_stick_y: u16,
    pub right_stick_x: u16,
    pub right_stick_y: u16,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwitchCalibrationSource {
    User,
    Factory,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchAxisCalibration {
    pub min: u16,
    pub center: u16,
    pub max: u16,
}

impl SwitchAxisCalibration {
    fn new(center: u16, below: u16, above: u16) -> Option<Self> {
        let min = center.checked_sub(below)?;
        let max = center.checked_add(above)?;
        (min < center && center < max && max <= 4095).then_some(Self { min, center, max })
    }

    /// Map a raw 12-bit stick value to signed full-scale using its calibrated range.
    pub fn normalize(self, raw: u16) -> Option<i16> {
        if raw > 4095 || !(self.min < self.center && self.center < self.max) || self.max > 4095 {
            return None;
        }
        let (distance, span) = if raw >= self.center {
            (
                i32::from(raw - self.center),
                i32::from(self.max - self.center),
            )
        } else {
            (
                i32::from(raw) - i32::from(self.center),
                i32::from(self.center - self.min),
            )
        };
        let scaled = distance * i32::from(i16::MAX) / span;
        Some(scaled.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchStickCalibration {
    pub x: SwitchAxisCalibration,
    pub y: SwitchAxisCalibration,
    pub source: SwitchCalibrationSource,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SwitchCalibration {
    pub left: Option<SwitchStickCalibration>,
    pub right: Option<SwitchStickCalibration>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SwitchSetupStep {
    LowPower,
    ReportMode,
    UserCalibration,
    FactoryCalibration,
}

impl SwitchSetupStep {
    pub(crate) fn subcommand(self) -> u8 {
        match self {
            Self::LowPower => SWITCH_SUBCMD_LOW_POWER_MODE,
            Self::ReportMode => SWITCH_SUBCMD_SET_REPORT_MODE,
            Self::UserCalibration | Self::FactoryCalibration => SWITCH_SUBCMD_SPI_FLASH_READ,
        }
    }
}

/// Error from the shared response-driven Switch setup sequence.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwitchSetupError {
    AlreadyStarted,
    NotStarted,
    MalformedAck,
    NegativeAck,
    InvalidCalibration,
    RetryLimit,
}

/// Result after the shared setup engine accepts a controller report.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwitchSetupProgress {
    /// Send this packet, then wait for its matching response.
    Send(SwitchOutputPacket),
    /// The controller returned valid calibration for all required sticks.
    Ready(SwitchCalibration),
    /// The report was valid HID input but did not match the pending subcommand.
    Ignored,
}

/// Shared Switch initialization state. Bluetooth and USB transports use this
/// engine so ACK, SPI echo, and calibration rules stay the same.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchSetupEngine {
    needs_left: bool,
    needs_right: bool,
    step: Option<SwitchSetupStep>,
    packet_counter: u8,
    retries: u8,
    calibration: Option<SwitchCalibration>,
}

impl SwitchSetupEngine {
    pub const fn new(needs_left: bool, needs_right: bool) -> Self {
        Self {
            needs_left,
            needs_right,
            step: None,
            packet_counter: 0,
            retries: 0,
            calibration: None,
        }
    }

    pub fn start(&mut self) -> Result<SwitchOutputPacket, SwitchSetupError> {
        if self.step.is_some() || self.calibration.is_some() {
            return Err(SwitchSetupError::AlreadyStarted);
        }
        self.queue_step(SwitchSetupStep::LowPower)
    }

    /// Retransmit the pending request once after its transport deadline.
    pub fn retry(&mut self) -> Result<SwitchOutputPacket, SwitchSetupError> {
        let step = self.step.ok_or(SwitchSetupError::NotStarted)?;
        if self.retries >= 1 {
            return Err(SwitchSetupError::RetryLimit);
        }
        self.retries += 1;
        self.queue_step(step)
    }

    pub const fn is_pending(&self) -> bool {
        self.step.is_some()
    }

    pub fn pending_subcommand(&self) -> Option<u8> {
        self.step.map(|step| step.subcommand())
    }

    pub const fn calibration(&self) -> Option<SwitchCalibration> {
        if self.step.is_none() {
            self.calibration
        } else {
            None
        }
    }

    /// Process one HID report. A wrong subcommand ACK is ignored while the
    /// current request stays pending. A matching NACK or malformed echo fails.
    pub fn receive(&mut self, report: &[u8]) -> Result<SwitchSetupProgress, SwitchSetupError> {
        let step = self.step.ok_or(SwitchSetupError::NotStarted)?;
        if report.first().copied() != Some(SWITCH_HID_INPUT_SUBCOMMAND_ACK) {
            return Ok(SwitchSetupProgress::Ignored);
        }
        let ack = parse_switch_subcommand_ack(report).ok_or(SwitchSetupError::MalformedAck)?;
        if ack.subcommand != step.subcommand() {
            return Ok(SwitchSetupProgress::Ignored);
        }
        if !ack.ack {
            return Err(SwitchSetupError::NegativeAck);
        }
        let data = match step {
            SwitchSetupStep::UserCalibration | SwitchSetupStep::FactoryCalibration => {
                let (address, length) = if step == SwitchSetupStep::UserCalibration {
                    (
                        SWITCH_USER_CALIBRATION_ADDRESS,
                        SWITCH_USER_CALIBRATION_LENGTH,
                    )
                } else {
                    (
                        SWITCH_FACTORY_CALIBRATION_ADDRESS,
                        SWITCH_FACTORY_CALIBRATION_LENGTH,
                    )
                };
                let expected = 5 + usize::from(length);
                if ack.data.len() < expected
                    || ack.data[..4] != address.to_le_bytes()
                    || ack.data[4] != length
                {
                    return Err(SwitchSetupError::MalformedAck);
                }
                &ack.data[5..expected]
            }
            SwitchSetupStep::LowPower | SwitchSetupStep::ReportMode => ack.data,
        };
        self.retries = 0;
        match step {
            SwitchSetupStep::LowPower => self
                .queue_step(SwitchSetupStep::ReportMode)
                .map(SwitchSetupProgress::Send),
            SwitchSetupStep::ReportMode => self
                .queue_step(SwitchSetupStep::UserCalibration)
                .map(SwitchSetupProgress::Send),
            SwitchSetupStep::UserCalibration => {
                let calibration =
                    parse_user_switch_calibration(data, self.needs_left, self.needs_right)
                        .map_err(|_| SwitchSetupError::InvalidCalibration)?;
                let missing_required = (self.needs_left && calibration.left.is_none())
                    || (self.needs_right && calibration.right.is_none());
                if missing_required {
                    self.calibration = Some(calibration);
                    self.queue_step(SwitchSetupStep::FactoryCalibration)
                        .map(SwitchSetupProgress::Send)
                } else {
                    self.calibration = Some(calibration);
                    self.finish_ready()
                }
            }
            SwitchSetupStep::FactoryCalibration => {
                let mut calibration = self
                    .calibration
                    .ok_or(SwitchSetupError::InvalidCalibration)?;
                let needs_left = self.needs_left && calibration.left.is_none();
                let needs_right = self.needs_right && calibration.right.is_none();
                let factory = parse_factory_switch_calibration(data, needs_left, needs_right)
                    .map_err(|_| SwitchSetupError::InvalidCalibration)?;
                if needs_left {
                    calibration.left = factory.left;
                }
                if needs_right {
                    calibration.right = factory.right;
                }
                self.calibration = Some(calibration);
                self.finish_ready()
            }
        }
    }

    fn queue_step(
        &mut self,
        step: SwitchSetupStep,
    ) -> Result<SwitchOutputPacket, SwitchSetupError> {
        let packet = build_switch_setup_packet(step, self.packet_counter)
            .ok_or(SwitchSetupError::InvalidCalibration)?;
        self.packet_counter = self.packet_counter.wrapping_add(1) & 0x0f;
        self.step = Some(step);
        Ok(packet)
    }

    fn finish_ready(&mut self) -> Result<SwitchSetupProgress, SwitchSetupError> {
        let calibration = self
            .calibration
            .ok_or(SwitchSetupError::InvalidCalibration)?;
        if (self.needs_left && calibration.left.is_none())
            || (self.needs_right && calibration.right.is_none())
        {
            return Err(SwitchSetupError::InvalidCalibration);
        }
        self.step = None;
        Ok(SwitchSetupProgress::Ready(calibration))
    }
}

/// Parsed Switch subcommand ACK report.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchSubcommandAck<'a> {
    pub ack: bool,
    pub ack_type: u8,
    pub subcommand: u8,
    pub data: &'a [u8],
}

/// Bounded Switch HID output packet.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SwitchOutputPacket {
    bytes: [u8; SWITCH_SUBCOMMAND_OUTPUT_BYTES],
    len: usize,
}

impl SwitchOutputPacket {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Builds a report `0x01` rumble plus subcommand packet.
pub fn build_switch_subcommand_packet(
    packet_counter: u8,
    rumble: [u8; 8],
    subcommand: u8,
    data: &[u8],
) -> Option<SwitchOutputPacket> {
    let len = 11usize.checked_add(data.len())?;
    if len > SWITCH_SUBCOMMAND_OUTPUT_BYTES {
        return None;
    }

    let mut bytes = [0u8; SWITCH_SUBCOMMAND_OUTPUT_BYTES];
    bytes[0] = SWITCH_HID_OUTPUT_SUBCOMMAND;
    bytes[1] = packet_counter & 0x0f;
    bytes[2..10].copy_from_slice(&rumble);
    bytes[10] = subcommand;
    bytes[11..len].copy_from_slice(data);
    Some(SwitchOutputPacket { bytes, len })
}

/// Builds a report `0x10` rumble-only keepalive packet.
pub fn build_switch_rumble_packet(packet_counter: u8, rumble: [u8; 8]) -> SwitchOutputPacket {
    let mut bytes = [0u8; SWITCH_SUBCOMMAND_OUTPUT_BYTES];
    bytes[0] = SWITCH_HID_OUTPUT_RUMBLE_ONLY;
    bytes[1] = packet_counter & 0x0f;
    bytes[2..10].copy_from_slice(&rumble);
    SwitchOutputPacket {
        bytes,
        len: SWITCH_RUMBLE_OUTPUT_BYTES,
    }
}

/// Builds the minimal Joy-Con/Pro init sequence packets.
pub fn build_switch_init_packets(packet_counter: u8) -> [SwitchOutputPacket; 2] {
    [
        build_switch_setup_packet(SwitchSetupStep::LowPower, packet_counter)
            .expect("fixed low-power packet fits"),
        build_switch_setup_packet(SwitchSetupStep::ReportMode, packet_counter.wrapping_add(1))
            .expect("fixed report-mode packet fits"),
    ]
}

/// Build one setup packet. The host and packet fixtures share this path.
pub(crate) fn build_switch_setup_packet(
    step: SwitchSetupStep,
    packet_counter: u8,
) -> Option<SwitchOutputPacket> {
    match step {
        SwitchSetupStep::LowPower => build_switch_subcommand_packet(
            packet_counter,
            SWITCH_NEUTRAL_RUMBLE,
            SWITCH_SUBCMD_LOW_POWER_MODE,
            &[0],
        ),
        SwitchSetupStep::ReportMode => build_switch_subcommand_packet(
            packet_counter,
            SWITCH_NEUTRAL_RUMBLE,
            SWITCH_SUBCMD_SET_REPORT_MODE,
            &[SWITCH_REPORT_MODE_STANDARD_FULL],
        ),
        SwitchSetupStep::UserCalibration => build_switch_spi_read_packet(
            packet_counter,
            SWITCH_USER_CALIBRATION_ADDRESS,
            SWITCH_USER_CALIBRATION_LENGTH,
        ),
        SwitchSetupStep::FactoryCalibration => build_switch_spi_read_packet(
            packet_counter,
            SWITCH_FACTORY_CALIBRATION_ADDRESS,
            SWITCH_FACTORY_CALIBRATION_LENGTH,
        ),
    }
}

/// Build a read-only SPI calibration request with the Switch little-endian address format.
pub fn build_switch_spi_read_packet(
    packet_counter: u8,
    address: u32,
    length: u8,
) -> Option<SwitchOutputPacket> {
    if length == 0 {
        return None;
    }
    let mut data = [0u8; 5];
    data[..4].copy_from_slice(&address.to_le_bytes());
    data[4] = length;
    build_switch_subcommand_packet(
        packet_counter,
        SWITCH_NEUTRAL_RUMBLE,
        SWITCH_SUBCMD_SPI_FLASH_READ,
        &data,
    )
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum CalibrationParseError {
    Length,
    InvalidRange,
}

/// Parse the user block. A missing magic means that stick needs factory data.
pub(crate) fn parse_user_switch_calibration(
    data: &[u8],
    needs_left: bool,
    needs_right: bool,
) -> Result<SwitchCalibration, CalibrationParseError> {
    if data.len() != usize::from(SWITCH_USER_CALIBRATION_LENGTH) {
        return Err(CalibrationParseError::Length);
    }
    let left = if needs_left && data[0..2] == SWITCH_USER_CALIBRATION_MAGIC {
        Some(parse_stick_calibration(
            &data[2..11],
            true,
            SwitchCalibrationSource::User,
        )?)
    } else {
        None
    };
    let right = if needs_right && data[11..13] == SWITCH_USER_CALIBRATION_MAGIC {
        Some(parse_stick_calibration(
            &data[13..22],
            false,
            SwitchCalibrationSource::User,
        )?)
    } else {
        None
    };
    Ok(SwitchCalibration { left, right })
}

pub(crate) fn parse_factory_switch_calibration(
    data: &[u8],
    needs_left: bool,
    needs_right: bool,
) -> Result<SwitchCalibration, CalibrationParseError> {
    if data.len() != usize::from(SWITCH_FACTORY_CALIBRATION_LENGTH) {
        return Err(CalibrationParseError::Length);
    }
    Ok(SwitchCalibration {
        left: needs_left
            .then(|| parse_stick_calibration(&data[..9], true, SwitchCalibrationSource::Factory))
            .transpose()?,
        right: needs_right
            .then(|| parse_stick_calibration(&data[9..], false, SwitchCalibrationSource::Factory))
            .transpose()?,
    })
}

fn parse_stick_calibration(
    data: &[u8],
    left: bool,
    source: SwitchCalibrationSource,
) -> Result<SwitchStickCalibration, CalibrationParseError> {
    if data.len() != 9 {
        return Err(CalibrationParseError::Length);
    }
    let first = read_stick12_pair(&data[0..3]);
    let second = read_stick12_pair(&data[3..6]);
    let third = read_stick12_pair(&data[6..9]);
    let (center, below, above) = if left {
        (second, third, first)
    } else {
        (first, second, third)
    };
    let x = SwitchAxisCalibration::new(center.0, below.0, above.0)
        .ok_or(CalibrationParseError::InvalidRange)?;
    let y = SwitchAxisCalibration::new(center.1, below.1, above.1)
        .ok_or(CalibrationParseError::InvalidRange)?;
    Ok(SwitchStickCalibration { x, y, source })
}

fn read_stick12_pair(bytes: &[u8]) -> (u16, u16) {
    (read_stick12_x(bytes), read_stick12_y(bytes))
}

/// Builds a player LED subcommand packet.
pub fn build_switch_player_led_packet(packet_counter: u8, led_mask: u8) -> SwitchOutputPacket {
    build_switch_subcommand_packet(
        packet_counter,
        SWITCH_NEUTRAL_RUMBLE,
        SWITCH_SUBCMD_SET_PLAYER_LIGHTS,
        &[led_mask],
    )
    .expect("fixed LED packet fits")
}

/// Writes a Bluetooth HIDP output payload for an L2CAP HID channel.
pub fn write_switch_hidp_output_payload(
    packet: &SwitchOutputPacket,
    out: &mut [u8],
) -> Option<usize> {
    let bytes = packet.bytes();
    let len = 1usize.checked_add(bytes.len())?;
    if out.len() < len {
        return None;
    }
    out[0] = SWITCH_HIDP_OUTPUT_HEADER;
    out[1..len].copy_from_slice(bytes);
    Some(len)
}

/// Returns the HID report bytes from a Bluetooth HIDP input payload.
pub fn switch_report_from_hidp_input(payload: &[u8]) -> Option<&[u8]> {
    let hidp = bt::parse_hidp_packet(payload).ok()?;
    (hidp.transaction == bt::HIDP_TRANS_DATA && hidp.parameter == bt::HIDP_DATA_RTYPE_INPUT)
        .then_some(hidp.payload)
}

/// Parses a Switch standard input report. Accepts the first 12 bytes needed for buttons/sticks.
pub fn parse_switch_standard_report(report: &[u8]) -> Option<SwitchStandardReport> {
    let report_id = report.first().copied()?;
    if !matches!(report_id, 0x30..=0x33) || report.len() < 12 {
        return None;
    }

    Some(SwitchStandardReport {
        report_id,
        timer: report[1],
        battery_connection: report[2],
        right_buttons: report[3],
        shared_buttons: report[4],
        left_buttons: report[5],
        left_stick_x: read_stick12_x(&report[6..9]),
        left_stick_y: read_stick12_y(&report[6..9]),
        right_stick_x: read_stick12_x(&report[9..12]),
        right_stick_y: read_stick12_y(&report[9..12]),
    })
}

/// Parses a Switch report `0x21` subcommand ACK.
pub fn parse_switch_subcommand_ack(report: &[u8]) -> Option<SwitchSubcommandAck<'_>> {
    if report.first().copied()? != SWITCH_HID_INPUT_SUBCOMMAND_ACK || report.len() < 15 {
        return None;
    }
    let ack = report[13];
    Some(SwitchSubcommandAck {
        ack: ack & 0x80 != 0,
        ack_type: ack & 0x7f,
        subcommand: report[14],
        data: &report[15..],
    })
}

fn read_stick12_x(bytes: &[u8]) -> u16 {
    bytes[0] as u16 | (((bytes[1] as u16) & 0x0f) << 8)
}

fn read_stick12_y(bytes: &[u8]) -> u16 {
    ((bytes[1] as u16) >> 4) | ((bytes[2] as u16) << 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    fn ack_report(subcommand: u8, data: &[u8], ack: bool) -> Vec<u8> {
        let mut report = vec![0u8; 15 + data.len()];
        report[0] = SWITCH_HID_INPUT_SUBCOMMAND_ACK;
        report[13] = if ack { 0x80 } else { 0 };
        report[14] = subcommand;
        report[15..].copy_from_slice(data);
        report
    }

    fn spi_ack(subcommand: u8, address: u32, data: &[u8]) -> Vec<u8> {
        spi_ack_padded(subcommand, address, data, 15 + 5 + data.len())
    }

    fn spi_ack_padded(subcommand: u8, address: u32, data: &[u8], report_len: usize) -> Vec<u8> {
        let mut echo = address.to_le_bytes().to_vec();
        echo.push(data.len() as u8);
        echo.extend_from_slice(data);
        let mut report = ack_report(subcommand, &echo, true);
        report.resize(report_len, 0);
        report
    }

    fn empty_user_calibration() -> [u8; 22] {
        [0; 22]
    }

    fn factory_calibration_fixture() -> [u8; 18] {
        [
            0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32, 0xd0, 0x47, 0x83, 0x84, 0x03,
            0x32, 0xe8, 0xc3, 0x44,
        ]
    }

    #[test]
    fn hidp_output_writes_fixed_packet_and_preserves_short_buffer() {
        let packet = build_switch_player_led_packet(16, 5);
        let mut output = [0xa5; 13];
        assert_eq!(
            write_switch_hidp_output_payload(&packet, &mut output[..12]),
            None
        );
        assert_eq!(output, [0xa5; 13]);
        assert_eq!(
            write_switch_hidp_output_payload(&packet, &mut output),
            Some(13)
        );
        assert_eq!(output, [0xa2, 1, 0, 0, 1, 64, 64, 0, 1, 64, 64, 48, 5]);
    }

    #[test]
    fn hidp_input_and_standard_report_check_headers_lengths_and_values() {
        let payload = [
            0xa1, 0x30, 7, 0x91, 0x12, 0x34, 0x56, 0x23, 0x61, 0x45, 0x89, 0xc7, 0xab,
        ];
        let report = switch_report_from_hidp_input(&payload).unwrap();
        let parsed = parse_switch_standard_report(report).unwrap();
        assert_eq!(
            (parsed.report_id, parsed.timer, parsed.battery_connection),
            (0x30, 7, 0x91)
        );
        assert_eq!(
            (
                parsed.right_buttons,
                parsed.shared_buttons,
                parsed.left_buttons
            ),
            (0x12, 0x34, 0x56)
        );
        assert_eq!((parsed.left_stick_x, parsed.left_stick_y), (0x123, 0x456));
        assert_eq!((parsed.right_stick_x, parsed.right_stick_y), (0x789, 0xabc));
        assert!(switch_report_from_hidp_input(&[]).is_none());
        assert!(switch_report_from_hidp_input(&[0xa2, 0x30]).is_none());
        assert!(parse_switch_standard_report(&report[..11]).is_none());
        let mut wrong_id = [0u8; 12];
        wrong_id[0] = 0x34;
        assert!(parse_switch_standard_report(&wrong_id).is_none());
    }

    #[test]
    fn initialization_and_rumble_have_independent_expected_bytes() {
        let init = build_switch_init_packets(14);
        assert_eq!(init[0].bytes(), &[1, 14, 0, 1, 64, 64, 0, 1, 64, 64, 8, 0]);
        assert_eq!(init[1].bytes(), &[1, 15, 0, 1, 64, 64, 0, 1, 64, 64, 3, 48]);
        assert_eq!(
            build_switch_player_led_packet(16, 5).bytes(),
            &[1, 0, 0, 1, 64, 64, 0, 1, 64, 64, 48, 5]
        );
        assert_eq!(
            build_switch_rumble_packet(17, SWITCH_NEUTRAL_RUMBLE).bytes(),
            &[16, 1, 0, 1, 64, 64, 0, 1, 64, 64]
        );
        assert!(build_switch_subcommand_packet(0, SWITCH_NEUTRAL_RUMBLE, 3, &[0; 54]).is_none());
    }
    #[test]
    fn ack_and_nack_are_distinct_and_short_replies_are_rejected() {
        let mut packet = [0u8; 15];
        packet[0] = 33;
        packet[13] = 128;
        packet[14] = 3;
        let ack = parse_switch_subcommand_ack(&packet).unwrap();
        assert!(ack.ack);
        assert_eq!(ack.subcommand, 3);
        packet[13] = 0;
        assert!(!parse_switch_subcommand_ack(&packet).unwrap().ack);
        assert!(parse_switch_subcommand_ack(&packet[..14]).is_none());
    }

    #[test]
    fn spi_calibration_read_packet_has_fixed_little_endian_fixture() {
        assert_eq!(
            build_switch_spi_read_packet(15, 0x8010, 22)
                .unwrap()
                .bytes(),
            &[1, 15, 0, 1, 64, 64, 0, 1, 64, 64, 16, 16, 128, 0, 0, 22]
        );
        assert!(build_switch_spi_read_packet(0, 0x8010, 0).is_none());
    }

    #[test]
    fn user_and_factory_calibration_fixtures_decode_without_defaults() {
        let user = [
            0xb2, 0xa1, // left user calibration marker
            0xe8, 0xc3, 0x44, // left x/y max above center
            0xd0, 0x47, 0x83, // left x/y center
            0x84, 0x03, 0x32, // left x/y min below center
            0xb2, 0xa1, // right user calibration marker
            0xd0, 0x47, 0x83, // right x/y center
            0x84, 0x03, 0x32, // right x/y min below center
            0xe8, 0xc3, 0x44, // right x/y max above center
        ];
        let calibration = parse_user_switch_calibration(&user, true, true).unwrap();
        let left = calibration.left.unwrap();
        assert_eq!((left.x.min, left.x.center, left.x.max), (1100, 2000, 3000));
        assert_eq!((left.y.min, left.y.center, left.y.max), (1300, 2100, 3200));
        assert_eq!(left.source, SwitchCalibrationSource::User);
        assert_eq!(left.x.normalize(1100), Some(-32767));
        assert_eq!(left.x.normalize(2000), Some(0));
        assert_eq!(left.x.normalize(3000), Some(32767));
        assert_eq!(left.x.normalize(4096), None);

        let factory = [
            0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32, 0xd0, 0x47, 0x83, 0x84, 0x03,
            0x32, 0xe8, 0xc3, 0x44,
        ];
        let fallback = parse_factory_switch_calibration(&factory, true, true).unwrap();
        assert_eq!(
            fallback.left.unwrap().source,
            SwitchCalibrationSource::Factory
        );
        assert_eq!(
            fallback.right.unwrap().source,
            SwitchCalibrationSource::Factory
        );
    }

    #[test]
    fn absent_user_magic_is_distinct_from_malformed_calibration() {
        let mut absent = [0u8; 22];
        absent[2..11].copy_from_slice(&[0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32]);
        let user = parse_user_switch_calibration(&absent, true, true).unwrap();
        assert!(user.left.is_none());
        assert!(user.right.is_none());

        let mut malformed = absent;
        malformed[..2].copy_from_slice(&SWITCH_USER_CALIBRATION_MAGIC);
        malformed[2..11].fill(0);
        assert_eq!(
            parse_user_switch_calibration(&malformed, true, true),
            Err(CalibrationParseError::InvalidRange)
        );
        assert_eq!(
            parse_factory_switch_calibration(&[0; 18], true, true),
            Err(CalibrationParseError::InvalidRange)
        );
        assert_eq!(
            parse_factory_switch_calibration(&[0; 17], true, true),
            Err(CalibrationParseError::Length)
        );
    }

    #[test]
    fn shared_setup_waits_for_ack_and_valid_factory_calibration() {
        let mut setup = SwitchSetupEngine::new(true, true);
        assert_eq!(setup.pending_subcommand(), None);
        assert_eq!(setup.calibration(), None);
        assert_eq!(
            setup.start().unwrap().bytes(),
            &[1, 0, 0, 1, 64, 64, 0, 1, 64, 64, 8, 0]
        );
        assert_eq!(
            setup.pending_subcommand(),
            Some(SWITCH_SUBCMD_LOW_POWER_MODE)
        );
        let report_mode = setup
            .receive(&ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], true))
            .unwrap();
        let SwitchSetupProgress::Send(report_mode) = report_mode else {
            panic!("low-power ACK must request report mode");
        };
        assert_eq!(
            setup.pending_subcommand(),
            Some(SWITCH_SUBCMD_SET_REPORT_MODE)
        );
        assert_eq!(
            report_mode.bytes(),
            &[1, 1, 0, 1, 64, 64, 0, 1, 64, 64, 3, 48]
        );
        let user_read = setup
            .receive(&ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true))
            .unwrap();
        let SwitchSetupProgress::Send(user_read) = user_read else {
            panic!("report-mode ACK must request user calibration");
        };
        assert_eq!(
            user_read.bytes(),
            &[1, 2, 0, 1, 64, 64, 0, 1, 64, 64, 16, 16, 128, 0, 0, 22]
        );
        let factory_read = setup
            .receive(&spi_ack_padded(
                SWITCH_SUBCMD_SPI_FLASH_READ,
                SWITCH_USER_CALIBRATION_ADDRESS,
                &empty_user_calibration(),
                49,
            ))
            .unwrap();
        let SwitchSetupProgress::Send(factory_read) = factory_read else {
            panic!("missing user magic must request factory calibration");
        };
        assert_eq!(setup.calibration(), None);
        assert_eq!(
            factory_read.bytes(),
            &[1, 3, 0, 1, 64, 64, 0, 1, 64, 64, 16, 61, 96, 0, 0, 18]
        );
        let ready = setup
            .receive(&spi_ack_padded(
                SWITCH_SUBCMD_SPI_FLASH_READ,
                SWITCH_FACTORY_CALIBRATION_ADDRESS,
                &factory_calibration_fixture(),
                64,
            ))
            .unwrap();
        let SwitchSetupProgress::Ready(calibration) = ready else {
            panic!("valid factory calibration must finish setup");
        };
        assert_eq!(
            calibration.left.unwrap().source,
            SwitchCalibrationSource::Factory
        );
        assert_eq!(
            calibration.right.unwrap().source,
            SwitchCalibrationSource::Factory
        );
        assert!(!setup.is_pending());
        assert_eq!(setup.pending_subcommand(), None);
        assert_eq!(setup.calibration(), Some(calibration));
    }

    #[test]
    fn shared_setup_ignores_wrong_ack_and_rejects_nack_or_bad_spi_echo() {
        let mut setup = SwitchSetupEngine::new(true, true);
        setup.start().unwrap();
        assert_eq!(
            setup
                .receive(&ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true))
                .unwrap(),
            SwitchSetupProgress::Ignored
        );
        assert!(setup.is_pending());
        assert_eq!(
            setup.receive(&ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], false)),
            Err(SwitchSetupError::NegativeAck)
        );

        let mut setup = SwitchSetupEngine::new(true, true);
        setup.start().unwrap();
        setup
            .receive(&ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], true))
            .unwrap();
        setup
            .receive(&ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true))
            .unwrap();
        let malformed = spi_ack(
            SWITCH_SUBCMD_SPI_FLASH_READ,
            SWITCH_USER_CALIBRATION_ADDRESS + 1,
            &empty_user_calibration(),
        );
        assert_eq!(
            setup.receive(&malformed),
            Err(SwitchSetupError::MalformedAck)
        );
        assert!(setup.is_pending());

        let short = spi_ack_padded(
            SWITCH_SUBCMD_SPI_FLASH_READ,
            SWITCH_USER_CALIBRATION_ADDRESS,
            &empty_user_calibration()[..21],
            41,
        );
        assert_eq!(setup.receive(&short), Err(SwitchSetupError::MalformedAck));
    }

    #[test]
    fn shared_setup_retry_is_bounded_and_keeps_the_pending_step() {
        let mut setup = SwitchSetupEngine::new(true, true);
        let first = setup.start().unwrap();
        let retry = setup.retry().unwrap();
        assert_eq!(first.bytes()[10], SWITCH_SUBCMD_LOW_POWER_MODE);
        assert_eq!(retry.bytes()[10], SWITCH_SUBCMD_LOW_POWER_MODE);
        assert_eq!(retry.bytes()[1], 1);
        assert_eq!(setup.retry(), Err(SwitchSetupError::RetryLimit));
        assert!(setup.is_pending());
    }
}
