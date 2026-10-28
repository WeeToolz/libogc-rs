#[cfg(any(test, target_arch = "powerpc"))]
use crate::InputTransitions;
use crate::{ControllerId, ControllerReset, TransitionError};
/// Maximum number of GameCube controller ports reported by the manager.
pub const MAX_GAMECUBE_CONTROLLERS: usize = 4;
/// Maximum number of WPAD Bluetooth channels reported by the manager.
pub const MAX_WPAD_CONTROLLERS: usize = 4;
/// Maximum number of generic USB HID controller slots tracked by the manager.
pub const MAX_USB_CONTROLLERS: usize = 4;
/// Maximum number of logical ports behind USB GameCube adapters.
pub const MAX_USB_GAMECUBE_PORTS: usize = MAX_USB_CONTROLLERS * MAX_GAMECUBE_CONTROLLERS;
/// Maximum number of non-WPAD Bluetooth HID slots tracked by the manager.
pub const MAX_BLUETOOTH_HID_CONTROLLERS: usize = 6;
/// Total number of internal transition slots.
pub const MAX_CONTROLLERS: usize = MAX_GAMECUBE_CONTROLLERS
    + MAX_WPAD_CONTROLLERS
    + MAX_USB_CONTROLLERS
    + MAX_USB_GAMECUBE_PORTS
    + MAX_BLUETOOTH_HID_CONTROLLERS;

/// Portable controller button bitset.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Buttons {
    bits: u32,
}

impl Buttons {
    pub const A: Self = Self { bits: 1 << 0 };
    pub const B: Self = Self { bits: 1 << 1 };
    pub const X: Self = Self { bits: 1 << 2 };
    pub const Y: Self = Self { bits: 1 << 3 };
    pub const UP: Self = Self { bits: 1 << 4 };
    pub const DOWN: Self = Self { bits: 1 << 5 };
    pub const LEFT: Self = Self { bits: 1 << 6 };
    pub const RIGHT: Self = Self { bits: 1 << 7 };
    pub const START: Self = Self { bits: 1 << 8 };
    pub const SELECT: Self = Self { bits: 1 << 9 };
    pub const HOME: Self = Self { bits: 1 << 10 };
    pub const L: Self = Self { bits: 1 << 11 };
    pub const R: Self = Self { bits: 1 << 12 };
    pub const ZL: Self = Self { bits: 1 << 13 };
    pub const ZR: Self = Self { bits: 1 << 14 };
    pub const L_STICK: Self = Self { bits: 1 << 15 };
    pub const R_STICK: Self = Self { bits: 1 << 16 };
    pub const SL: Self = Self { bits: 1 << 17 };
    pub const SR: Self = Self { bits: 1 << 18 };
    pub const CAPTURE: Self = Self { bits: 1 << 19 };
    pub const Z: Self = Self { bits: 1 << 20 };

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn contains(self, button: Self) -> bool {
        self.bits & button.bits != 0
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn insert(&mut self, button: Self) {
        self.bits |= button.bits;
    }

    pub fn remove(&mut self, button: Self) {
        self.bits &= !button.bits;
    }
}

impl core::ops::BitOr for Buttons {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits | rhs.bits,
        }
    }
}

impl core::ops::BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl core::ops::BitAnd for Buttons {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits & rhs.bits,
        }
    }
}

/// Normalized analog axes and trigger values.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Axes {
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

impl Axes {
    /// Clamp stick values to the common symmetric normalized range.
    pub fn clamp_normalized(self) -> Self {
        Self {
            left_x: self.left_x.clamp(-32767, 32767),
            left_y: self.left_y.clamp(-32767, 32767),
            right_x: self.right_x.clamp(-32767, 32767),
            right_y: self.right_y.clamp(-32767, 32767),
            ..self
        }
    }
}

