//! Production stateful decoder for descriptor-backed USB HID input.

extern crate alloc;

use crate::hid::{HidError, HidReportMap};
use crate::reports::map_hat;
use crate::{Axes, Buttons};
#[cfg(any(test, target_arch = "powerpc"))]
use crate::{ControllerKind, ControllerSource, ControllerState};
#[cfg(any(test, target_arch = "powerpc"))]
use alloc::boxed::Box;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbHidError {
    Descriptor(HidError),
    NotController,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbHidProfileError {
    InvalidIdentity,
    InvalidBinding,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) struct UsbHidInterfaceCandidate {
    pub configuration: u8,
    pub interface: u8,
    pub alternate_setting: u8,
    pub endpoint: u8,
    pub endpoint_out: Option<u8>,
    pub endpoint_out_size: u16,
    pub endpoint_size: u16,
}

#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) const fn is_switch_pro_usb_interface(candidate: UsbHidInterfaceCandidate) -> bool {
    candidate.endpoint_size == 64
        && candidate.endpoint_out.is_some()
        && candidate.endpoint_out_size == 64
}

#[derive(Debug)]
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) struct UsbHidSelectedInterface {
    pub candidate: UsbHidInterfaceCandidate,
    pub decoder: Box<UsbHidDecoder>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) enum UsbHidSelectionError {
    Descriptor(HidError),
    Unsupported,
}

#[derive(Debug, Eq, PartialEq)]
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) enum UsbHidInterfaceLoadError<E> {
    Transport(E),
    Selection(UsbHidSelectionError),
}

/// Try eligible HID interfaces in descriptor order. Application collections
/// identify the actual game controller; a keyboard or mouse interface before
/// it does not prevent a later controller interface from being selected.
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) struct UsbHidInterfaceSelector {
    bindings: HidButtonBindings,
    selected: Option<UsbHidSelectedInterface>,
    first_descriptor_error: Option<HidError>,
}

#[cfg(any(test, target_arch = "powerpc"))]
impl UsbHidInterfaceSelector {
    pub fn new(bindings: HidButtonBindings) -> Self {
        Self {
            bindings,
            selected: None,
            first_descriptor_error: None,
        }
    }

    pub fn consider(&mut self, candidate: UsbHidInterfaceCandidate, descriptor: &[u8]) {
        if self.selected.is_some() {
            return;
        }
        match UsbHidDecoder::parse_with_bindings(descriptor, self.bindings) {
            Ok(decoder)
                if decoder.maximum_input_bytes() > 0
                    && decoder.maximum_input_bytes() <= candidate.endpoint_size as usize
                    && decoder.maximum_input_bytes() <= 64 =>
            {
                self.selected = Some(UsbHidSelectedInterface {
                    candidate,
                    decoder: Box::new(decoder),
                });
            }
            Ok(_) | Err(UsbHidError::NotController) => {}
            Err(UsbHidError::Descriptor(error)) => {
                self.first_descriptor_error.get_or_insert(error);
            }
        }
    }

    pub fn is_selected(&self) -> bool {
        self.selected.is_some()
    }

    pub fn finish(self) -> Result<UsbHidSelectedInterface, UsbHidSelectionError> {
        self.selected.ok_or_else(|| {
            self.first_descriptor_error
                .map(UsbHidSelectionError::Descriptor)
                .unwrap_or(UsbHidSelectionError::Unsupported)
        })
    }
}

/// Load candidate descriptors in order and return the first supported
/// controller interface. A transport failure stops selection and remains
/// visible to the caller; it is never treated as an unsupported descriptor.
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) fn select_controller_interface<E>(
    candidates: &[UsbHidInterfaceCandidate],
    bindings: HidButtonBindings,
    mut load_descriptor: impl FnMut(UsbHidInterfaceCandidate) -> Result<Option<alloc::vec::Vec<u8>>, E>,
) -> Result<UsbHidSelectedInterface, UsbHidInterfaceLoadError<E>> {
    let mut selector = UsbHidInterfaceSelector::new(bindings);
    for candidate in candidates.iter().copied() {
        let Some(descriptor) =
            load_descriptor(candidate).map_err(UsbHidInterfaceLoadError::Transport)?
        else {
            continue;
        };
        selector.consider(candidate, &descriptor);
        if selector.is_selected() {
            break;
        }
    }
    selector
        .finish()
        .map_err(UsbHidInterfaceLoadError::Selection)
}

/// One parsed USB report plus the merged state after this report. Raw usage
/// pages, usages, and values remain available to identified device profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbHidDecoded {
    pub buttons: Buttons,
    pub axes: Axes,
    pub controls: crate::hid::HidInput,
    pub capabilities: crate::hid::HidControllerCapabilities,
    pub controller_report: bool,
}

/// One decoded port from the Nintendo Wii U GameCube USB adapter report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbGameCubePortReport {
    pub connected: bool,
    pub buttons: Buttons,
    pub axes: crate::Axes,
}

