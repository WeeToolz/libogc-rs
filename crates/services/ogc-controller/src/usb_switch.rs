//! Async Nintendo Switch Pro USB initialization over interrupt endpoints.

#[cfg(test)]
extern crate alloc;

use ogc_bt_sync::switch::{
    SWITCH_HID_INPUT_SUBCOMMAND_ACK, SwitchCalibration, SwitchOutputPacket, SwitchSetupEngine,
    SwitchSetupError, SwitchSetupProgress, parse_switch_subcommand_ack,
};

const USB_REPORT_ID: u8 = 0x80;
const USB_RESPONSE_ID: u8 = 0x81;
const USB_HANDSHAKE: u8 = 0x02;
const USB_BAUDRATE_3M: u8 = 0x03;
const USB_NO_TIMEOUT: u8 = 0x04;
const OUTPUT_CAPACITY: usize = 64;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct EarlyResponse {
    bytes: [u8; OUTPUT_CAPACITY],
    len: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SwitchInputDispatch {
    StandardInput,
    Ignore,
    Malformed,
    Unsupported,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsbSwitchSetupError {
    Output(i32),
    Input(i32),
    Timeout,
    Protocol,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsbSwitchInputProgress {
    Ignore,
    Send,
    Ready(SwitchCalibration),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsbSwitchOutput {
    bytes: [u8; OUTPUT_CAPACITY],
    len: u8,
}

impl UsbSwitchOutput {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum NextUsbStep {
    Baudrate,
    SecondHandshake,
    NoTimeout,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Phase {
    SendUsb {
        command: u8,
        next: NextUsbStep,
        retries: u8,
    },
    WritingUsb {
        command: u8,
        next: NextUsbStep,
        retries: u8,
        deadline: u64,
    },
    WaitingUsb {
        command: u8,
        next: NextUsbStep,
        retries: u8,
        deadline: u64,
    },
    SendNoTimeout,
    WritingNoTimeout {
        deadline: u64,
    },
    SendSwitch(SwitchOutputPacket),
    WritingSwitch {
        deadline: u64,
    },
    WaitingSwitch {
        deadline: u64,
    },
    Ready,
    Failed,
}

/// Owns the protocol state for Nintendo Switch Pro USB setup. The USB caller
/// owns real IOS buffers and calls these methods as transfers complete.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsbSwitchSetup {
    phase: Phase,
    switch: SwitchSetupEngine,
    timeout_ticks: u64,
    early_response: Option<EarlyResponse>,
}

impl UsbSwitchSetup {
    pub(crate) const fn new(timeout_ticks: u64) -> Self {
        Self {
            phase: Phase::SendUsb {
                command: USB_HANDSHAKE,
                next: NextUsbStep::Baudrate,
                retries: 0,
            },
            switch: SwitchSetupEngine::new(true, true),
            timeout_ticks,
            early_response: None,
        }
    }

    pub(crate) fn next_output(&mut self, now: u64) -> Option<UsbSwitchOutput> {
        let phase = core::mem::replace(&mut self.phase, Phase::Failed);
        match phase {
            Phase::SendUsb {
                command,
                next,
                retries,
            } => {
                self.phase = Phase::WritingUsb {
                    command,
                    next,
                    retries,
                    deadline: now.saturating_add(self.timeout_ticks),
                };
                Some(usb_command(command))
            }
            Phase::SendNoTimeout => {
                self.phase = Phase::WritingNoTimeout {
                    deadline: now.saturating_add(self.timeout_ticks),
                };
                Some(usb_command(USB_NO_TIMEOUT))
            }
            Phase::SendSwitch(packet) => {
                let output = switch_packet(packet);
                self.phase = Phase::WritingSwitch {
                    deadline: now.saturating_add(self.timeout_ticks),
                };
                Some(output)
            }
            other => {
                self.phase = other;
                None
            }
        }
    }

    /// Complete one submitted interrupt OUT operation. libogc forwards the
    /// raw IOS status, so every nonnegative completion is successful.
    pub(crate) fn output_complete(
        &mut self,
        result: i32,
        now: u64,
    ) -> Result<Option<UsbSwitchInputProgress>, UsbSwitchSetupError> {
        if result < 0 {
            self.phase = Phase::Failed;
            return Err(UsbSwitchSetupError::Output(result));
        }
        let phase = core::mem::replace(&mut self.phase, Phase::Failed);
        self.phase = match phase {
            Phase::WritingUsb {
                command,
                next,
                retries,
                ..
            } => Phase::WaitingUsb {
                command,
                next,
                retries,
                deadline: now.saturating_add(self.timeout_ticks),
            },
            Phase::WritingNoTimeout { .. } => {
                let packet = self.switch.start().map_err(map_switch_error)?;
                Phase::SendSwitch(packet)
            }
            Phase::WritingSwitch { .. } => Phase::WaitingSwitch {
                deadline: now.saturating_add(self.timeout_ticks),
            },
            other => {
                self.phase = other;
                return Err(UsbSwitchSetupError::Protocol);
            }
        };
        let early = self.early_response.take();
        match early {
            Some(response) => self
                .receive(&response.bytes[..usize::from(response.len)], now)
                .map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn input_failed(&mut self, code: i32) -> UsbSwitchSetupError {
        self.phase = Phase::Failed;
        UsbSwitchSetupError::Input(code)
    }

    /// Accept a response from the interrupt IN endpoint. Unrelated reports and
    /// USB replies for a different command leave the current step pending.
    pub(crate) fn receive(
        &mut self,
        report: &[u8],
        _now: u64,
    ) -> Result<UsbSwitchInputProgress, UsbSwitchSetupError> {
        match self.phase {
            Phase::WaitingUsb { command, next, .. } => {
                if report.first().copied() != Some(USB_RESPONSE_ID) {
                    return Ok(UsbSwitchInputProgress::Ignore);
                }
                if report.len() < 2 {
                    return self.fail_protocol();
                }
                if report[1] != command {
                    return Ok(UsbSwitchInputProgress::Ignore);
                }
                self.phase = match next {
                    NextUsbStep::Baudrate => Phase::SendUsb {
                        command: USB_BAUDRATE_3M,
                        next: NextUsbStep::SecondHandshake,
                        retries: 0,
                    },
                    NextUsbStep::SecondHandshake => Phase::SendUsb {
                        command: USB_HANDSHAKE,
                        next: NextUsbStep::NoTimeout,
                        retries: 0,
                    },
                    NextUsbStep::NoTimeout => Phase::SendNoTimeout,
                };
                Ok(UsbSwitchInputProgress::Send)
            }
            Phase::WaitingSwitch { .. } => {
                if report.first().copied() != Some(SWITCH_HID_INPUT_SUBCOMMAND_ACK) {
                    return Ok(UsbSwitchInputProgress::Ignore);
                }
                let progress = match self.switch.receive(report) {
                    Ok(progress) => progress,
                    Err(error) => {
                        self.phase = Phase::Failed;
                        return Err(map_switch_error(error));
                    }
                };
                match progress {
                    SwitchSetupProgress::Ignored => Ok(UsbSwitchInputProgress::Ignore),
                    SwitchSetupProgress::Send(packet) => {
                        self.phase = Phase::SendSwitch(packet);
                        Ok(UsbSwitchInputProgress::Send)
                    }
                    SwitchSetupProgress::Ready(calibration) => {
                        self.phase = Phase::Ready;
                        Ok(UsbSwitchInputProgress::Ready(calibration))
                    }
                }
            }
            Phase::WritingUsb { command, .. }
                if report.first().copied() == Some(USB_RESPONSE_ID)
                    && report.get(1).copied() == Some(command) =>
            {
                self.save_early_response(report)?;
                Ok(UsbSwitchInputProgress::Ignore)
            }
            Phase::WritingSwitch { .. }
                if report.first().copied() == Some(SWITCH_HID_INPUT_SUBCOMMAND_ACK)
                    && report.get(14).copied() == self.switch.pending_subcommand() =>
            {
                self.save_early_response(report)?;
                Ok(UsbSwitchInputProgress::Ignore)
            }
            Phase::WritingUsb { .. }
            | Phase::WritingNoTimeout { .. }
            | Phase::WritingSwitch { .. }
            | Phase::SendUsb { .. }
            | Phase::SendNoTimeout
            | Phase::SendSwitch(_) => Ok(UsbSwitchInputProgress::Ignore),
            Phase::Ready => Ok(UsbSwitchInputProgress::Ignore),
            _ => self.fail_protocol(),
        }
    }

    /// Apply a real tick deadline. The caller must retire the device when this
    /// returns an error because a pending IOS operation cannot be cancelled.
    pub(crate) fn expire(&mut self, now: u64) -> Result<(), UsbSwitchSetupError> {
        match self.phase {
            Phase::WritingUsb { deadline, .. }
            | Phase::WritingNoTimeout { deadline }
            | Phase::WritingSwitch { deadline }
                if now >= deadline =>
            {
                self.phase = Phase::Failed;
                Err(UsbSwitchSetupError::Timeout)
            }
            Phase::WaitingUsb {
                command,
                next,
                retries,
                deadline,
            } if now >= deadline => {
                if retries >= 1 {
                    self.phase = Phase::Failed;
                    return Err(UsbSwitchSetupError::Timeout);
                }
                self.phase = Phase::SendUsb {
                    command,
                    next,
                    retries: retries + 1,
                };
                Ok(())
            }
            Phase::WaitingSwitch { deadline } if now >= deadline => match self.switch.retry() {
                Ok(packet) => {
                    self.phase = Phase::SendSwitch(packet);
                    Ok(())
                }
                Err(SwitchSetupError::RetryLimit) => {
                    self.phase = Phase::Failed;
                    Err(UsbSwitchSetupError::Timeout)
                }
                Err(_) => self.fail_protocol(),
            },
            _ => Ok(()),
        }
    }

    pub(crate) const fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    pub(crate) const fn timeout_operation(&self) -> &'static str {
        match self.phase {
            Phase::WritingUsb { .. }
            | Phase::WritingNoTimeout { .. }
            | Phase::WritingSwitch { .. } => "Switch USB write wait timeout",
            Phase::WaitingUsb { .. } => "Switch USB handshake response timeout",
            Phase::WaitingSwitch { .. } => "Switch USB subcommand response timeout",
            _ => "Switch USB setup timeout",
        }
    }

    fn fail_protocol<T>(&mut self) -> Result<T, UsbSwitchSetupError> {
        self.phase = Phase::Failed;
        Err(UsbSwitchSetupError::Protocol)
    }

    fn save_early_response(&mut self, report: &[u8]) -> Result<(), UsbSwitchSetupError> {
        if report.len() > OUTPUT_CAPACITY {
            return self.fail_protocol();
        }
        if self.early_response.is_some() {
            return Ok(());
        }
        let mut bytes = [0; OUTPUT_CAPACITY];
        bytes[..report.len()].copy_from_slice(report);
        self.early_response = Some(EarlyResponse {
            bytes,
            len: report.len() as u8,
        });
        Ok(())
    }
}

pub(crate) fn classify_ready_input(report: &[u8]) -> SwitchInputDispatch {
    match report.first().copied() {
        Some(0x30..=0x33) => SwitchInputDispatch::StandardInput,
        Some(SWITCH_HID_INPUT_SUBCOMMAND_ACK) => {
            if parse_switch_subcommand_ack(report).is_some() {
                SwitchInputDispatch::Ignore
            } else {
                SwitchInputDispatch::Malformed
            }
        }
        Some(USB_RESPONSE_ID) => {
            if report.len() >= 2 && matches!(report[1], USB_HANDSHAKE | USB_BAUDRATE_3M) {
                SwitchInputDispatch::Ignore
            } else {
                SwitchInputDispatch::Malformed
            }
        }
        None => SwitchInputDispatch::Malformed,
        _ => SwitchInputDispatch::Unsupported,
    }
}

fn map_switch_error(error: SwitchSetupError) -> UsbSwitchSetupError {
    match error {
        SwitchSetupError::RetryLimit => UsbSwitchSetupError::Timeout,
        _ => UsbSwitchSetupError::Protocol,
    }
}

fn usb_command(command: u8) -> UsbSwitchOutput {
    let mut bytes = [0; OUTPUT_CAPACITY];
    bytes[0] = USB_REPORT_ID;
    bytes[1] = command;
    UsbSwitchOutput { bytes, len: 2 }
}

fn switch_packet(packet: SwitchOutputPacket) -> UsbSwitchOutput {
    let mut bytes = [0; OUTPUT_CAPACITY];
    let packet_bytes = packet.bytes();
    bytes[..packet_bytes.len()].copy_from_slice(packet_bytes);
    UsbSwitchOutput {
        bytes,
        len: packet_bytes.len() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    use ogc_bt_sync::switch::{
        SWITCH_FACTORY_CALIBRATION_ADDRESS, SWITCH_SUBCMD_LOW_POWER_MODE,
        SWITCH_SUBCMD_SET_REPORT_MODE, SWITCH_SUBCMD_SPI_FLASH_READ,
        SWITCH_USER_CALIBRATION_ADDRESS, SwitchCalibrationSource,
    };

    fn usb_reply(command: u8) -> [u8; 2] {
        [USB_RESPONSE_ID, command]
    }

    fn ack_report(subcommand: u8, data: &[u8], ack: bool, report_len: usize) -> Vec<u8> {
        let mut report = vec![0; report_len];
        report[0] = SWITCH_HID_INPUT_SUBCOMMAND_ACK;
        report[13] = if ack { 0x80 } else { 0 };
        report[14] = subcommand;
        report[15..15 + data.len()].copy_from_slice(data);
        report
    }

    fn spi_ack(subcommand: u8, address: u32, data: &[u8], report_len: usize) -> Vec<u8> {
        let mut echo = address.to_le_bytes().to_vec();
        echo.push(data.len() as u8);
        echo.extend_from_slice(data);
        ack_report(subcommand, &echo, true, report_len)
    }

    fn empty_user_calibration() -> [u8; 22] {
        [0; 22]
    }

    fn factory_calibration() -> [u8; 18] {
        [
            0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32, 0xd0, 0x47, 0x83, 0x84, 0x03,
            0x32, 0xe8, 0xc3, 0x44,
        ]
    }

    fn advance_to_first_switch_command(setup: &mut UsbSwitchSetup) -> u64 {
        let mut now = 0;
        for (command, response) in [(0x02, true), (0x03, true), (0x02, true)] {
            let output = setup.next_output(now).unwrap();
            assert_eq!(output.bytes(), &[0x80, command]);
            setup.output_complete(0, now + 1).unwrap();
            if response {
                assert_eq!(
                    setup.receive(&usb_reply(command), now + 2).unwrap(),
                    UsbSwitchInputProgress::Send
                );
            }
            now += 3;
        }
        let no_timeout = setup.next_output(now).unwrap();
        assert_eq!(no_timeout.bytes(), &[0x80, 0x04]);
        setup.output_complete(0, now + 1).unwrap();
        let low_power = setup.next_output(now + 2).unwrap();
        assert_eq!(low_power.bytes(), &[1, 0, 0, 1, 64, 64, 0, 1, 64, 64, 8, 0]);
        setup.output_complete(0, now + 3).unwrap();
        now + 3
    }

    #[test]
    fn switch_pro_usb_handshake_matches_each_response_before_advancing() {
        let mut setup = UsbSwitchSetup::new(10);
        let first = setup.next_output(0).unwrap();
        assert_eq!(first.bytes(), &[0x80, 0x02]);
        setup.output_complete(0, 1).unwrap();
        assert_eq!(
            setup.receive(&usb_reply(0x03), 2).unwrap(),
            UsbSwitchInputProgress::Ignore
        );
        assert!(setup.next_output(2).is_none());
        assert_eq!(
            setup.receive(&usb_reply(0x02), 3).unwrap(),
            UsbSwitchInputProgress::Send
        );
        assert_eq!(setup.next_output(4).unwrap().bytes(), &[0x80, 0x03]);
        setup.output_complete(0, 5).unwrap();
        assert_eq!(
            setup.receive(&usb_reply(0x03), 6).unwrap(),
            UsbSwitchInputProgress::Send
        );
        assert_eq!(setup.next_output(7).unwrap().bytes(), &[0x80, 0x02]);
        setup.output_complete(0, 8).unwrap();
        setup.receive(&usb_reply(0x02), 9).unwrap();
        assert_eq!(setup.next_output(10).unwrap().bytes(), &[0x80, 0x04]);
        setup.output_complete(0, 11).unwrap();
        assert_eq!(
            setup.next_output(12).unwrap().bytes()[10],
            SWITCH_SUBCMD_LOW_POWER_MODE
        );
    }

    #[test]
    fn early_handshake_reply_waits_for_successful_output_completion() {
        let mut setup = UsbSwitchSetup::new(10);
        setup.next_output(0).unwrap();
        assert_eq!(
            setup.receive(&usb_reply(USB_HANDSHAKE), 1).unwrap(),
            UsbSwitchInputProgress::Ignore
        );
        assert_eq!(
            setup.output_complete(1, 2),
            Ok(Some(UsbSwitchInputProgress::Send))
        );
        assert_eq!(
            setup.next_output(3).unwrap().bytes(),
            &[0x80, USB_BAUDRATE_3M]
        );

        let mut failed = UsbSwitchSetup::new(10);
        failed.next_output(0).unwrap();
        failed.receive(&usb_reply(USB_HANDSHAKE), 1).unwrap();
        assert_eq!(
            failed.output_complete(-5, 2),
            Err(UsbSwitchSetupError::Output(-5))
        );
        assert!(failed.next_output(3).is_none());

        let mut subcommand = UsbSwitchSetup::new(10);
        let mut now = advance_to_first_switch_command(&mut subcommand);
        subcommand
            .receive(
                &ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], true, 64),
                now,
            )
            .unwrap();
        subcommand.next_output(now).unwrap();
        let early_ack = ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true, 64);
        assert_eq!(
            subcommand.receive(&early_ack, now + 1).unwrap(),
            UsbSwitchInputProgress::Ignore
        );
        assert_eq!(
            subcommand.output_complete(1, now + 2),
            Ok(Some(UsbSwitchInputProgress::Send))
        );
        now += 3;
        assert_eq!(
            subcommand.next_output(now).unwrap().bytes()[10],
            SWITCH_SUBCMD_SPI_FLASH_READ
        );
    }

    #[test]
    fn switch_pro_usb_waits_for_valid_calibration_and_accepts_padded_64_byte_reports() {
        let mut setup = UsbSwitchSetup::new(10);
        let mut now = advance_to_first_switch_command(&mut setup);
        let report_mode = ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], true, 64);
        assert_eq!(
            setup.receive(&report_mode, now).unwrap(),
            UsbSwitchInputProgress::Send
        );
        let packet = setup.next_output(now).unwrap();
        assert_eq!(packet.bytes()[10], SWITCH_SUBCMD_SET_REPORT_MODE);
        setup.output_complete(0, now + 1).unwrap();
        now += 2;

        let calibration_request = ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true, 64);
        assert_eq!(
            setup.receive(&calibration_request, now).unwrap(),
            UsbSwitchInputProgress::Send
        );
        let packet = setup.next_output(now).unwrap();
        assert_eq!(packet.bytes()[10], SWITCH_SUBCMD_SPI_FLASH_READ);
        setup.output_complete(0, now + 1).unwrap();
        now += 2;

        let user = spi_ack(
            SWITCH_SUBCMD_SPI_FLASH_READ,
            SWITCH_USER_CALIBRATION_ADDRESS,
            &empty_user_calibration(),
            64,
        );
        assert_eq!(
            setup.receive(&user, now).unwrap(),
            UsbSwitchInputProgress::Send
        );
        let factory_packet = setup.next_output(now).unwrap();
        assert_eq!(factory_packet.bytes()[10], SWITCH_SUBCMD_SPI_FLASH_READ);
        setup.output_complete(0, now + 1).unwrap();
        now += 2;

        let factory = spi_ack(
            SWITCH_SUBCMD_SPI_FLASH_READ,
            SWITCH_FACTORY_CALIBRATION_ADDRESS,
            &factory_calibration(),
            64,
        );
        let UsbSwitchInputProgress::Ready(calibration) = setup.receive(&factory, now).unwrap()
        else {
            panic!("both valid stick calibrations must mark setup ready");
        };
        assert!(setup.is_ready());
        assert_eq!(
            calibration.left.unwrap().source,
            SwitchCalibrationSource::Factory
        );
        assert_eq!(
            calibration.right.unwrap().source,
            SwitchCalibrationSource::Factory
        );
    }

    #[test]
    fn ready_dispatch_ignores_only_validated_status_and_ack_reports() {
        let mut standard = [0u8; 64];
        standard[0] = 0x30;
        assert_eq!(
            classify_ready_input(&standard),
            SwitchInputDispatch::StandardInput
        );
        assert_eq!(
            classify_ready_input(&ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true, 64)),
            SwitchInputDispatch::Ignore
        );
        let mut response = [0; 64];
        response[0] = USB_RESPONSE_ID;
        response[1] = USB_HANDSHAKE;
        response[2] = 0x42;
        assert_eq!(classify_ready_input(&response), SwitchInputDispatch::Ignore);
        response[1] = 0xff;
        assert_eq!(
            classify_ready_input(&response),
            SwitchInputDispatch::Malformed
        );
        assert_eq!(
            classify_ready_input(&[SWITCH_HID_INPUT_SUBCOMMAND_ACK]),
            SwitchInputDispatch::Malformed
        );
        assert_eq!(
            classify_ready_input(&[0x3f]),
            SwitchInputDispatch::Unsupported
        );
        assert_eq!(classify_ready_input(&[]), SwitchInputDispatch::Malformed);
        assert_eq!(
            classify_ready_input(&[0x99]),
            SwitchInputDispatch::Unsupported
        );
    }

    #[test]
    fn switch_pro_usb_transport_errors_nack_and_bad_echo_fail() {
        let mut output_error = UsbSwitchSetup::new(10);
        output_error.next_output(0).unwrap();
        assert_eq!(
            output_error.output_complete(-5, 1),
            Err(UsbSwitchSetupError::Output(-5))
        );
        assert!(output_error.next_output(2).is_none());

        let mut positive_output = UsbSwitchSetup::new(10);
        positive_output.next_output(0).unwrap();
        assert_eq!(positive_output.output_complete(2, 1), Ok(None));

        let mut read_error = UsbSwitchSetup::new(10);
        read_error.next_output(0).unwrap();
        read_error.output_complete(0, 1).unwrap();
        assert_eq!(read_error.input_failed(-9), UsbSwitchSetupError::Input(-9));

        let mut nack = UsbSwitchSetup::new(10);
        let mut now = advance_to_first_switch_command(&mut nack);
        let nack_report = ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], false, 64);
        assert_eq!(
            nack.receive(&nack_report, now),
            Err(UsbSwitchSetupError::Protocol)
        );
        assert!(nack.next_output(now + 1).is_none());

        let mut bad_echo = UsbSwitchSetup::new(10);
        now = advance_to_first_switch_command(&mut bad_echo);
        let low_power = ack_report(SWITCH_SUBCMD_LOW_POWER_MODE, &[], true, 64);
        bad_echo.receive(&low_power, now).unwrap();
        let report_mode = bad_echo.next_output(now).unwrap();
        bad_echo.output_complete(0, now + 1).unwrap();
        now += 2;
        bad_echo
            .receive(
                &ack_report(SWITCH_SUBCMD_SET_REPORT_MODE, &[], true, 64),
                now,
            )
            .unwrap();
        bad_echo.next_output(now).unwrap();
        bad_echo.output_complete(0, now + 1).unwrap();
        let wrong_echo = spi_ack(
            SWITCH_SUBCMD_SPI_FLASH_READ,
            SWITCH_USER_CALIBRATION_ADDRESS + 1,
            &empty_user_calibration(),
            64,
        );
        assert_eq!(
            bad_echo.receive(&wrong_echo, now + 2),
            Err(UsbSwitchSetupError::Protocol)
        );
        assert!(bad_echo.next_output(now + 3).is_none());
        let _ = report_mode;
    }

    #[test]
    fn switch_pro_usb_retries_timeouts_once_using_ticks() {
        let mut handshake = UsbSwitchSetup::new(10);
        handshake.next_output(100).unwrap();
        handshake.output_complete(0, 101).unwrap();
        assert_eq!(
            handshake.timeout_operation(),
            "Switch USB handshake response timeout"
        );
        assert!(handshake.expire(111).is_ok());
        assert_eq!(handshake.next_output(111).unwrap().bytes(), &[0x80, 0x02]);
        assert_eq!(
            handshake.timeout_operation(),
            "Switch USB write wait timeout"
        );
        handshake.output_complete(0, 112).unwrap();
        assert_eq!(handshake.expire(122), Err(UsbSwitchSetupError::Timeout));

        let mut subcommand = UsbSwitchSetup::new(10);
        let now = advance_to_first_switch_command(&mut subcommand);
        assert_eq!(
            subcommand.timeout_operation(),
            "Switch USB subcommand response timeout"
        );
        assert!(subcommand.expire(now + 10).is_ok());
        let retry = subcommand.next_output(now + 10).unwrap();
        assert_eq!(retry.bytes()[10], SWITCH_SUBCMD_LOW_POWER_MODE);
        assert_eq!(retry.bytes()[1], 1);
        subcommand.output_complete(0, now + 11).unwrap();
        assert_eq!(
            subcommand.expire(now + 21),
            Err(UsbSwitchSetupError::Timeout)
        );
    }
}