/// Controller family detected by a bus probe or report parser.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ControllerKind {
    Unknown,
    GameCube,
    WiiRemote,
    WiiClassic,
    WiiUPro,
    JoyConLeft,
    JoyConRight,
    JoyConPair,
    SwitchPro,
    XboxOne,
    GenericBluetoothHid,
    GenericUsbHid,
}

/// Physical or logical source for a controller state.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ControllerSource {
    Unknown,
    GameCube(u8),
    WiiBluetooth(u8),
    UsbHid(u8),
    /// One port on a Nintendo Wii U GameCube USB adapter.
    UsbGameCube {
        adapter_slot: u8,
        port: u8,
    },
    BluetoothHid(u8),
    BluetoothHidPair(u8, u8),
}

/// Current normalized state for one controller.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ControllerState {
    pub source: ControllerSource,
    pub kind: ControllerKind,
    pub connected: bool,
    pub buttons: Buttons,
    pub buttons_down: Buttons,
    pub buttons_up: Buttons,
    pub axes: Axes,
}

impl ControllerState {
    /// Empty disconnected state.
    pub const fn empty() -> Self {
        Self {
            source: ControllerSource::Unknown,
            kind: ControllerKind::Unknown,
            connected: false,
            buttons: Buttons::empty(),
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: Axes {
                left_x: 0,
                left_y: 0,
                right_x: 0,
                right_y: 0,
                left_trigger: 0,
                right_trigger: 0,
            },
        }
    }
}

impl Default for ControllerState {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFailure {
    AlreadyOwned,
    Source(i32),
    Timeout,
    MalformedReport,
    UnsupportedReport,
    HidDescriptor(crate::hid::HidError),
    HidReport(crate::hid::HidError),
    Transition(TransitionError),
    Trace(TraceError),
}

/// Failure returned by an input trace sink. The application owns the code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceError(pub u32);

/// Identity of the physical USB device that supplied a raw report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbTraceIdentity {
    pub device_id: i32,
    pub vid: u16,
    pub pid: u16,
}

/// Borrowed production input event delivered synchronously to an observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputTraceEvent<'a> {
    RawReport {
        source: ControllerSource,
        generation: Option<u32>,
        usb_identity: Option<UsbTraceIdentity>,
        timestamp_ticks: Option<u64>,
        bytes: &'a [u8],
    },
    Decoded {
        timestamp_ticks: Option<u64>,
        /// Snapshot identity for this source. Manual snapshots may omit it.
        id: Option<ControllerId>,
        state: ControllerState,
    },
    Reset {
        timestamp_ticks: Option<u64>,
        reset: ControllerReset,
    },
    Error {
        timestamp_ticks: Option<u64>,
        error: ControllerError,
    },
}

pub type InputTraceObserver = fn(&InputTraceEvent<'_>) -> Result<(), TraceError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerError {
    pub source: ControllerSource,
    pub operation: &'static str,
    pub failure: InputFailure,
    pub cleanup_code: Option<i32>,
}

/// Capabilities observed for one controller in the current scan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerCapabilities {
    pub buttons: bool,
    pub left_stick: bool,
    pub right_stick: bool,
    pub triggers: bool,
    pub pointer: bool,
    pub motion: bool,
    pub expansion: bool,
}

/// Valid IR pointer coordinates and the resolution reported by WPAD.
/// Coordinates are stored as IEEE-754 bits so this public snapshot remains
/// `Eq`; `position()` returns the original finite sample values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerMetadata {
    x_bits: u32,
    y_bits: u32,
    pub resolution: [u32; 2],
}

impl PointerMetadata {
    pub fn new(x: f32, y: f32, resolution: [u32; 2]) -> Option<Self> {
        (x.is_finite() && y.is_finite() && resolution[0] != 0 && resolution[1] != 0).then_some(
            Self {
                x_bits: x.to_bits(),
                y_bits: y.to_bits(),
                resolution,
            },
        )
    }

    pub fn position(self) -> (f32, f32) {
        (f32::from_bits(self.x_bits), f32::from_bits(self.y_bits))
    }
}