/// Validate a completed USB input length before slicing the IOS report buffer.
/// Zero means that the completed operation carried no controller report.
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) fn validate_usb_input_length(
    length: usize,
    report_capacity: usize,
    endpoint_size: usize,
) -> Result<usize, HidError> {
    if length == 0 || length > report_capacity || length > endpoint_size {
        return Err(HidError::ReportLength);
    }
    Ok(length)
}

/// Decode the adapter's report ID plus four nine-byte port records.
///
/// The byte offsets and connection flags match Nintendont's 057e:0337
/// controller configuration and PADReadGC implementation.
pub fn decode_usb_gamecube_adapter_report(
    bytes: &[u8],
) -> Result<[UsbGameCubePortReport; 4], HidError> {
    if bytes.len() != 37 {
        return Err(HidError::ReportLength);
    }
    if bytes[0] != 0x21 {
        return Err(HidError::InvalidReportId);
    }
    let mut ports = [UsbGameCubePortReport {
        connected: false,
        buttons: Buttons::empty(),
        axes: crate::Axes::default(),
    }; 4];
    for (port, decoded) in ports.iter_mut().enumerate() {
        let offset = 1 + port * 9;
        let status = bytes[offset];
        decoded.connected = status & 0x10 != 0 || status & 0x22 == 0x22;
        if !decoded.connected {
            continue;
        }
        let controls = bytes[offset + 2];
        let dpad = bytes[offset + 1];
        let mut buttons = Buttons::empty();
        for (mask, button) in [
            (0x01, Buttons::A),
            (0x02, Buttons::B),
            (0x04, Buttons::X),
            (0x08, Buttons::Y),
            (0x10, Buttons::LEFT),
            (0x20, Buttons::RIGHT),
            (0x40, Buttons::DOWN),
            (0x80, Buttons::UP),
        ] {
            if dpad & mask != 0 {
                buttons.insert(button);
            }
        }
        for (mask, button) in [
            (0x02, Buttons::Z),
            (0x08, Buttons::L),
            (0x04, Buttons::R),
            (0x01, Buttons::START),
        ] {
            if controls & mask != 0 {
                buttons.insert(button);
            }
        }
        decoded.buttons = buttons;
        decoded.axes = crate::Axes {
            left_x: (bytes[offset + 3] as i16 - 128) * 256,
            left_y: (bytes[offset + 4] as i16 - 128) * 256,
            right_x: (bytes[offset + 5] as i16 - 128) * 256,
            right_y: (bytes[offset + 6] as i16 - 128) * 256,
            left_trigger: bytes[offset + 7],
            right_trigger: bytes[offset + 8],
        }
        .clamp_normalized();
    }
    Ok(ports)
}

/// Send the required adapter initialization report through the caller's real
/// interrupt output transport and preserve a negative IOS result.
#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) fn initialize_usb_gamecube_adapter(
    mut write_interrupt_report: impl FnMut(&[u8]) -> i32,
) -> Result<(), i32> {
    let result = write_interrupt_report(&[0x13]);
    if result < 0 { Err(result) } else { Ok(()) }
}

#[cfg(any(test, target_arch = "powerpc"))]
pub(crate) fn usb_gamecube_port_states(
    adapter_slot: u8,
    ports: [UsbGameCubePortReport; 4],
) -> [ControllerState; 4] {
    core::array::from_fn(|port| {
        let decoded = ports[port];
        ControllerState {
            source: ControllerSource::UsbGameCube {
                adapter_slot,
                port: port as u8,
            },
            kind: ControllerKind::GameCube,
            connected: decoded.connected,
            buttons: decoded.buttons,
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: decoded.axes,
        }
    })
}

/// Explicit mapping from HID Button usages to this crate's logical buttons.
/// The empty default avoids assigning a physical meaning to Button 1, which
/// HID does not define. A device profile can bind usages after identification.
#[derive(Clone, Copy, Debug)]
pub struct HidButtonBindings([Buttons; 64]);

impl HidButtonBindings {
    pub const fn empty() -> Self {
        Self([Buttons::empty(); 64])
    }

    pub fn bind(&mut self, usage: u16, button: Buttons) -> bool {
        if !(1..=64).contains(&usage) || button.is_empty() {
            return false;
        }
        self.0[usage as usize - 1] = button;
        true
    }
}

impl Default for HidButtonBindings {
    fn default() -> Self {
        Self::empty()
    }
}

/// Holds decoded state across report IDs. Only controls present in each report
/// update; absent controls retain their last values. Any error clears state.
#[derive(Debug)]
pub struct UsbHidDecoder {
    map: HidReportMap,
    capabilities: crate::hid::HidControllerCapabilities,
    bindings: HidButtonBindings,
    buttons: Buttons,
    axes: Axes,
    held_button_usages: u64,
    dpad: u8,
    hat_buttons: Buttons,
}

impl UsbHidDecoder {
    pub fn parse(descriptor: &[u8]) -> Result<Self, UsbHidError> {
        Self::parse_with_bindings(descriptor, HidButtonBindings::empty())
    }