/// Capability and pointer information associated with a normalized state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControllerMetadata {
    pub capabilities: ControllerCapabilities,
    pub pointer: Option<PointerMetadata>,
}

impl ControllerMetadata {
    pub const fn empty() -> Self {
        Self {
            capabilities: ControllerCapabilities {
                buttons: false,
                left_stick: false,
                right_stick: false,
                triggers: false,
                pointer: false,
                motion: false,
                expansion: false,
            },
            pointer: None,
        }
    }

    fn for_state(state: ControllerState) -> Self {
        let (left_stick, right_stick, triggers, buttons) = match state.kind {
            ControllerKind::GameCube => (true, true, true, true),
            ControllerKind::WiiRemote => (false, false, false, true),
            ControllerKind::WiiClassic => (true, true, true, true),
            ControllerKind::WiiUPro | ControllerKind::SwitchPro => (true, true, false, true),
            ControllerKind::JoyConLeft => (true, false, false, true),
            ControllerKind::JoyConRight => (false, true, false, true),
            ControllerKind::JoyConPair => (true, true, false, true),
            ControllerKind::XboxOne => (true, true, true, true),
            ControllerKind::Unknown
            | ControllerKind::GenericBluetoothHid
            | ControllerKind::GenericUsbHid => (false, false, false, false),
        };
        Self {
            capabilities: ControllerCapabilities {
                buttons: state.connected && buttons,
                left_stick: state.connected && left_stick,
                right_stick: state.connected && right_stick,
                triggers: state.connected && triggers,
                ..ControllerCapabilities::default()
            },
            pointer: None,
        }
    }
}

/// Bounded controller list returned by one manager scan.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ControllerSnapshot {
    controllers: [ControllerState; MAX_CONTROLLERS],
    metadata: [ControllerMetadata; MAX_CONTROLLERS],
    identities: [Option<ControllerId>; MAX_CONTROLLERS],
    players: [ControllerState; MAX_CONTROLLERS],
    player_seen: [bool; MAX_CONTROLLERS],
    len: usize,
    resets: [Option<ControllerReset>; MAX_CONTROLLERS],
    errors: [Option<ControllerError>; MAX_CONTROLLERS],
}

impl ControllerSnapshot {
    /// Creates an empty snapshot.
    pub const fn empty() -> Self {
        Self {
            controllers: [ControllerState::empty(); MAX_CONTROLLERS],
            metadata: [ControllerMetadata::empty(); MAX_CONTROLLERS],
            identities: [None; MAX_CONTROLLERS],
            players: [ControllerState::empty(); MAX_CONTROLLERS],
            player_seen: [false; MAX_CONTROLLERS],
            len: 0,
            resets: [None; MAX_CONTROLLERS],
            errors: [None; MAX_CONTROLLERS],
        }
    }

    /// Appends a controller when capacity remains.
    pub fn push(&mut self, controller: ControllerState) {
        if self.len < self.controllers.len() {
            self.controllers[self.len] = controller;
            self.metadata[self.len] = ControllerMetadata::for_state(controller);
            self.len += 1;
        }
    }

    /// Metadata for a controller in this snapshot, matched by physical source.
    pub fn metadata(&self, source: ControllerSource) -> Option<ControllerMetadata> {
        self.controllers()
            .iter()
            .position(|state| state.source == source)
            .map(|index| self.metadata[index])
    }

    #[cfg(target_arch = "powerpc")]
    pub(crate) fn set_metadata(&mut self, source: ControllerSource, metadata: ControllerMetadata) {
        if let Some(index) = self
            .controllers()
            .iter()
            .position(|state| state.source == source)
        {
            self.metadata[index] = metadata;
        }
    }

    #[cfg(any(test, target_arch = "powerpc"))]
    pub(crate) fn push_identified(&mut self, controller: ControllerState, id: ControllerId) {
        if self.len < MAX_CONTROLLERS {
            self.identities[self.len] = Some(id);
            self.push(controller);
        }
    }

    /// Identity of a sampled source; manually constructed snapshots may omit it.
    pub fn controller_id(&self, source: ControllerSource) -> Option<ControllerId> {
        self.controllers()
            .iter()
            .position(|state| state.source == source)
            .and_then(|index| self.identities[index])
    }

    /// Assigns a controller to a player index when capacity remains.
    pub fn assign_player(&mut self, index: usize, controller: ControllerState) {
        if index < self.players.len() {
            self.players[index] = controller;
            self.player_seen[index] = true;
        }
    }

    /// Number of controllers in this snapshot.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true when no controllers were seen.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Release/reset events. These are separate from connected controllers.
    pub fn resets(&self) -> impl Iterator<Item = &ControllerReset> {
        self.resets.iter().flatten()
    }

    #[cfg(any(test, target_arch = "powerpc"))]
    pub(crate) fn take_resets(&mut self, input: &mut InputTransitions<MAX_CONTROLLERS>) {
        for (index, destination) in self.resets.iter_mut().enumerate() {
            *destination = input.take_reset(index);
        }
    }

    pub fn errors(&self) -> impl Iterator<Item = &ControllerError> {
        self.errors.iter().flatten()
    }

    #[cfg(any(test, target_arch = "powerpc"))]
    pub(crate) fn take_errors(&mut self, errors: &mut [Option<ControllerError>; MAX_CONTROLLERS]) {
        for (destination, source) in self.errors.iter_mut().zip(errors.iter_mut()) {
            *destination = source.take();
        }
    }

    pub fn merged_buttons_up(&self) -> Buttons {
        let mut buttons = Buttons::empty();
        for state in self.controllers() {
            buttons |= state.buttons_up;
        }
        for reset in self.resets() {
            buttons |= reset.state.buttons_up;
        }
        buttons
    }

    /// Controllers captured during this scan.
    pub fn controllers(&self) -> &[ControllerState] {
        &self.controllers[..self.len]
    }

    /// Returns the controller assigned to a player index.
    pub fn player(&self, index: usize) -> Option<&ControllerState> {
        self.player_seen
            .get(index)
            .copied()
            .unwrap_or(false)
            .then(|| &self.players[index])
    }

    /// Held buttons for one assigned player.
    pub fn player_buttons(&self, index: usize) -> Buttons {
        self.player(index)
            .map(|controller| controller.buttons)
            .unwrap_or_else(Buttons::empty)
    }

    /// Newly pressed buttons for one assigned player.
    pub fn player_buttons_down(&self, index: usize) -> Buttons {
        self.player(index)
            .map(|controller| controller.buttons_down)
            .unwrap_or_else(Buttons::empty)
    }

    /// ORs all held buttons across all controllers.
    pub fn merged_buttons(&self) -> Buttons {
        let mut buttons = Buttons::empty();
        for controller in self.controllers() {
            buttons |= controller.buttons;
        }
        buttons
    }

    /// ORs all newly pressed buttons across all controllers.
    pub fn merged_buttons_down(&self) -> Buttons {
        let mut buttons = Buttons::empty();
        for controller in self.controllers() {
            buttons |= controller.buttons_down;
        }
        for reset in self.resets() {
            buttons |= reset.state.buttons_down;
        }
        buttons
    }
}