    /// Parse with button bindings selected by an identified device profile.
    /// HID Button usages do not imply physical button meanings.
    pub fn parse_with_bindings(
        descriptor: &[u8],
        bindings: HidButtonBindings,
    ) -> Result<Self, UsbHidError> {
        let map = HidReportMap::parse(descriptor).map_err(UsbHidError::Descriptor)?;
        if !map.has_game_controller_application() {
            return Err(UsbHidError::NotController);
        }
        let capabilities = map.game_controller_capabilities();
        Ok(Self {
            map,
            capabilities,
            bindings,
            buttons: Buttons::empty(),
            axes: Axes::default(),
            held_button_usages: 0,
            dpad: 0,
            hat_buttons: Buttons::empty(),
        })
    }

    pub fn maximum_input_bytes(&self) -> usize {
        self.map.maximum_input_bytes()
    }

    pub fn bind_button(&mut self, usage: u16, button: Buttons) -> bool {
        let bound = self.bindings.bind(usage, button);
        if bound {
            self.refresh_buttons();
        }
        bound
    }

    pub fn decode(&mut self, bytes: &[u8]) -> Result<(Buttons, Axes), HidError> {
        self.decode_with_controls(bytes)
            .map(|decoded| (decoded.buttons, decoded.axes))
    }

    /// Decode a report and retain raw control data for a device profile or
    /// probe. Other application reports in a mixed interface do not change
    /// the controller state.
    pub fn decode_with_controls(&mut self, bytes: &[u8]) -> Result<UsbHidDecoded, HidError> {
        let controller_report = match self.map.is_game_controller_report(bytes) {
            Ok(controller_report) => controller_report,
            Err(error) => {
                self.reset();
                return Err(error);
            }
        };
        let input = match self.map.decode(bytes) {
            Ok(input) => input,
            Err(error) => {
                if controller_report {
                    self.reset();
                }
                return Err(error);
            }
        };
        if controller_report {
            self.merge(&input);
        }
        Ok(UsbHidDecoded {
            buttons: self.buttons,
            axes: self.axes,
            controls: input,
            capabilities: self.capabilities,
            controller_report,
        })
    }

    pub fn reset(&mut self) {
        self.buttons = Buttons::empty();
        self.axes = Axes::default();
        self.held_button_usages = 0;
        self.dpad = 0;
        self.hat_buttons = Buttons::empty();
    }

    fn merge(&mut self, input: &crate::hid::HidInput) {
        // Button usages have no standard face-button meaning. Only update
        // actions that an identified profile explicitly bound.
        for usage in 1..=64 {
            let mask = 1u64 << (usage - 1);
            if input.present_button_usages & mask == 0 {
                continue;
            }
            if input.button_usages & mask != 0 {
                self.held_button_usages |= mask;
            } else {
                self.held_button_usages &= !mask;
            }
        }
        if input.hat_present {
            self.hat_buttons = Buttons::empty();
            if let Some(hat) = input.hat {
                map_hat(&mut self.hat_buttons, hat);
            }
        }
        for mask in [1, 2, 4, 8] {
            if input.dpad_present & mask != 0 {
                if input.dpad & mask != 0 {
                    self.dpad |= mask;
                } else {
                    self.dpad &= !mask;
                }
            }
        }
        self.refresh_buttons();
        for (index, value) in input.axes.iter().copied().enumerate() {
            if input.present_axes & (1 << index) == 0 || input.relative_axes & (1 << index) != 0 {
                continue;
            }
            match index {
                0 => self.axes.left_x = value,
                1 => self.axes.left_y = value.saturating_neg(),
                3 => self.axes.right_x = value,
                4 => self.axes.right_y = value.saturating_neg(),
                _ => {}
            }
        }
        self.axes = self.axes.clamp_normalized();
    }

    fn refresh_buttons(&mut self) {
        let mut buttons = self.hat_buttons;
        for (mask, button) in [
            (1, Buttons::UP),
            (2, Buttons::DOWN),
            (4, Buttons::RIGHT),
            (8, Buttons::LEFT),
        ] {
            if self.dpad & mask != 0 {
                buttons |= button;
            }
        }
        for usage in 1..=64 {
            let mask = 1u64 << (usage - 1);
            if self.held_button_usages & mask != 0 {
                buttons |= self.bindings.0[usage - 1];
            }
        }
        self.buttons = buttons;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamecube_adapter_initialization_checks_interrupt_result() {
        let mut output = None;
        let result = initialize_usb_gamecube_adapter(|report| {
            output = Some(report.to_vec());
            0
        });
        assert_eq!(result, Ok(()));
        assert_eq!(output, Some(alloc::vec![0x13]));

        let mut output = None;
        assert_eq!(
            initialize_usb_gamecube_adapter(|report| {
                output = Some(report.to_vec());
                -9
            }),
            Err(-9)
        );
        assert_eq!(output, Some(alloc::vec![0x13]));
    }

    #[test]
    fn gamecube_adapter_decodes_four_port_records_and_releases_disconnected_ports() {
        let mut report = [0u8; 37];
        report[0] = 0x21;
        report[1] = 0x10;
        report[2] = 0x11; // A and left
        report[3] = 0x0b; // Z, L, and start
        report[4] = 0x00;
        report[5] = 0xff;
        report[6] = 0x80;
        report[7] = 0x80;
        report[8] = 0x00;
        report[9] = 0xff;
        report[10] = 0x22; // WaveBird communicating state
        report[13..17].fill(0x80);

        let ports = decode_usb_gamecube_adapter_report(&report).unwrap();
        assert_eq!(
            ports[0],
            UsbGameCubePortReport {
                connected: true,
                buttons: Buttons::A | Buttons::LEFT | Buttons::Z | Buttons::L | Buttons::START,
                axes: Axes {
                    left_x: -32767,
                    left_y: 32512,
                    right_x: 0,
                    right_y: 0,
                    left_trigger: 0,
                    right_trigger: 255,
                },
            }
        );
        assert_eq!(
            ports[1],
            UsbGameCubePortReport {
                connected: true,
                buttons: Buttons::empty(),
                axes: Axes::default(),
            }
        );
        assert!(ports[2..].iter().all(|port| !port.connected
            && port.buttons.is_empty()
            && port.axes == Axes::default()));

        report[1] = 0;
        let disconnected = decode_usb_gamecube_adapter_report(&report).unwrap();
        assert_eq!(disconnected[0].buttons, Buttons::empty());
        assert_eq!(disconnected[0].axes, Axes::default());
        assert!(!disconnected[0].connected);
    }

    #[test]
    fn gamecube_adapter_rejects_short_reports() {
        assert_eq!(
            decode_usb_gamecube_adapter_report(&[0; 36]),
            Err(HidError::ReportLength)
        );
    }

    #[test]
    fn usb_input_length_rejects_empty_and_out_of_bounds_completions() {
        assert_eq!(
            validate_usb_input_length(0, 64, 64),
            Err(HidError::ReportLength)
        );
        assert_eq!(
            validate_usb_input_length(65, 64, 128),
            Err(HidError::ReportLength)
        );
        assert_eq!(
            validate_usb_input_length(65, 128, 64),
            Err(HidError::ReportLength)
        );
        assert_eq!(validate_usb_input_length(37, 64, 64), Ok(37));
    }

    #[test]
    fn gamecube_adapter_rejects_wrong_report_id_and_padded_reports() {
        let mut report = [0u8; 37];
        assert_eq!(
            decode_usb_gamecube_adapter_report(&report),
            Err(HidError::InvalidReportId)
        );
        report[0] = 0x21;
        let mut padded = [0u8; 38];
        padded[..37].copy_from_slice(&report);
        assert_eq!(
            decode_usb_gamecube_adapter_report(&padded),
            Err(HidError::ReportLength)
        );
    }

    #[test]
    fn gamecube_adapter_ports_have_independent_transition_identity_and_disconnect_release() {
        let mut transitions = InputTransitions::<MAX_USB_GAMECUBE_PORTS>::new();
        let report = |connected: [bool; 4], buttons: [u8; 4]| {
            let mut report = [0u8; 37];
            report[0] = 0x21;
            for port in 0..4 {
                let offset = 1 + port * 9;
                report[offset] = if connected[port] { 0x10 } else { 0 };
                report[offset + 1] = buttons[port];
                report[offset + 3..offset + 7].fill(0x80);
            }
            report
        };
        let sample_ports = |transitions: &mut InputTransitions<MAX_USB_GAMECUBE_PORTS>,
                            states: [ControllerState; 4]| {
            for (port, state) in states.into_iter().enumerate() {
                transitions.sample(port, state).unwrap();
            }
        };
        let held_buttons = [Buttons::A, Buttons::B, Buttons::X, Buttons::Y];
        let initial = decode_usb_gamecube_adapter_report(&report([true; 4], [1, 2, 4, 8])).unwrap();
        let initial_states = usb_gamecube_port_states(0, initial);
        let mut initial_ids = [None; 4];
        for (port, state) in initial_states.into_iter().enumerate() {
            initial_ids[port] = Some(transitions.sample(port, state).unwrap());
        }
        let initial_samples: [ControllerState; 4] =
            core::array::from_fn(|slot| transitions.take(slot).unwrap());
        for (port, sample) in initial_samples.iter().enumerate() {
            assert_eq!(
                sample.source,
                ControllerSource::UsbGameCube {
                    adapter_slot: 0,
                    port: port as u8,
                }
            );
            assert_eq!(sample.buttons_down, Buttons::empty());
        }

        let released = decode_usb_gamecube_adapter_report(&report([true; 4], [0; 4])).unwrap();
        sample_ports(&mut transitions, usb_gamecube_port_states(0, released));
        for (port, expected) in held_buttons.iter().copied().enumerate() {
            assert_eq!(transitions.take(port).unwrap().buttons_up, expected);
        }

        let pressed = decode_usb_gamecube_adapter_report(&report([true; 4], [1, 2, 4, 8])).unwrap();
        sample_ports(&mut transitions, usb_gamecube_port_states(0, pressed));
        for (port, expected) in held_buttons.iter().copied().enumerate() {
            assert_eq!(transitions.take(port).unwrap().buttons_down, expected);
        }

        let live =
            decode_usb_gamecube_adapter_report(&report([true, true, false, true], [1, 2, 4, 8]))
                .unwrap();
        let live_states = usb_gamecube_port_states(0, live);
        for (port, state) in live_states.into_iter().enumerate() {
            transitions.sample(port, state).unwrap();
        }
        let disconnect = transitions.take_reset(2).unwrap();
        assert_eq!(disconnect.reason, ResetReason::Disconnected);
        assert_eq!(disconnect.state.buttons_up, Buttons::X);
        for (port, expected) in [(0, Buttons::A), (1, Buttons::B), (3, Buttons::Y)] {
            assert_eq!(transitions.take(port).unwrap().buttons, expected);
        }

        let reconnected_port =
            decode_usb_gamecube_adapter_report(&report([true; 4], [0; 4])).unwrap();
        sample_ports(
            &mut transitions,
            usb_gamecube_port_states(0, reconnected_port),
        );
        for port in 0..4 {
            transitions.take(port).unwrap();
        }

        assert_eq!(
            decode_usb_gamecube_adapter_report(&[0; 37]),
            Err(HidError::InvalidReportId)
        );
        for port in 0..4 {
            transitions.reset(port, ResetReason::InvalidReport).unwrap();
        }
        for reset in (0..4).map(|port| transitions.take_reset(port).unwrap()) {
            assert_eq!(reset.reason, ResetReason::InvalidReport);
        }

        let old_generation = initial_ids[0].unwrap().generation;
        let reconnect =
            decode_usb_gamecube_adapter_report(&report([true; 4], [1, 2, 4, 8])).unwrap();
        let reconnect_states = usb_gamecube_port_states(0, reconnect);
        let mut reconnect_ids = [None; 4];
        for (port, state) in reconnect_states.into_iter().enumerate() {
            reconnect_ids[port] = Some(transitions.sample(port, state).unwrap());
        }
        assert_eq!(reconnect_ids[0].unwrap().generation, old_generation + 1);
        assert!(transitions.take(0).unwrap().buttons_down.is_empty());
    }
    use crate::{
        ControllerSource, ControllerState, InputTransitions, MAX_USB_GAMECUBE_PORTS, ResetReason,
    };

    const DESCRIPTOR: &[u8] = &[
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x08, 0x15,
        0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x85, 0x02, 0x05, 0x01, 0x09, 0x30,
        0x09, 0x31, 0x15, 0x81, 0x25, 0x7f, 0x75, 0x08, 0x95, 0x02, 0x81, 0x02, 0xc0,
    ];

    const SPLIT_REPORT_DESCRIPTOR: &[u8] = &[
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08,
        0x95, 0x01, 0x09, 0x30, 0x81, 0x02, 0x85, 0x02, 0x09, 0x31, 0x81, 0x02, 0x85, 0x03, 0x05,
        0x09, 0x19, 0x01, 0x29, 0x01, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02,
        0x75, 0x07, 0x95, 0x01, 0x81, 0x01, 0xc0,
    ];

    const MIXED_KEYBOARD_MOUSE_DESCRIPTOR: &[u8] = &[
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08,
        0x95, 0x01, 0x09, 0x30, 0x81, 0x02, 0xc0, 0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x85, 0x02,
        0x95, 0x02, 0x09, 0x30, 0x09, 0x31, 0x81, 0x06, 0x05, 0x09, 0x09, 0x01, 0x15, 0x00, 0x25,
        0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x75, 0x07, 0x95, 0x01, 0x81, 0x01, 0xc0, 0x05,
        0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x03, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x15, 0x00,
        0x25, 0x65, 0x75, 0x08, 0x95, 0x06, 0x81, 0x00, 0xc0,
    ];

    #[test]
    fn descriptor_capabilities_are_stable_across_split_report_ids() {
        let mut bindings = HidButtonBindings::empty();
        assert!(bindings.bind(1, Buttons::A));
        let mut decoder =
            UsbHidDecoder::parse_with_bindings(SPLIT_REPORT_DESCRIPTOR, bindings).unwrap();
        let expected = crate::hid::HidControllerCapabilities {
            buttons: true,
            left_stick: true,
            right_stick: false,
            triggers: false,
        };

        let x = decoder.decode_with_controls(&[1, 0xff]).unwrap();
        assert_eq!(x.capabilities, expected);
        assert_eq!(x.axes.left_x, 32767);
        let y = decoder.decode_with_controls(&[2, 0x00]).unwrap();
        assert_eq!(y.capabilities, expected);
        assert_eq!([y.axes.left_x, y.axes.left_y], [32767, 32767]);
        let button = decoder.decode_with_controls(&[3, 0x01]).unwrap();
        assert_eq!(button.capabilities, expected);
        assert_eq!(button.buttons, Buttons::A);
        assert_eq!([button.axes.left_x, button.axes.left_y], [32767, 32767]);
        assert_eq!(button.controls.present_axes, 0);
        assert_eq!(button.controls.present_button_usages, 1);
    }

    #[test]
    fn hid_map_exposes_controller_application_and_report_capabilities() {
        let map = crate::hid::HidReportMap::parse(SPLIT_REPORT_DESCRIPTOR).unwrap();
        assert!(map.has_game_controller_application());
        assert_eq!(map.maximum_input_bytes(), 2);
        assert_eq!(
            map.game_controller_capabilities(),
            crate::hid::HidControllerCapabilities {
                buttons: true,
                left_stick: true,
                right_stick: false,
                triggers: false,
            }
        );
        assert!(map.is_game_controller_report(&[1, 0]).unwrap());
        assert!(map.is_game_controller_report(&[2, 0]).unwrap());
        assert_eq!(
            map.is_game_controller_report(&[]),
            Err(HidError::ReportLength)
        );

        let mixed = crate::hid::HidReportMap::parse(MIXED_KEYBOARD_MOUSE_DESCRIPTOR).unwrap();
        assert!(mixed.has_game_controller_application());
        assert!(mixed.is_game_controller_report(&[1, 0]).unwrap());
        assert!(!mixed.is_game_controller_report(&[2, 0, 0]).unwrap());
        assert!(
            !mixed
                .is_game_controller_report(&[3, 0, 0, 0, 0, 0, 0])
                .unwrap()
        );
    }

    #[test]
    fn descriptor_capabilities_ignore_keyboard_and_mouse_collection_fields() {
        let mut decoder = UsbHidDecoder::parse(MIXED_KEYBOARD_MOUSE_DESCRIPTOR).unwrap();
        let expected = crate::hid::HidControllerCapabilities::default();
        let controller_axis = decoder.decode_with_controls(&[1, 0xff]).unwrap();
        assert!(controller_axis.controller_report);
        assert_eq!(controller_axis.capabilities, expected);

        let mouse = decoder
            .decode_with_controls(&[2, 0x10, 0x20, 0x01])
            .unwrap();
        assert!(!mouse.controller_report);
        assert_eq!(mouse.capabilities, expected);
        assert_eq!(mouse.axes, controller_axis.axes);

        let keyboard = decoder
            .decode_with_controls(&[3, 0x04, 0, 0, 0, 0, 0])
            .unwrap();
        assert!(!keyboard.controller_report);
        assert_eq!(keyboard.capabilities, expected);
        assert_eq!(keyboard.axes, controller_axis.axes);
    }

    #[test]
    fn merges_partial_reports_and_uses_only_explicit_button_bindings() {
        let mut bindings = HidButtonBindings::empty();
        assert!(bindings.bind(1, Buttons::A));
        let mut decoder = UsbHidDecoder::parse_with_bindings(DESCRIPTOR, bindings).unwrap();
        assert_eq!(decoder.maximum_input_bytes(), 3);
        let expected_down = (Buttons::A, Axes::default());
        let pressed = decoder.decode_with_controls(&[1, 1]).unwrap();
        assert_eq!((pressed.buttons, pressed.axes), expected_down);
        assert!(pressed.controller_report);
        assert_eq!(pressed.controls.button_usages, 1);
        assert_eq!(pressed.controls.controls[0].usage_page, 9);
        assert_eq!(pressed.controls.controls[0].usage, 1);
        assert_eq!(pressed.controls.controls[0].value, 1);
        let expected_axis = (
            Buttons::A,
            Axes {
                left_x: 32767,
                left_y: 32767,
                ..Axes::default()
            },
        );
        assert_eq!(decoder.decode(&[2, 0x7f, 0x81]).unwrap(), expected_axis);
        assert_eq!(
            decoder.decode(&[1, 0]).unwrap(),
            (Buttons::empty(), expected_axis.1)
        );
    }

    #[test]
    fn aliased_usages_remain_held_until_all_bound_usages_release() {
        let mut bindings = HidButtonBindings::empty();
        assert!(bindings.bind(1, Buttons::A));
        assert!(bindings.bind(2, Buttons::A));
        let mut decoder = UsbHidDecoder::parse_with_bindings(DESCRIPTOR, bindings).unwrap();
        assert_eq!(decoder.decode(&[1, 3]).unwrap().0, Buttons::A);
        assert_eq!(decoder.decode(&[1, 2]).unwrap().0, Buttons::A);
        assert_eq!(decoder.decode(&[1, 0]).unwrap().0, Buttons::empty());
    }

    #[test]
    fn rebinding_a_held_usage_recomputes_the_logical_state() {
        let mut decoder = UsbHidDecoder::parse(DESCRIPTOR).unwrap();
        assert_eq!(decoder.decode(&[1, 1]).unwrap().0, Buttons::empty());
        assert!(decoder.bind_button(1, Buttons::A));
        assert_eq!(decoder.decode(&[1, 1]).unwrap().0, Buttons::A);
        assert!(decoder.bind_button(1, Buttons::B));
        assert_eq!(decoder.decode(&[1, 1]).unwrap().0, Buttons::B);
    }

    #[test]
    fn generic_axis_defaults_preserve_z_and_map_rx_ry_to_right_stick() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x04, 0x09, 0x32, 0x09, 0x33, 0x09, 0x34,
            0x09, 0x35, 0x15, 0x81, 0x25, 0x7f, 0x75, 0x08, 0x95, 0x04, 0x81, 0x02, 0xc0,
        ];
        let mut decoder = UsbHidDecoder::parse(&descriptor).unwrap();
        let decoded = decoder
            .decode_with_controls(&[4, 0x11, 0x7f, 0x81, 0x22])
            .unwrap();
        assert_eq!(
            decoded.axes,
            Axes {
                right_x: 32767,
                right_y: 32767,
                ..Axes::default()
            }
        );
        assert!(
            decoded.controls.controls[..decoded.controls.control_count as usize]
                .iter()
                .any(|control| control.usage == 0x32 && control.value == 0x11)
        );
        assert!(
            decoded.controls.controls[..decoded.controls.control_count as usize]
                .iter()
                .any(|control| control.usage == 0x35 && control.value == 0x22)
        );
    }

    #[test]
    fn hat_and_dpad_sources_do_not_clear_each_other() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x09, 0x39, 0x15, 0x01, 0x25, 0x08,
            0x35, 0x00, 0x46, 0x3b, 0x01, 0x75, 0x04, 0x95, 0x01, 0x81, 0x42, 0x85, 0x02, 0x19,
            0x90, 0x29, 0x93, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x04, 0x81, 0x02, 0xc0,
        ];
        let mut decoder = UsbHidDecoder::parse(&descriptor).unwrap();
        assert_eq!(
            decoder.decode(&[1, 2]).unwrap().0,
            Buttons::UP | Buttons::RIGHT
        );
        assert_eq!(
            decoder.decode(&[2, 8]).unwrap().0,
            Buttons::UP | Buttons::RIGHT | Buttons::LEFT
        );
        assert_eq!(decoder.decode(&[1, 0]).unwrap().0, Buttons::LEFT);
    }

    #[test]
    fn malformed_and_unknown_reports_clear_state_before_returning_error() {
        let mut decoder = UsbHidDecoder::parse(DESCRIPTOR).unwrap();
        decoder.bind_button(1, Buttons::A);
        decoder.decode(&[1, 1]).unwrap();
        assert_eq!(
            decoder.decode(&[0xff, 0]).unwrap_err(),
            HidError::UnknownReportId
        );
        assert_eq!(decoder.decode(&[1]), Err(HidError::ReportLength));
        assert_eq!(
            decoder.decode(&[1, 0]).unwrap(),
            (Buttons::empty(), Axes::default())
        );
    }

    #[test]
    fn mixed_keyboard_report_does_not_establish_or_change_controller_state() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x09, 0x09, 0x01, 0x15, 0x00,
            0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0xc0, 0x05, 0x01, 0x09, 0x05, 0xa1,
            0x01, 0x85, 0x02, 0x05, 0x09, 0x09, 0x01, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95,
            0x01, 0x81, 0x02, 0xc0,
        ];
        let mut bindings = HidButtonBindings::empty();
        assert!(bindings.bind(1, Buttons::A));
        let mut decoder = UsbHidDecoder::parse_with_bindings(&descriptor, bindings).unwrap();

        let keyboard = decoder.decode_with_controls(&[1, 1]).unwrap();
        assert!(!keyboard.controller_report);
        assert_eq!(keyboard.buttons, Buttons::empty());
        assert_eq!(keyboard.axes, Axes::default());

        let gamepad = decoder.decode_with_controls(&[2, 1]).unwrap();
        assert!(gamepad.controller_report);
        assert_eq!(gamepad.buttons, Buttons::A);
    }

    #[test]
    fn reconnect_clears_decoder_and_uses_the_transition_held_input_gate() {
        let source = ControllerSource::UsbHid(0);
        let state = |buttons| ControllerState {
            source,
            connected: true,
            buttons,
            ..ControllerState::empty()
        };
        let mut decoder = UsbHidDecoder::parse(DESCRIPTOR).unwrap();
        decoder.bind_button(1, Buttons::A);
        let mut transitions = InputTransitions::<1>::new();
        transitions.sample(0, state(Buttons::empty())).unwrap();
        transitions
            .sample(0, state(decoder.decode(&[1, 1]).unwrap().0))
            .unwrap();
        assert_eq!(transitions.take(0).unwrap().buttons_down, Buttons::A);

        decoder.reset();
        transitions.reset(0, ResetReason::Disconnected).unwrap();
        assert_eq!(
            transitions.take_reset(0).unwrap().state.buttons_up,
            Buttons::A
        );

        let mut reconnected = UsbHidDecoder::parse(DESCRIPTOR).unwrap();
        reconnected.bind_button(1, Buttons::A);
        let held = reconnected.decode(&[1, 1]).unwrap().0;
        transitions.sample(0, state(held)).unwrap();
        assert_eq!(transitions.take(0).unwrap().buttons_down, Buttons::empty());
    }

    #[test]
    fn keyboard_or_mouse_only_descriptors_are_rejected() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x09, 0xe0, 0x15, 0, 0x25, 1, 0x75, 1,
            0x95, 1, 0x81, 2, 0xc0,
        ];
        assert_eq!(
            UsbHidDecoder::parse(&descriptor).unwrap_err(),
            UsbHidError::NotController
        );
    }

    #[test]
    fn interface_selector_skips_non_controller_and_preserves_selected_altsetting() {
        const KEYBOARD: &[u8] = &[
            0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x07, 0x09, 0xe0, 0x15, 0x00,
            0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0xc0,
        ];
        let first = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 0,
            alternate_setting: 0,
            endpoint: 0x81,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 8,
        };
        let second = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 2,
            alternate_setting: 1,
            endpoint: 0x83,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 8,
        };
        let candidates = [first, second];
        let selected =
            select_controller_interface(&candidates, HidButtonBindings::empty(), |candidate| {
                Ok::<_, ()>(Some(if candidate.interface == first.interface {
                    KEYBOARD.to_vec()
                } else {
                    DESCRIPTOR.to_vec()
                }))
            })
            .unwrap();
        assert_eq!(selected.candidate, second);
        assert_eq!(selected.decoder.maximum_input_bytes(), 3);

        let selected =
            select_controller_interface(&candidates[..1], HidButtonBindings::empty(), |_| {
                Ok::<_, ()>(Some(KEYBOARD.to_vec()))
            });
        assert_eq!(
            selected.unwrap_err(),
            UsbHidInterfaceLoadError::Selection(UsbHidSelectionError::Unsupported)
        );

        let mut undersized = second;
        undersized.endpoint_size = 2;
        let selected =
            select_controller_interface(&[undersized], HidButtonBindings::empty(), |_| {
                Ok::<_, ()>(Some(DESCRIPTOR.to_vec()))
            });
        assert_eq!(
            selected.unwrap_err(),
            UsbHidInterfaceLoadError::Selection(UsbHidSelectionError::Unsupported)
        );
    }

    #[test]
    fn interface_selector_lifecycle_reports_selection_and_decoder() {
        const KEYBOARD: &[u8] = &[
            0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x09, 0xe0, 0x15, 0x00, 0x25, 0x01,
            0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0xc0,
        ];
        let candidate = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 3,
            alternate_setting: 2,
            endpoint: 0x84,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 8,
        };
        let mut selector = UsbHidInterfaceSelector::new(HidButtonBindings::empty());
        assert!(!selector.is_selected());
        selector.consider(candidate, KEYBOARD);
        assert!(!selector.is_selected());
        selector.consider(candidate, DESCRIPTOR);
        assert!(selector.is_selected());

        let selected = selector.finish().unwrap();
        assert_eq!(selected.candidate, candidate);
        assert_eq!(selected.decoder.maximum_input_bytes(), 3);
    }

    #[test]
    fn button_bindings_reject_invalid_usages_and_empty_buttons() {
        let mut bindings = HidButtonBindings::empty();
        assert!(!bindings.bind(0, Buttons::A));
        assert!(!bindings.bind(65, Buttons::A));
        assert!(!bindings.bind(1, Buttons::empty()));
        assert!(bindings.bind(64, Buttons::A));
    }

    #[test]
    fn switch_pro_usb_requires_exact_64_byte_in_and_out_endpoints() {
        let candidate = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 0,
            alternate_setting: 0,
            endpoint: 0x81,
            endpoint_out: Some(0x02),
            endpoint_out_size: 64,
            endpoint_size: 64,
        };
        assert!(is_switch_pro_usb_interface(candidate));
        assert!(!is_switch_pro_usb_interface(UsbHidInterfaceCandidate {
            endpoint_size: 63,
            ..candidate
        }));
        assert!(!is_switch_pro_usb_interface(UsbHidInterfaceCandidate {
            endpoint_size: 65,
            ..candidate
        }));
        assert!(!is_switch_pro_usb_interface(UsbHidInterfaceCandidate {
            endpoint_out_size: 63,
            ..candidate
        }));
        assert!(!is_switch_pro_usb_interface(UsbHidInterfaceCandidate {
            endpoint_out_size: 65,
            ..candidate
        }));
        assert!(!is_switch_pro_usb_interface(UsbHidInterfaceCandidate {
            endpoint_out: None,
            ..candidate
        }));
    }

    #[test]
    fn interface_selector_reports_a_bad_descriptor_when_no_controller_is_found() {
        let candidate = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 0,
            alternate_setting: 0,
            endpoint: 0x81,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 8,
        };
        let selected =
            select_controller_interface(&[candidate], HidButtonBindings::empty(), |_| {
                Ok::<_, ()>(Some(alloc::vec![0xfe]))
            });
        assert_eq!(
            selected.unwrap_err(),
            UsbHidInterfaceLoadError::Selection(UsbHidSelectionError::Descriptor(
                HidError::LongItemsUnsupported
            ))
        );
    }

    #[test]
    fn interface_selector_propagates_descriptor_transport_errors() {
        let candidate = UsbHidInterfaceCandidate {
            configuration: 1,
            interface: 0,
            alternate_setting: 0,
            endpoint: 0x81,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 8,
        };
        let result = select_controller_interface(&[candidate], HidButtonBindings::empty(), |_| {
            Err("descriptor read failed")
        });
        assert!(matches!(
            result,
            Err(UsbHidInterfaceLoadError::Transport(
                "descriptor read failed"
            ))
        ));
    }
}