impl Default for ControllerSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResetReason;
    #[test]
    fn resets_keep_release_and_error_evidence_without_a_connected_controller() {
        let mut transitions = InputTransitions::<MAX_CONTROLLERS>::new();
        let state = ControllerState {
            source: ControllerSource::UsbHid(0),
            connected: true,
            buttons: Buttons::A,
            ..ControllerState::empty()
        };
        let id = transitions.sample(0, state).unwrap();
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push_identified(state, id);
        assert_eq!(snapshot.controller_id(state.source), Some(id));
        transitions.reset(0, ResetReason::Disconnected).unwrap();
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.take_resets(&mut transitions);
        let expected = ControllerError {
            source: state.source,
            operation: "USB close",
            failure: InputFailure::Source(-6),
            cleanup_code: Some(-7),
        };
        let mut errors = [None; MAX_CONTROLLERS];
        errors[0] = Some(expected);
        snapshot.take_errors(&mut errors);
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.merged_buttons_up(), Buttons::A);
        assert_eq!(snapshot.resets().count(), 1);
        assert_eq!(snapshot.errors().copied().next(), Some(expected));
        assert_eq!(errors[0], None);
    }

    #[test]
    fn merged_edges_include_press_accumulated_before_disconnect() {
        let mut transitions = InputTransitions::<MAX_CONTROLLERS>::new();
        let source = ControllerSource::UsbHid(0);
        let state = |buttons| ControllerState {
            source,
            connected: true,
            buttons,
            ..ControllerState::empty()
        };
        transitions.sample(0, state(Buttons::empty())).unwrap();
        transitions.sample(0, state(Buttons::A)).unwrap();
        transitions.sample(0, state(Buttons::empty())).unwrap();
        transitions.reset(0, ResetReason::Disconnected).unwrap();

        let mut snapshot = ControllerSnapshot::empty();
        snapshot.take_resets(&mut transitions);
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.merged_buttons_down(), Buttons::A);
        assert_eq!(snapshot.merged_buttons_up(), Buttons::A);
    }

    #[test]
    fn normalized_axes_use_symmetric_signed_endpoints() {
        let axes = Axes {
            left_x: i16::MIN,
            left_y: i16::MAX,
            right_x: -12,
            right_y: 12,
            ..Axes::default()
        };
        assert_eq!(axes.clamp_normalized().left_x, -32767);
        assert_eq!(axes.clamp_normalized().left_y, 32767);
        assert_eq!(axes.clamp_normalized().right_x, -12);
        assert_eq!(axes.clamp_normalized().right_y, 12);
    }

    #[test]
    fn capabilities_are_conservative_for_unknown_layouts_and_complete_for_pad() {
        let state = |source, kind| ControllerState {
            source,
            kind,
            connected: true,
            ..ControllerState::empty()
        };
        let mut snapshot = ControllerSnapshot::empty();
        snapshot.push(state(
            ControllerSource::WiiBluetooth(0),
            ControllerKind::WiiRemote,
        ));
        snapshot.push(state(
            ControllerSource::GameCube(0),
            ControllerKind::GameCube,
        ));
        snapshot.push(state(
            ControllerSource::UsbHid(0),
            ControllerKind::GenericUsbHid,
        ));
        assert_eq!(
            snapshot
                .metadata(ControllerSource::WiiBluetooth(0))
                .unwrap()
                .capabilities,
            ControllerCapabilities {
                buttons: true,
                ..ControllerCapabilities::default()
            }
        );
        assert_eq!(
            snapshot
                .metadata(ControllerSource::GameCube(0))
                .unwrap()
                .capabilities,
            ControllerCapabilities {
                buttons: true,
                left_stick: true,
                right_stick: true,
                triggers: true,
                ..ControllerCapabilities::default()
            }
        );
        assert_eq!(
            snapshot
                .metadata(ControllerSource::UsbHid(0))
                .unwrap()
                .capabilities,
            ControllerCapabilities::default()
        );
    }

    #[test]
    fn pointer_metadata_rejects_invalid_geometry_and_preserves_sample_bits() {
        assert!(PointerMetadata::new(1.0, 2.0, [0, 480]).is_none());
        assert!(PointerMetadata::new(f32::NAN, 2.0, [640, 480]).is_none());
        let pointer = PointerMetadata::new(123.5, 44.25, [640, 480]).unwrap();
        assert_eq!(pointer.position(), (123.5, 44.25));
        assert_eq!(pointer.resolution, [640, 480]);
    }
}
