//! Event-driven Bluetooth HID host. No operation waits for a device reply.
use crate::*;
use alloc::{collections::VecDeque, vec::Vec};
use wiiuse_rs::{AdapterPatch, AdapterPatchAction, AdapterPatchError};

pub const HID_SLOTS: usize = 6;
// Keep enough keys for libogc's ten registered remotes plus four SYSCONF guests.
// These limits match CONF_PAD_MAX_REGISTERED / CONF_PAD_MAX_ACTIVE in ogc-conf-sys.
const REGISTERED_LINK_KEY_LIMIT: usize = 10;
const GUEST_LINK_KEY_LIMIT: usize = 4;
const LINK_KEY_CAPACITY: usize = REGISTERED_LINK_KEY_LIMIT + GUEST_LINK_KEY_LIMIT;
const QUEUE_LIMIT: usize = 64;
const COMMAND_TIMEOUT_MS: u64 = 5_000;
const CONNECT_TIMEOUT_MS: u64 = 20_000;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum LinkKeyOrigin {
    Registered,
    Guest,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct StoredLinkKey {
    address: BluetoothAddress,
    key: [u8; 16],
    origin: LinkKeyOrigin,
}

fn switch_stick_requirements(family: ControllerFamily) -> (bool, bool) {
    (
        matches!(
            family,
            ControllerFamily::SwitchPro | ControllerFamily::JoyConLeft
        ),
        matches!(
            family,
            ControllerFamily::SwitchPro | ControllerFamily::JoyConRight
        ),
    )
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ControllerFamily {
    WiiRemote,
    SwitchPro,
    JoyConLeft,
    JoyConRight,
    Xbox,
    Generic,
}
impl ControllerFamily {
    pub fn from_name(name: &[u8]) -> Self {
        // Bluetooth names are discovery hints, not device authentication.
        // Use complete known names so third-party product names cannot select
        // Nintendo's vendor-specific initialization sequence by substring.
        let name = name.split(|byte| *byte == 0).next().unwrap_or(name);
        if name.starts_with(b"Nintendo RVL-CNT") {
            Self::WiiRemote
        } else if name == b"Joy-Con (L)" || name == b"Nintendo Joy-Con (L)" {
            Self::JoyConLeft
        } else if name == b"Joy-Con (R)" || name == b"Nintendo Joy-Con (R)" {
            Self::JoyConRight
        } else if name == b"Pro Controller" || name == b"Nintendo Pro Controller" {
            Self::SwitchPro
        } else if name.eq_ignore_ascii_case(b"Xbox Wireless Controller") {
            Self::Xbox
        } else {
            Self::Generic
        }
    }
    pub fn is_switch(self) -> bool {
        matches!(self, Self::SwitchPro | Self::JoyConLeft | Self::JoyConRight)
    }
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HostStage {
    Disabled,
    AdapterOpen,
    Reset,
    BufferSize,
    Configure,
    Address,
    Features,
    Pairing,
    StoredKeys,
    AdapterPatch,
    ScanEnable,
    EventFilter,
    Ready,
    Failed,
    Closing,
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HidChannel {
    Control,
    Interrupt,
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    Transport(i32),
    Timeout,
    Protocol,
    QueueFull,
    Authentication,
    Channel,
    Unsupported,
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct HostDiagnostics {
    pub stage: HostStage,
    pub failed_stage: Option<HostStage>,
    pub failure: Option<Failure>,
    pub events: u32,
    pub acl_packets: u32,
    pub reports: u32,
    pub malformed: u32,
    pub connections: u32,
    pub disconnects: u32,
    pub connection_attempts: u32,
    pub connection_failures: u32,
    pub last_connection_status: Option<u8>,
    pub hci_command_failures: u32,
    pub authentication_failures: u32,
    pub last_authentication_status: Option<u8>,
    pub link_key_requests: u32,
    pub link_key_matches: u32,
    pub link_key_misses: u32,
    pub pin_code_requests: u32,
    pub searching: bool,
    pub failed_opcode: Option<HciOpcode>,
    pub hci_status: Option<u8>,
    pub simple_pairing: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outgoing {
    Hci { opcode: HciOpcode, params: Vec<u8> },
    Acl(Vec<u8>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HidEvent {
    Connected {
        slot: usize,
        address: BluetoothAddress,
        handle: u16,
        family: ControllerFamily,
        calibration: Option<switch::SwitchCalibration>,
        guest_pairing: bool,
        guest_key: Option<[u8; 16]>,
    },
    Report {
        slot: usize,
        data: Vec<u8>,
    },
    Disconnected {
        slot: usize,
        reason: u8,
    },
    ChannelDisconnected {
        slot: usize,
        channel: HidChannel,
    },
    Failed {
        slot: Option<usize>,
        failure: Failure,
    },
}
#[derive(Clone, Default)]
struct PendingConfig {
    options: Vec<u8>,
    has_unknown: bool,
}

#[derive(Clone)]
struct Link {
    address: BluetoothAddress,
    handle: Option<u16>,
    family: ControllerFamily,
    channels: HidL2capState,
    deadline: u64,
    ready: bool,
    naming: bool,
    incoming: bool,
    guest_pairing: bool,
    authenticated: bool,
    switch_setup: Option<switch::SwitchSetupEngine>,
    switch_setup_deadline: u64,
    assembly: Vec<u8>,
    outstanding_acl: u16,
    control_config: PendingConfig,
    interrupt_config: PendingConfig,
}
impl Link {
    fn new(address: BluetoothAddress, now: u64) -> Self {
        Self {
            address,
            handle: None,
            family: ControllerFamily::Generic,
            channels: HidL2capState::new(),
            deadline: now + CONNECT_TIMEOUT_MS,
            ready: false,
            naming: true,
            incoming: false,
            guest_pairing: false,
            authenticated: false,
            switch_setup: None,
            switch_setup_deadline: 0,
            assembly: Vec::new(),
            outstanding_acl: 0,
            control_config: PendingConfig::default(),
            interrupt_config: PendingConfig::default(),
        }
    }

    fn pending_config_mut(&mut self, local_cid: u16) -> Option<(&mut PendingConfig, u16)> {
        if self.channels.control.local_cid == local_cid {
            Some((&mut self.control_config, self.channels.control.remote_cid))
        } else if self.channels.interrupt.local_cid == local_cid {
            Some((
                &mut self.interrupt_config,
                self.channels.interrupt.remote_cid,
            ))
        } else {
            None
        }
    }
}
struct PendingCommand {
    opcode: HciOpcode,
    deadline: u64,
    authentication_handle: Option<u16>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    General,
    WiiGuest,
}

/// One host owns HCI event routing, pairing, channel setup, and HID reports.
pub struct HidHost {
    diagnostic: HostDiagnostics,
    links: [Option<Link>; HID_SLOTS],
    hci: VecDeque<Outgoing>,
    acl: VecDeque<Vec<u8>>,
    events: VecDeque<HidEvent>,
    command: Option<PendingCommand>,
    local_address: BluetoothAddress,
    keys: [Option<StoredLinkKey>; LINK_KEY_CAPACITY],
    acl_credits: u16,
    acl_capacity: u16,
    acl_size: usize,
    signal_id: u8,
    search_deadline: u64,
    search_mode: SearchMode,
    setup_step: u8,
    stored_keys_expected: Option<u16>,
    stored_keys_returned: u16,
    stored_keys_deadline: u64,
    adapter_patch: AdapterPatch,
    patch_applied: bool,
    now_ms: u64,
}
impl Default for HidHost {
    fn default() -> Self {
        Self::new()
    }
}
impl HidHost {
    pub fn new() -> Self {
        Self {
            diagnostic: HostDiagnostics {
                stage: HostStage::Disabled,
                failed_stage: None,
                failure: None,
                events: 0,
                acl_packets: 0,
                reports: 0,
                malformed: 0,
                connections: 0,
                disconnects: 0,
                connection_attempts: 0,
                connection_failures: 0,
                last_connection_status: None,
                hci_command_failures: 0,
                authentication_failures: 0,
                last_authentication_status: None,
                link_key_requests: 0,
                link_key_matches: 0,
                link_key_misses: 0,
                pin_code_requests: 0,
                searching: false,
                failed_opcode: None,
                hci_status: None,
                simple_pairing: false,
            },
            links: core::array::from_fn(|_| None),
            hci: VecDeque::new(),
            acl: VecDeque::new(),
            events: VecDeque::new(),
            command: None,
            local_address: BluetoothAddress::new([0; 6]),
            keys: [None; LINK_KEY_CAPACITY],
            acl_credits: 0,
            acl_capacity: 0,
            acl_size: 0,
            signal_id: 0,
            search_deadline: 0,
            search_mode: SearchMode::General,
            setup_step: 0,
            stored_keys_expected: None,
            stored_keys_returned: 0,
            stored_keys_deadline: 0,
            adapter_patch: AdapterPatch::new(),
            patch_applied: false,
            now_ms: 0,
        }
    }
    pub fn diagnostics(&self) -> HostDiagnostics {
        self.diagnostic
    }
    /// Return the validated stick calibration for a ready Switch-family link.
    pub fn switch_calibration(&self, slot: usize) -> Option<switch::SwitchCalibration> {
        self.links
            .get(slot)
            .and_then(Option::as_ref)
            .filter(|link| link.ready && link.family.is_switch())
            .and_then(|link| link.switch_setup)
            .and_then(|setup| setup.calibration())
    }
    pub fn next_event(&mut self) -> Option<HidEvent> {
        self.events.pop_front()
    }
    pub fn peek_event(&self) -> Option<HidEvent> {
        self.events.front().cloned()
    }
    /// Whether the transport should accept one more adapter input before backpressure.
    /// One input can publish several events, so the queue can exceed its limit by that batch.
    /// Keep completed reads owned by the transport and drain events before accepting more input.
    pub fn has_event_capacity(&self) -> bool {
        self.events.len() < QUEUE_LIMIT
    }
    /// Reset HCI state while retaining the active key cache.
    pub fn reset_preserving_keys(&mut self) {
        let keys = self.keys;
        *self = Self::new();
        self.keys = keys;
    }
    /// Add a link key loaded from the platform's persistent pairing store.
    pub fn load_link_key(&mut self, address: BluetoothAddress, key: [u8; 16]) {
        self.save_key(address, key, LinkKeyOrigin::Registered);
    }
    /// Load a guest key from the platform's guest pairing store.
    pub fn load_guest_link_key(&mut self, address: BluetoothAddress, key: [u8; 16]) {
        self.save_key(address, key, LinkKeyOrigin::Guest);
    }
    /// Drop volatile guest keys while retaining normal registered keys.
    pub fn clear_guest_link_keys(&mut self) {
        for key in &mut self.keys {
            if key.is_some_and(|key| key.origin == LinkKeyOrigin::Guest) {
                *key = None;
            }
        }
    }
    fn emit(&mut self, event: HidEvent) {
        self.events.push_back(event);
    }
    pub fn adapter_opening(&mut self) {
        self.diagnostic.stage = HostStage::AdapterOpen;
    }
    pub fn start(&mut self) {
        if !matches!(
            self.diagnostic.stage,
            HostStage::Disabled | HostStage::AdapterOpen
        ) {
            return;
        }
        self.diagnostic.stage = HostStage::Reset;
        self.queue_hci(HciOpcode::RESET, &[]);
    }
    pub fn fail_timeout(&mut self) {
        self.fail(Failure::Timeout);
    }
    pub fn fail_transport(&mut self, result: i32) {
        self.fail(Failure::Transport(result));
    }
    fn fail(&mut self, failure: Failure) {
        if self.diagnostic.stage == HostStage::Failed {
            return;
        }
        self.diagnostic.failed_stage = Some(self.diagnostic.stage);
        self.diagnostic.failure = Some(failure);
        self.diagnostic.stage = HostStage::Failed;
        self.diagnostic.searching = false;
        self.hci.clear();
        self.acl.clear();
        self.command = None;
        for slot in 0..HID_SLOTS {
            self.remove(slot, 0x16);
        }
        self.emit(HidEvent::Failed {
            slot: None,
            failure,
        });
    }
    fn queue_hci(&mut self, opcode: HciOpcode, params: &[u8]) {
        if self.hci.len() >= QUEUE_LIMIT {
            self.fail(Failure::QueueFull);
            return;
        }
        self.hci.push_back(Outgoing::Hci {
            opcode,
            params: params.to_vec(),
        });
    }
    fn configure_next(&mut self) {
        self.diagnostic.stage = HostStage::Configure;
        let (opcode, params) = if self.patch_applied {
            match self.setup_step {
                // BTE_InitSub after the patch reset: inquiry mode, scan types,
                // Wii class, page timeout, local name, then page scanning.
                0 => (HciOpcode::new(3, 0x45), alloc::vec![1]),
                1 => (HciOpcode::new(3, 0x47), alloc::vec![1]),
                2 => (HciOpcode::new(3, 0x43), alloc::vec![1]),
                3 => (HciOpcode::new(3, 0x24), alloc::vec![0, 4, 0x48]),
                4 => (HciOpcode::new(3, 0x18), alloc::vec![0, 0x80]),
                5 => {
                    let mut name = alloc::vec![0; 248];
                    name[..4].copy_from_slice(b"Wii\0");
                    (HciOpcode::new(3, 0x13), name)
                }
                _ => {
                    self.diagnostic.stage = HostStage::ScanEnable;
                    self.queue_hci(HciOpcode::WRITE_SCAN_ENABLE, &[2]);
                    return;
                }
            }
        } else {
            match self.setup_step {
                0 => (HciOpcode::new(3, 0x24), alloc::vec![0, 0x1f, 0]),
                1 => (HciOpcode::new(3, 0x13), alloc::vec![0; 248]),
                2 => (HciOpcode::new(3, 0x0a), alloc::vec![0]),
                // Receive buffers hold up to 1020 ACL payload bytes plus the HCI header.
                3 => (
                    HciOpcode::new(3, 0x33),
                    alloc::vec![0xfc, 3, 0xff, 8, 0, 1, 0],
                ),
                4 => (HciOpcode::new(4, 1), Vec::new()),
                // BTE_InitCore's second phase runs before stored-key loading
                // and the adapter patch.
                5 => (HciOpcode::new(3, 0x45), alloc::vec![1]),
                6 => (HciOpcode::new(3, 0x47), alloc::vec![1]),
                7 => (HciOpcode::new(3, 0x43), alloc::vec![1]),
                8 => (HciOpcode::new(3, 0x24), alloc::vec![0x04, 0x02, 0x40]),
                9 => (HciOpcode::new(3, 0x18), alloc::vec![0, 0x20]),
                _ => {
                    self.read_keys();
                    return;
                }
            }
        };
        self.queue_hci(opcode, &params);
    }
    fn read_keys(&mut self) {
        self.diagnostic.stage = HostStage::StoredKeys;
        self.stored_keys_expected = None;
        self.stored_keys_returned = 0;
        let mut params = self.local_address.bytes.to_vec();
        params.push(1);
        self.queue_hci(HciOpcode::new(3, 0x0d), &params);
    }
    fn continue_after_pairing(&mut self) {
        if self.patch_applied {
            self.continue_post_patch_configuration();
        } else {
            self.setup_step = 5;
            self.configure_next();
        }
    }
    fn continue_post_patch_configuration(&mut self) {
        self.setup_step = 0;
        self.configure_next();
    }
    fn start_adapter_patch(&mut self) {
        self.diagnostic.stage = HostStage::AdapterPatch;
        match self.adapter_patch.start() {
            Ok(action) => self.queue_adapter_patch_action(action),
            Err(_) => self.fail(Failure::Protocol),
        }
    }
    fn advance_after_stored_keys(&mut self) {
        if self.stored_keys_expected == Some(self.stored_keys_returned) {
            self.stored_keys_expected = None;
            self.start_adapter_patch();
        }
    }
    fn complete_adapter_patch_command(&mut self, opcode: HciOpcode, status: u8) {
        match self.adapter_patch.command_complete(opcode.raw(), status) {
            Ok(action) => self.queue_adapter_patch_action(action),
            Err(AdapterPatchError::CommandFailed { .. }) => self.fail(Failure::Protocol),
            Err(AdapterPatchError::UnexpectedOpcode { .. } | AdapterPatchError::InvalidState) => {
                self.fail(Failure::Protocol)
            }
        }
    }
    fn queue_adapter_patch_action(&mut self, action: AdapterPatchAction) {
        match action {
            AdapterPatchAction::Command { ogf, ocf, params } => {
                self.diagnostic.stage = HostStage::AdapterPatch;
                self.queue_hci(HciOpcode::new(ogf, ocf), params);
            }
            AdapterPatchAction::Reconfigure => {
                self.patch_applied = true;
                self.diagnostic.stage = HostStage::BufferSize;
                self.queue_hci(HciOpcode::new(4, 5), &[]);
            }
        }
    }
    fn save_key(&mut self, address: BluetoothAddress, key: [u8; 16], origin: LinkKeyOrigin) {
        let index = self
            .keys
            .iter()
            .position(|k| k.is_some_and(|stored| stored.address == address))
            .or_else(|| self.keys.iter().position(Option::is_none));
        if let Some(index) = index {
            self.keys[index] = Some(StoredLinkKey {
                address,
                key,
                origin,
            });
        }
    }
    /// Services timeouts only. Pending IOS read buffers belong to the transport.
    pub fn tick(&mut self, now: u64) {
        self.now_ms = now;
        if let Some(command) = self.command.as_ref().filter(|c| now >= c.deadline) {
            self.diagnostic.failed_opcode = Some(command.opcode);
            self.fail(Failure::Timeout);
        }
        if self.stored_keys_expected.is_some() && now >= self.stored_keys_deadline {
            self.fail(Failure::Timeout);
        }
        if self.diagnostic.searching && now >= self.search_deadline {
            self.stop_search();
        }
        for slot in 0..HID_SLOTS {
            let switch_step_timed_out = self.links[slot].as_ref().is_some_and(|link| {
                link.switch_setup.is_some_and(|setup| setup.is_pending())
                    && now >= link.switch_setup_deadline
            });
            if switch_step_timed_out {
                let retry = self.links[slot]
                    .as_mut()
                    .and_then(|link| link.switch_setup.as_mut())
                    .map(switch::SwitchSetupEngine::retry);
                match retry {
                    Some(Ok(packet)) if self.send_switch_setup_packet(slot, packet) => {}
                    Some(Err(switch::SwitchSetupError::RetryLimit)) => {
                        self.fail_switch_setup(slot, Failure::Timeout)
                    }
                    _ => self.fail_switch_setup(slot, Failure::Protocol),
                }
            }
            if self.links[slot]
                .as_ref()
                .is_some_and(|l| !l.ready && now >= l.deadline)
            {
                self.disconnect(slot);
                self.emit(HidEvent::Failed {
                    slot: Some(slot),
                    failure: Failure::Timeout,
                });
            }
        }
    }
    pub fn next_outgoing(&mut self, now: u64) -> Option<Outgoing> {
        self.now_ms = now;
        if matches!(
            self.diagnostic.stage,
            HostStage::Disabled | HostStage::Failed | HostStage::Closing
        ) {
            return None;
        }
        if self.command.is_none()
            && let Some(out) = self.hci.pop_front()
        {
            if let Outgoing::Hci { opcode, ref params } = out {
                self.command = Some(PendingCommand {
                    opcode,
                    deadline: now + COMMAND_TIMEOUT_MS,
                    authentication_handle: if opcode == HciOpcode::AUTH_REQUESTED
                        && params.len() == 2
                    {
                        Some(u16::from_le_bytes([params[0], params[1]]) & 0xfff)
                    } else {
                        None
                    },
                });
            }
            return Some(out);
        }
        if self.acl_credits > 0
            && let Some(packet) = self.acl.pop_front()
        {
            let handle = u16::from_le_bytes([packet[0], packet[1]]) & 0xfff;
            if let Some(slot) = self.slot_handle(handle) {
                self.links[slot].as_mut().unwrap().outstanding_acl += 1;
                self.acl_credits -= 1;
                return Some(Outgoing::Acl(packet));
            }
        }
        None
    }
    pub fn search(&mut self, now: u64) -> bool {
        self.begin_search(now, SearchMode::General)
    }
    pub fn search_wii_guest(&mut self, now: u64) -> bool {
        self.begin_search(now, SearchMode::WiiGuest)
    }
    /// Return a bit for each active Wii Remote link that guest search will close.
    pub fn wii_remote_link_mask(&self) -> u8 {
        self.links
            .iter()
            .enumerate()
            .filter(|(_, link)| {
                link.as_ref().is_some_and(|link| {
                    link.family == ControllerFamily::WiiRemote && link.handle.is_some()
                })
            })
            .fold(0, |mask, (slot, _)| mask | (1 << slot))
    }
    fn begin_search(&mut self, now: u64, mode: SearchMode) -> bool {
        if self.diagnostic.stage != HostStage::Ready || self.diagnostic.searching {
            return false;
        }
        if mode == SearchMode::WiiGuest {
            let remote_mask = self.wii_remote_link_mask();
            let slots = (0..HID_SLOTS)
                .filter(|&slot| remote_mask & (1 << slot) != 0)
                .collect::<Vec<_>>();
            if self.hci.len().saturating_add(slots.len()).saturating_add(1) > QUEUE_LIMIT {
                return false;
            }
            for slot in slots {
                if self.request_disconnect(slot).is_err() {
                    return false;
                }
            }
            self.clear_guest_link_keys();
        }
        self.diagnostic.searching = true;
        self.search_deadline = now + 15_000;
        self.search_mode = mode;
        // WPAD_Search uses BTE_PeriodicInquiry with LIAC. Guest Wii Remotes
        // use their own address as the PIN, including before name lookup ends.
        match mode {
            SearchMode::General => self.queue_hci(HciOpcode::INQUIRY, &[0x33, 0x8b, 0x9e, 8, 8]),
            SearchMode::WiiGuest => self.queue_hci(
                HciOpcode::PERIODIC_INQUIRY,
                &[5, 0, 4, 0, 0, 0x8b, 0x9e, 3, 8],
            ),
        }
        self.diagnostic.stage != HostStage::Failed
    }
    pub fn stop_search(&mut self) {
        if self.diagnostic.searching {
            let opcode = match self.search_mode {
                SearchMode::General => HciOpcode::new(1, 2),
                SearchMode::WiiGuest => HciOpcode::EXIT_PERIODIC_INQUIRY,
            };
            self.queue_hci(opcode, &[]);
            self.diagnostic.searching = false;
        }
    }
    /// Return whether the adapter still owns a connection in this slot.
    pub fn has_link(&self, slot: usize) -> bool {
        self.links.get(slot).is_some_and(Option::is_some)
    }
    /// Submit a disconnect command. The link remains present until the adapter
    /// reports disconnection. `Ok` means submission, not completed cleanup.
    pub fn request_disconnect(&mut self, slot: usize) -> core::result::Result<(), Failure> {
        if self.diagnostic.stage == HostStage::Failed {
            return Err(self.diagnostic.failure.unwrap_or(Failure::Protocol));
        }
        let handle = self
            .links
            .get(slot)
            .and_then(Option::as_ref)
            .and_then(|link| link.handle)
            .ok_or(Failure::Protocol)?;
        if self.hci.len() >= QUEUE_LIMIT {
            return Err(Failure::QueueFull);
        }
        let mut params = handle.to_le_bytes().to_vec();
        params.push(0x16);
        self.queue_hci(HciOpcode::DISCONNECT, &params);
        Ok(())
    }
    pub fn disconnect(&mut self, slot: usize) {
        if slot >= HID_SLOTS {
            return;
        }
        if self
            .links
            .get(slot)
            .and_then(|l| l.as_ref())
            .and_then(|l| l.handle)
            .is_some()
            && let Err(failure) = self.request_disconnect(slot)
        {
            self.fail(failure);
            return;
        }
        self.remove(slot, 0x16);
    }
    fn remove(&mut self, slot: usize, reason: u8) {
        if let Some(link) = self.links[slot].take() {
            self.acl_credits = self
                .acl_credits
                .saturating_add(link.outstanding_acl)
                .min(self.acl_capacity);
            if let Some(handle) = link.handle {
                self.acl
                    .retain(|p| u16::from_le_bytes([p[0], p[1]]) & 0xfff != handle);
            }
            self.diagnostic.disconnects += 1;
            self.emit(HidEvent::Disconnected { slot, reason });
        }
    }
    fn slot_address(&self, address: BluetoothAddress) -> Option<usize> {
        self.links
            .iter()
            .position(|l| l.as_ref().is_some_and(|l| l.address == address))
    }
    fn slot_handle(&self, handle: u16) -> Option<usize> {
        self.links
            .iter()
            .position(|l| l.as_ref().is_some_and(|l| l.handle == Some(handle)))
    }
    fn allocate(&mut self, address: BluetoothAddress, now: u64) -> Option<usize> {
        if let Some(slot) = self.slot_address(address) {
            return Some(slot);
        }
        let slot = self.links.iter().position(Option::is_none)?;
        self.links[slot] = Some(Link::new(address, now));
        Some(slot)
    }
    fn request_name(&mut self, address: BluetoothAddress, repetition: u8, clock: u16) {
        let mut params = address.bytes.to_vec();
        params.extend_from_slice(&[repetition, 0]);
        params.extend_from_slice(&(clock | 0x8000).to_le_bytes());
        self.queue_hci(HciOpcode::REMOTE_NAME_REQUEST, &params);
    }
    fn authentication_failed(&mut self, handle: u16, status: u8) {
        self.diagnostic.authentication_failures += 1;
        self.diagnostic.last_authentication_status = Some(status);
        if let Some(slot) = self.slot_handle(handle) {
            self.disconnect(slot);
            self.emit(HidEvent::Failed {
                slot: Some(slot),
                failure: Failure::Authentication,
            });
        }
    }
    fn command_failed(
        &mut self,
        opcode: HciOpcode,
        status: u8,
        authentication_handle: Option<u16>,
    ) {
        self.diagnostic.failed_opcode = Some(opcode);
        self.diagnostic.hci_status = Some(status);
        self.diagnostic.hci_command_failures += 1;
        if self.diagnostic.stage != HostStage::Ready {
            self.fail(Failure::Protocol);
        } else if let Some(handle) = authentication_handle {
            self.authentication_failed(handle, status);
        } else {
            self.emit(HidEvent::Failed {
                slot: None,
                failure: Failure::Protocol,
            });
        }
    }
    /// Process one complete HCI event after checking `has_event_capacity`.
    pub fn receive_event(&mut self, bytes: &[u8], now: u64) {
        self.now_ms = now;
        self.diagnostic.events += 1;
        let event = match parse_event(bytes) {
            Ok(e) => e,
            Err(_) => {
                self.diagnostic.malformed += 1;
                // A truncated Command Complete can still identify a failed
                // discovery request. Report failure without tearing down live
                // controller links for this optional operation. Never infer a
                // successful status from the missing byte.
                if bytes.len() == 5 && bytes[0] == 0x0e && bytes[1] == 3 {
                    let opcode = HciOpcode::new(
                        u16::from_le_bytes([bytes[3], bytes[4]]) >> 10,
                        u16::from_le_bytes([bytes[3], bytes[4]]) & 0x03ff,
                    );
                    if self.diagnostic.stage == HostStage::Ready
                        && matches!(
                            opcode,
                            HciOpcode::INQUIRY
                                | HciOpcode::PERIODIC_INQUIRY
                                | HciOpcode::EXIT_PERIODIC_INQUIRY
                        )
                        && self
                            .command
                            .as_ref()
                            .is_some_and(|pending| pending.opcode == opcode)
                    {
                        self.command = None;
                        self.diagnostic.searching = false;
                        self.diagnostic.failed_opcode = Some(opcode);
                        self.diagnostic.hci_status = None;
                        self.diagnostic.hci_command_failures += 1;
                        self.emit(HidEvent::Failed {
                            slot: None,
                            failure: Failure::Protocol,
                        });
                    }
                }
                return;
            }
        };
        match event {
            HciEvent::CommandComplete {
                opcode,
                status,
                params,
            } => {
                if !self.command.as_ref().is_some_and(|c| c.opcode == opcode) {
                    return;
                }
                let authentication_handle =
                    self.command.take().and_then(|c| c.authentication_handle);
                if status != 0 {
                    self.command_failed(opcode, status, authentication_handle);
                    return;
                }
                if self.diagnostic.stage == HostStage::AdapterPatch {
                    self.complete_adapter_patch_command(opcode, status);
                    return;
                }
                match (self.diagnostic.stage, opcode) {
                    (HostStage::Reset, HciOpcode::RESET) => {
                        self.diagnostic.stage = HostStage::BufferSize;
                        self.queue_hci(HciOpcode::new(4, 5), &[]);
                    }
                    (HostStage::BufferSize, op) if op == HciOpcode::new(4, 5) => {
                        if params.len() < 7 {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        self.acl_size = u16::from_le_bytes([params[0], params[1]]) as usize;
                        self.acl_capacity = u16::from_le_bytes([params[3], params[4]]);
                        self.acl_credits = self.acl_capacity;
                        if self.acl_size < 8 || self.acl_capacity == 0 {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        self.setup_step = 0;
                        self.configure_next();
                    }
                    (HostStage::Address, HciOpcode::READ_BD_ADDR) => {
                        if params.len() != 6 {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        self.local_address.bytes.copy_from_slice(params);
                        self.diagnostic.stage = HostStage::Features;
                        self.queue_hci(HciOpcode::new(4, 3), &[]);
                    }
                    (HostStage::Configure, _) => {
                        self.setup_step += 1;
                        if !self.patch_applied && self.setup_step == 5 {
                            self.diagnostic.stage = HostStage::Address;
                            self.queue_hci(HciOpcode::READ_BD_ADDR, &[]);
                        } else {
                            self.configure_next();
                        }
                    }
                    (HostStage::Features, op) if op == HciOpcode::new(4, 3) => {
                        if params.len() != 8 {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        self.diagnostic.simple_pairing = params[6] & 0x08 != 0;
                        if self.diagnostic.simple_pairing {
                            self.diagnostic.stage = HostStage::Pairing;
                            self.queue_hci(HciOpcode::new(3, 0x56), &[1]);
                        } else {
                            self.continue_after_pairing();
                        }
                    }
                    (HostStage::Pairing, _) => self.continue_after_pairing(),
                    (HostStage::StoredKeys, _) => {
                        // Command Complete omits Status from `params`; the
                        // remaining return values are two little-endian u16s.
                        if params.len() != 4 {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        let max_keys = u16::from_le_bytes([params[0], params[1]]);
                        let keys_read = u16::from_le_bytes([params[2], params[3]]);
                        if keys_read > max_keys || self.stored_keys_returned > keys_read {
                            self.fail(Failure::Protocol);
                            return;
                        }
                        self.stored_keys_expected = Some(keys_read);
                        self.stored_keys_deadline = now + COMMAND_TIMEOUT_MS;
                        self.advance_after_stored_keys();
                    }
                    (HostStage::ScanEnable, HciOpcode::WRITE_SCAN_ENABLE) => {
                        self.diagnostic.stage = HostStage::EventFilter;
                        self.queue_hci(HciOpcode::SET_EVENT_FILTER, &[1, 0]);
                    }
                    (HostStage::EventFilter, HciOpcode::SET_EVENT_FILTER) => {
                        if self.patch_applied
                            && self.adapter_patch.reconfiguration_complete().is_err()
                        {
                            self.fail(Failure::Protocol);
                        } else {
                            self.diagnostic.stage = HostStage::Ready;
                        }
                    }
                    _ => {}
                }
            }
            HciEvent::CommandStatus { opcode, status } => {
                if !self.command.as_ref().is_some_and(|c| c.opcode == opcode) {
                    return;
                }
                if self.diagnostic.stage == HostStage::AdapterPatch {
                    // This patch contract requires HCI Command Complete. A
                    // Command Status event alone does not confirm the vendor
                    // operation, so stop instead of waiting without a timer.
                    self.diagnostic.failed_opcode = Some(opcode);
                    self.diagnostic.hci_status = Some(status);
                    self.diagnostic.hci_command_failures += 1;
                    self.fail(Failure::Protocol);
                    return;
                }
                let authentication_handle =
                    self.command.take().and_then(|c| c.authentication_handle);
                if status != 0 {
                    self.command_failed(opcode, status, authentication_handle);
                }
            }
            HciEvent::InquiryComplete { status } => {
                // An inquiry cycle can end while periodic inquiry stays active.
                if self.search_mode == SearchMode::General {
                    self.diagnostic.searching = false;
                }
                if status != 0 {
                    self.stop_search();
                    self.emit(HidEvent::Failed {
                        slot: None,
                        failure: Failure::Protocol,
                    });
                }
            }
            HciEvent::InquiryResults { count, results } => {
                for result in results.iter().take(count) {
                    if !result.class_of_device.likely_game_controller()
                        || self.slot_address(result.address).is_some()
                    {
                        continue;
                    }
                    let Some(slot) = self.allocate(result.address, now) else {
                        break;
                    };
                    self.links[slot].as_mut().unwrap().guest_pairing =
                        self.search_mode == SearchMode::WiiGuest;
                    self.request_name(
                        result.address,
                        result.page_scan_rep_mode,
                        result.clock_offset,
                    );
                    let mut params = result.address.bytes.to_vec();
                    params.extend_from_slice(&0xcc18u16.to_le_bytes());
                    params.extend_from_slice(&[result.page_scan_rep_mode, 0]);
                    params.extend_from_slice(&(result.clock_offset | 0x8000).to_le_bytes());
                    params.push(1);
                    self.queue_hci(HciOpcode::CREATE_CONNECTION, &params);
                    self.diagnostic.connection_attempts += 1;
                }
            }
            HciEvent::ConnectionRequest {
                address,
                link_type,
                class_of_device,
            } => {
                let guest_pairing =
                    self.diagnostic.searching && self.search_mode == SearchMode::WiiGuest;
                let existing_slot = self.slot_address(address);
                let acceptable_device = if guest_pairing {
                    // Inquiry results start an outgoing link. Accept a guest
                    // request only when no link for this address is pending.
                    existing_slot.is_none()
                } else {
                    // Registered peers can advertise a different class during
                    // reconnect. libogc identifies those Remotes by address.
                    ClassOfDevice::new(class_of_device).likely_game_controller()
                        || existing_slot.is_some()
                        || self
                            .keys
                            .iter()
                            .flatten()
                            .any(|paired| paired.address == address)
                };
                let accepted_slot = (link_type == 1 && acceptable_device)
                    .then(|| existing_slot.or_else(|| self.allocate(address, now)))
                    .flatten();
                if let Some(slot) = accepted_slot {
                    let link = self.links[slot].as_mut().unwrap();
                    link.incoming = true;
                    link.guest_pairing = guest_pairing;
                    let mut params = address.bytes.to_vec();
                    // libogc accepts as master and listens for the peer's HID
                    // channel requests instead of starting competing requests.
                    params.push(0);
                    self.queue_hci(HciOpcode::ACCEPT_CONNECTION_REQUEST, &params);
                    self.diagnostic.connection_attempts += 1;
                    self.request_name(address, 1, 0);
                } else {
                    let mut params = address.bytes.to_vec();
                    params.push(0x0d);
                    self.queue_hci(HciOpcode::REJECT_CONNECTION_REQUEST, &params);
                }
            }
            HciEvent::RemoteNameComplete {
                address,
                status,
                name,
            } => {
                if let Some(slot) = self.slot_address(address) {
                    let link = self.links[slot].as_mut().unwrap();
                    if status == 0 {
                        link.family = ControllerFamily::from_name(name);
                    }
                    link.naming = false;
                    self.check_ready(slot);
                }
            }
            HciEvent::ConnectionComplete {
                status,
                address,
                handle,
                ..
            } => {
                let Some(slot) = self.slot_address(address) else {
                    return;
                };
                if status != 0 {
                    self.diagnostic.connection_failures += 1;
                    self.diagnostic.last_connection_status = Some(status);
                    self.remove(slot, status);
                    return;
                }
                self.diagnostic.last_connection_status = Some(0);
                self.links[slot].as_mut().unwrap().handle = Some(handle & 0xfff);
                self.diagnostic.connections += 1;
                self.queue_hci(HciOpcode::AUTH_REQUESTED, &(handle & 0xfff).to_le_bytes());
                if self.links[slot].as_ref().is_some_and(|link| !link.incoming) {
                    self.connect_channel(slot, L2CAP_PSM_HID_CTRL, L2CAP_HID_CTRL_LOCAL_CID);
                }
            }
            HciEvent::DisconnectionComplete {
                status: 0,
                handle,
                reason,
            } => {
                if let Some(slot) = self.slot_handle(handle) {
                    self.remove(slot, reason);
                }
            }
            HciEvent::PinCodeRequest { address } => {
                self.diagnostic.pin_code_requests += 1;
                let Some(slot) = self.slot_address(address) else {
                    self.queue_hci(HciOpcode::PIN_CODE_REQUEST_NEG_REPLY, &address.bytes);
                    return;
                };
                // BTE pin_req selects the peer address for guest (1+2) pairing
                // and the adapter address for normal pairing. Keep the mode
                // on the link so ending inquiry does not change an active PIN.
                let pin = if self.links[slot]
                    .as_ref()
                    .is_some_and(|link| link.guest_pairing)
                {
                    &address.bytes
                } else {
                    &self.local_address.bytes
                };
                let mut params = address.bytes.to_vec();
                params.push(pin.len() as u8);
                params.extend_from_slice(pin);
                params.resize(23, 0);
                self.queue_hci(HciOpcode::PIN_CODE_REQUEST_REPLY, &params);
            }
            HciEvent::LinkKeyRequest { address } => {
                self.diagnostic.link_key_requests += 1;
                if let Some(key) = self
                    .keys
                    .iter()
                    .flatten()
                    .find(|key| key.address == address)
                {
                    self.diagnostic.link_key_matches += 1;
                    let mut params = address.bytes.to_vec();
                    params.extend_from_slice(&key.key);
                    self.queue_hci(HciOpcode::LINK_KEY_REQUEST_REPLY, &params);
                } else {
                    self.diagnostic.link_key_misses += 1;
                    self.queue_hci(HciOpcode::LINK_KEY_REQUEST_NEG_REPLY, &address.bytes);
                }
            }
            HciEvent::LinkKeyNotification { address, key, .. } => {
                if let Some(slot) = self.slot_address(address) {
                    let guest_pairing = self.links[slot]
                        .as_ref()
                        .is_some_and(|link| link.guest_pairing);
                    let origin = if guest_pairing {
                        LinkKeyOrigin::Guest
                    } else {
                        LinkKeyOrigin::Registered
                    };
                    self.save_key(address, key, origin);
                    if guest_pairing {
                        // Keep the guest key volatile until a ready connection
                        // reports it to WPAD.
                    } else {
                        let mut params = alloc::vec![1];
                        params.extend_from_slice(&address.bytes);
                        params.extend_from_slice(&key);
                        self.queue_hci(HciOpcode::new(3, 0x11), &params);
                    }
                }
            }
            HciEvent::Unknown {
                event_code: 0x31,
                payload,
            } => {
                if payload.len() != 6 {
                    self.diagnostic.malformed += 1;
                    return;
                }
                let mut bytes = [0; 6];
                bytes.copy_from_slice(payload);
                let address = BluetoothAddress::new(bytes);
                let mut params = payload.to_vec();
                if self.diagnostic.simple_pairing && self.slot_address(address).is_some() {
                    // No display or keyboard: dedicated bonding without MITM.
                    params.extend_from_slice(&[3, 0, 1]);
                    self.queue_hci(HciOpcode::new(1, 0x2b), &params);
                } else {
                    params.push(0x18);
                    self.queue_hci(HciOpcode::new(1, 0x34), &params);
                }
            }
            HciEvent::Unknown {
                event_code: 0x33,
                payload,
            } => {
                if payload.len() != 10 {
                    self.diagnostic.malformed += 1;
                    return;
                }
                let mut bytes = [0; 6];
                bytes.copy_from_slice(&payload[..6]);
                let accepted = self.diagnostic.simple_pairing
                    && self.slot_address(BluetoothAddress::new(bytes)).is_some();
                self.queue_hci(
                    HciOpcode::new(1, if accepted { 0x2c } else { 0x2d }),
                    &bytes,
                );
            }
            HciEvent::Unknown {
                event_code: 0x36,
                payload,
            } => {
                if payload.len() != 7 {
                    self.diagnostic.malformed += 1;
                    return;
                }
                if payload[0] != 0 {
                    self.diagnostic.authentication_failures += 1;
                    self.diagnostic.last_authentication_status = Some(payload[0]);
                    let mut bytes = [0; 6];
                    bytes.copy_from_slice(&payload[1..]);
                    if let Some(slot) = self.slot_address(BluetoothAddress::new(bytes)) {
                        self.disconnect(slot);
                        self.emit(HidEvent::Failed {
                            slot: Some(slot),
                            failure: Failure::Authentication,
                        });
                    }
                }
            }
            HciEvent::Unknown {
                event_code: 0x15,
                payload,
            } => {
                if payload.is_empty()
                    || payload[0] == 0
                    || payload.len() != 1 + payload[0] as usize * 22
                {
                    self.diagnostic.malformed += 1;
                    return;
                }
                if self.diagnostic.stage == HostStage::StoredKeys {
                    let Some(returned) =
                        self.stored_keys_returned.checked_add(u16::from(payload[0]))
                    else {
                        self.fail(Failure::Protocol);
                        return;
                    };
                    if self
                        .stored_keys_expected
                        .is_some_and(|expected| returned > expected)
                    {
                        self.fail(Failure::Protocol);
                        return;
                    }
                    self.stored_keys_returned = returned;
                }
                // The Wii's legacy Bluetooth firmware can return actual key
                // bytes here, as libogc's BTE path also accepts. Spec-compliant
                // controllers return zeroed key fields, which are not usable.
                for record in payload[1..].as_chunks::<22>().0 {
                    let mut key = [0; 16];
                    key.copy_from_slice(&record[6..]);
                    if key.iter().any(|byte| *byte != 0) {
                        let mut address = [0; 6];
                        address.copy_from_slice(&record[..6]);
                        self.save_key(
                            BluetoothAddress::new(address),
                            key,
                            LinkKeyOrigin::Registered,
                        );
                    }
                }
                self.advance_after_stored_keys();
            }
            HciEvent::Unknown {
                event_code: 0x13,
                payload,
            } => {
                if payload.is_empty() || payload.len() != 1 + payload[0] as usize * 4 {
                    self.diagnostic.malformed += 1;
                    return;
                }
                for record in payload[1..].as_chunks::<4>().0 {
                    let handle = u16::from_le_bytes([record[0], record[1]]) & 0xfff;
                    let count = u16::from_le_bytes([record[2], record[3]]);
                    if let Some(slot) = self.slot_handle(handle) {
                        let outstanding = &mut self.links[slot].as_mut().unwrap().outstanding_acl;
                        let completed = count.min(*outstanding);
                        *outstanding -= completed;
                        self.acl_credits = self
                            .acl_credits
                            .saturating_add(completed)
                            .min(self.acl_capacity);
                    }
                }
            }
            HciEvent::Unknown {
                event_code: 0x06,
                payload,
            } => {
                if payload.len() != 3 {
                    self.diagnostic.malformed += 1;
                    return;
                }
                let handle = u16::from_le_bytes([payload[1], payload[2]]) & 0xfff;
                if payload[0] != 0 {
                    self.authentication_failed(handle, payload[0]);
                } else if let Some(slot) = self.slot_handle(handle) {
                    self.links[slot].as_mut().unwrap().authenticated = true;
                    let mut params = handle.to_le_bytes().to_vec();
                    params.extend_from_slice(&[5, 0]);
                    self.queue_hci(HciOpcode::WRITE_LINK_POLICY, &params);
                    self.check_ready(slot);
                }
            }
            _ => {}
        }
    }
    fn next_id(&mut self) -> u8 {
        self.signal_id = self.signal_id.wrapping_add(1).max(1);
        self.signal_id
    }
    fn connect_channel(&mut self, slot: usize, psm: u16, cid: u16) {
        let Some(handle) = self.links[slot].as_ref().and_then(|l| l.handle) else {
            return;
        };
        let mut packet = [0; 64];
        let id = self.next_id();
        if let Ok(len) = build_l2cap_connection_request(&mut packet, handle, id, psm, cid) {
            if let Some(link) = self.links[slot].as_mut() {
                let channel = if cid == L2CAP_HID_CTRL_LOCAL_CID {
                    &mut link.channels.control
                } else if cid == L2CAP_HID_INTR_LOCAL_CID {
                    &mut link.channels.interrupt
                } else {
                    return;
                };
                channel.active = true;
            }
            self.queue_acl(&packet[..len]);
        }
    }
    fn queue_acl(&mut self, packet: &[u8]) {
        if let Err(failure) = self.try_queue_acl(packet) {
            self.fail(failure);
        }
    }
    fn try_queue_acl(&mut self, packet: &[u8]) -> core::result::Result<(), Failure> {
        if self.acl_size == 0 || packet.len() < 4 {
            return Err(Failure::Protocol);
        }
        let handle = u16::from_le_bytes([packet[0], packet[1]]) & 0xfff;
        let size = self.acl_size.min(1020);
        let chunks = packet[4..].chunks(size);
        if self.acl.len() + chunks.len() > QUEUE_LIMIT {
            return Err(Failure::QueueFull);
        }
        for (index, data) in chunks.enumerate() {
            let flags = if index == 0 { 0x2000 } else { 0x1000 };
            let mut part = (handle | flags).to_le_bytes().to_vec();
            part.extend_from_slice(&(data.len() as u16).to_le_bytes());
            part.extend_from_slice(data);
            self.acl.push_back(part);
        }
        Ok(())
    }
    pub fn send_report(&mut self, slot: usize, report: &[u8]) -> bool {
        let Some(link) = self.links.get(slot).and_then(|l| l.as_ref()) else {
            return false;
        };
        let Some(handle) = link.handle else {
            return false;
        };
        if !link.channels.interrupt.ready() {
            return false;
        }
        // HIDP DATA output belongs on the interrupt channel. SET_REPORT would
        // use the control channel and a different transaction header.
        let cid = link.channels.interrupt.remote_cid;
        let mut payload = Vec::with_capacity(report.len() + 1);
        payload.push(0xa2);
        payload.extend_from_slice(report);
        let mut packet = [0; 1024];
        match build_l2cap_data(&mut packet, handle, cid, &payload) {
            Ok(len) => match self.try_queue_acl(&packet[..len]) {
                Ok(()) => true,
                Err(failure) => {
                    self.fail(failure);
                    false
                }
            },
            Err(_) => false,
        }
    }
    fn check_ready(&mut self, slot: usize) {
        // bte_connectasync_step2 starts only after control configuration has
        // completed. For inbound links, bte_listenasync_step2 keeps listening.
        if self.links[slot].as_ref().is_some_and(|link| {
            !link.incoming && link.channels.control.ready() && !link.channels.interrupt.active
        }) {
            self.connect_channel(slot, L2CAP_PSM_HID_INTR, L2CAP_HID_INTR_LOCAL_CID);
        }
        let Some(link) = self.links[slot].as_ref() else {
            return;
        };
        if link.ready || !link.channels.ready() || link.naming || !link.authenticated {
            return;
        }
        if link.family.is_switch() {
            if link.switch_setup.is_some_and(|setup| setup.is_pending()) {
                return;
            }
            let (needs_left, needs_right) = switch_stick_requirements(link.family);
            if self.links[slot].as_ref().unwrap().switch_setup.is_none() {
                self.links[slot].as_mut().unwrap().switch_setup =
                    Some(switch::SwitchSetupEngine::new(needs_left, needs_right));
            }
            let packet = self.links[slot]
                .as_mut()
                .and_then(|link| link.switch_setup.as_mut())
                .and_then(|setup| setup.start().ok());
            if !packet.is_some_and(|packet| self.send_switch_setup_packet(slot, packet)) {
                self.fail_switch_setup(slot, Failure::Protocol);
            }
            return;
        }
        self.mark_ready(slot);
    }

    fn send_switch_setup_packet(
        &mut self,
        slot: usize,
        packet: switch::SwitchOutputPacket,
    ) -> bool {
        if !self.send_report(slot, packet.bytes()) {
            return false;
        }
        if let Some(link) = self.links[slot].as_mut() {
            link.switch_setup_deadline = self.now_ms.saturating_add(COMMAND_TIMEOUT_MS);
            true
        } else {
            false
        }
    }

    fn fail_switch_setup(&mut self, slot: usize, failure: Failure) {
        self.disconnect(slot);
        self.emit(HidEvent::Failed {
            slot: Some(slot),
            failure,
        });
    }

    fn mark_ready(&mut self, slot: usize) {
        let link = self.links[slot].as_mut().unwrap();
        link.ready = true;
        let address = link.address;
        let handle = link.handle.unwrap();
        let family = link.family;
        let calibration = link.switch_setup.and_then(|setup| setup.calibration());
        let guest_pairing = link.guest_pairing;
        let guest_key = if guest_pairing {
            self.keys
                .iter()
                .flatten()
                .find(|key| key.address == address && key.origin == LinkKeyOrigin::Guest)
                .map(|key| key.key)
        } else {
            None
        };
        let event = HidEvent::Connected {
            slot,
            address,
            handle,
            family,
            calibration,
            guest_pairing,
            guest_key,
        };
        self.emit(event);
    }
    /// Process one complete ACL frame after checking `has_event_capacity`.
    pub fn receive_acl(&mut self, bytes: &[u8]) {
        self.diagnostic.acl_packets += 1;
        let acl = match parse_acl_packet(bytes) {
            Ok(a) => a,
            Err(_) => {
                self.diagnostic.malformed += 1;
                return;
            }
        };
        let Some(slot) = self.slot_handle(acl.handle) else {
            return;
        };
        let link = self.links[slot].as_mut().unwrap();
        if acl.packet_boundary == L2CAP_ACL_START || acl.packet_boundary == 0 {
            link.assembly.clear();
        } else if acl.packet_boundary != L2CAP_ACL_CONT || link.assembly.is_empty() {
            self.diagnostic.malformed += 1;
            return;
        }
        if link.assembly.len() + acl.payload.len() > 4096 {
            link.assembly.clear();
            self.diagnostic.malformed += 1;
            return;
        }
        link.assembly.extend_from_slice(acl.payload);
        if link.assembly.len() < 4 {
            return;
        }
        let expected = 4 + u16::from_le_bytes([link.assembly[0], link.assembly[1]]) as usize;
        if link.assembly.len() < expected {
            return;
        }
        let packet = core::mem::take(&mut link.assembly);
        if packet.len() != expected {
            self.diagnostic.malformed += 1;
            return;
        }
        let l2cap = match parse_l2cap_packet(&packet) {
            Ok(l) => l,
            Err(_) => {
                self.diagnostic.malformed += 1;
                return;
            }
        };
        if l2cap.channel_id == L2CAP_CID_SIGNALING {
            let mut signals = l2cap.payload;
            loop {
                match parse_next_l2cap_signal(&mut signals) {
                    Ok(Some(signal)) => {
                        self.signal(slot, signal);
                        if self.links[slot].is_none() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        self.diagnostic.malformed += 1;
                        break;
                    }
                }
            }
            self.check_ready(slot);
        } else if l2cap.channel_id == L2CAP_HID_INTR_LOCAL_CID {
            let hid = match parse_hidp_packet(l2cap.payload) {
                Ok(h) => h,
                Err(_) => {
                    self.diagnostic.malformed += 1;
                    return;
                }
            };
            if hid.transaction != HIDP_TRANS_DATA || hid.parameter != HIDP_DATA_RTYPE_INPUT {
                return;
            }
            let pending_setup = self.links[slot]
                .as_ref()
                .and_then(|link| link.switch_setup)
                .is_some_and(|setup| setup.is_pending());
            if pending_setup {
                let progress = self.links[slot]
                    .as_mut()
                    .and_then(|link| link.switch_setup.as_mut())
                    .map(|setup| setup.receive(hid.payload));
                match progress {
                    Some(Ok(switch::SwitchSetupProgress::Send(packet))) => {
                        if !self.send_switch_setup_packet(slot, packet) {
                            self.fail_switch_setup(slot, Failure::Protocol);
                        }
                    }
                    Some(Ok(switch::SwitchSetupProgress::Ready(_))) => self.mark_ready(slot),
                    Some(Ok(switch::SwitchSetupProgress::Ignored)) => {}
                    Some(Err(_)) | None => self.fail_switch_setup(slot, Failure::Protocol),
                }
                return;
            }
            if self.links[slot].as_ref().is_some_and(|l| l.ready) {
                self.diagnostic.reports += 1;
                self.emit(HidEvent::Report {
                    slot,
                    data: hid.payload.to_vec(),
                });
            }
        }
    }
    fn signal(&mut self, slot: usize, signal: L2capSignal<'_>) {
        let payload = match parse_l2cap_signal_payload(signal) {
            Ok(p) => p,
            Err(_) => {
                self.diagnostic.malformed += 1;
                return;
            }
        };
        let handle = self.links[slot].as_ref().unwrap().handle.unwrap();
        let mut response = [0; MAX_EVENT_PACKET_BYTES];
        match payload {
            L2capSignalPayload::ConnectionRequest { psm, source_cid } => {
                let local = match psm {
                    L2CAP_PSM_HID_CTRL => L2CAP_HID_CTRL_LOCAL_CID,
                    L2CAP_PSM_HID_INTR => L2CAP_HID_INTR_LOCAL_CID,
                    _ => 0,
                };
                let result = if local == 0 { 2 } else { 0 };
                if let Ok(len) = build_l2cap_connection_response(
                    &mut response,
                    handle,
                    signal.identifier,
                    local,
                    source_cid,
                    result,
                ) {
                    self.queue_acl(&response[..len]);
                }
                if local == 0 {
                    return;
                }
                let channel = if local == L2CAP_HID_CTRL_LOCAL_CID {
                    &mut self.links[slot].as_mut().unwrap().channels.control
                } else {
                    &mut self.links[slot].as_mut().unwrap().channels.interrupt
                };
                channel.active = true;
                channel.connected = true;
                channel.remote_cid = source_cid;
                let id = self.next_id();
                if let Ok(len) =
                    build_l2cap_configuration_request(&mut response, handle, id, source_cid)
                {
                    self.queue_acl(&response[..len]);
                }
            }
            L2capSignalPayload::ConnectionResponse {
                dest_cid,
                source_cid,
                result,
                ..
            } => {
                if result == L2CAP_CONN_PENDING {
                    return;
                }
                if result != 0 {
                    self.disconnect(slot);
                    self.emit(HidEvent::Failed {
                        slot: Some(slot),
                        failure: Failure::Channel,
                    });
                    return;
                }
                if source_cid != L2CAP_HID_CTRL_LOCAL_CID && source_cid != L2CAP_HID_INTR_LOCAL_CID
                {
                    self.diagnostic.malformed += 1;
                    return;
                }
                let _ = self.links[slot]
                    .as_mut()
                    .unwrap()
                    .channels
                    .observe_signal(signal);
                let id = self.next_id();
                if let Ok(len) =
                    build_l2cap_configuration_request(&mut response, handle, id, dest_cid)
                {
                    self.queue_acl(&response[..len]);
                }
            }
            L2capSignalPayload::ConfigurationRequest {
                dest_cid,
                flags,
                options,
            } => {
                if flags & !1 != 0 {
                    self.disconnect(slot);
                    self.emit(HidEvent::Failed {
                        slot: Some(slot),
                        failure: Failure::Channel,
                    });
                    return;
                }
                let mut segment_options = [0; MAX_EVENT_PACKET_BYTES];
                let (segment_len, segment_unknown) =
                    match collect_config_response_options(options, &mut segment_options) {
                        Ok(result) => result,
                        Err(_) => {
                            self.disconnect(slot);
                            self.emit(HidEvent::Failed {
                                slot: Some(slot),
                                failure: Failure::Channel,
                            });
                            return;
                        }
                    };

                let (remote, result, accumulated) = {
                    let link = self.links[slot].as_mut().unwrap();
                    let Some((pending, remote)) = link.pending_config_mut(dest_cid) else {
                        return;
                    };
                    if segment_unknown && !pending.has_unknown {
                        pending.options.clear();
                        pending.has_unknown = true;
                    }
                    if segment_len != 0 {
                        pending
                            .options
                            .extend_from_slice(&segment_options[..segment_len]);
                    }
                    if flags & 1 != 0 {
                        (remote, L2CAP_CFG_SUCCESS, Vec::new())
                    } else {
                        let result = if pending.has_unknown {
                            L2CAP_CFG_UNKNOWN
                        } else {
                            L2CAP_CFG_SUCCESS
                        };
                        let options = core::mem::take(&mut pending.options);
                        pending.has_unknown = false;
                        (remote, result, options)
                    }
                };
                if let Ok(len) = build_l2cap_configuration_response_with_options(
                    &mut response,
                    handle,
                    signal.identifier,
                    remote,
                    result,
                    &accumulated,
                ) {
                    self.queue_acl(&response[..len]);
                }
                if flags & 1 == 0 && result == L2CAP_CFG_SUCCESS {
                    let _ = self.links[slot]
                        .as_mut()
                        .unwrap()
                        .channels
                        .observe_signal(signal);
                }
            }
            L2capSignalPayload::ConfigurationResponse {
                source_cid,
                result,
                flags,
                ..
            } => {
                if flags & !1 != 0 {
                    self.disconnect(slot);
                    self.emit(HidEvent::Failed {
                        slot: Some(slot),
                        failure: Failure::Channel,
                    });
                    return;
                }

                if result == L2CAP_CFG_SUCCESS {
                    let _ = self.links[slot]
                        .as_mut()
                        .unwrap()
                        .channels
                        .observe_signal(signal);
                } else if flags & 1 == 0 && result != L2CAP_CFG_UNACCEPT {
                    self.disconnect(slot);
                    self.emit(HidEvent::Failed {
                        slot: Some(slot),
                        failure: Failure::Channel,
                    });
                    return;
                }

                if flags & 1 != 0 || result == L2CAP_CFG_UNACCEPT {
                    // C continues non-success responses while the continuation
                    // bit is set, and retries an unacceptable final response.
                    let id = self.next_id();
                    if let Ok(len) =
                        build_l2cap_configuration_request(&mut response, handle, id, source_cid)
                    {
                        self.queue_acl(&response[..len]);
                    }
                }
            }
            L2capSignalPayload::DisconnectionRequest {
                dest_cid,
                source_cid: _,
            } => {
                let channel = self.links[slot]
                    .as_ref()
                    .filter(|link| link.channels.local_channel_is_active(dest_cid))
                    .and_then(|link| {
                        if link.channels.control.local_cid == dest_cid {
                            Some((HidChannel::Control, link.channels.control.remote_cid))
                        } else if link.channels.interrupt.local_cid == dest_cid {
                            Some((HidChannel::Interrupt, link.channels.interrupt.remote_cid))
                        } else {
                            None
                        }
                    });
                if let Some((channel, remote_cid)) = channel {
                    if let Ok(len) = build_l2cap_disconnection_response(
                        &mut response,
                        handle,
                        signal.identifier,
                        dest_cid,
                        remote_cid,
                    ) {
                        match self.try_queue_acl(&response[..len]) {
                            // C closes and reports the channel after it has
                            // allocated the response, even if transmission
                            // fails. A full Rust ACL queue is that send-failure
                            // case; response construction above succeeded.
                            Ok(()) | Err(Failure::QueueFull) => {
                                let link = self.links[slot].as_mut().unwrap();
                                if link.channels.close_local_channel(dest_cid) {
                                    link.ready = false;
                                }
                                self.emit(HidEvent::ChannelDisconnected { slot, channel });
                            }
                            Err(failure) => self.fail(failure),
                        }
                    }
                } else if let Ok(len) = build_l2cap_invalid_cid_reject(
                    &mut response,
                    handle,
                    signal.identifier,
                    dest_cid,
                ) {
                    self.queue_acl(&response[..len]);
                }
            }
            L2capSignalPayload::Unknown { .. }
                if !matches!(
                    signal.code,
                    L2CAP_CMD_REJECT | L2CAP_ECHO_REQ | L2CAP_ECHO_RSP
                ) =>
            {
                // The parser also preserves known but unsupported standard
                // commands as Unknown. C handles Command Reject and Echo
                // separately; Information commands reach its default case.
                if let Ok(len) = build_l2cap_command_reject(
                    &mut response,
                    handle,
                    signal.identifier,
                    L2CAP_CMD_NOT_UNDERSTOOD,
                ) {
                    self.queue_acl(&response[..len]);
                }
            }
            L2capSignalPayload::Unknown { .. } => {}
            _ => {}
        }
    }
}
fn collect_config_response_options(
    mut options: &[u8],
    unknown: &mut [u8],
) -> Result<(usize, bool)> {
    let mut unknown_len = 0usize;
    let mut has_unknown = false;
    while !options.is_empty() {
        if options.len() < 2 || options.len() < 2 + options[1] as usize {
            return Err(Error::MalformedL2cap);
        }
        let kind = options[0];
        let length = options[1] as usize;
        if kind & 0x80 == 0 {
            let option_len = length + 2;
            match kind {
                1 if length == 2 && u16::from_le_bytes([options[2], options[3]]) >= 48 => {}
                2 if length == 2 => {}
                3 if length == 22 => {
                    if options[3] == 2 {
                        let end = unknown_len
                            .checked_add(option_len)
                            .ok_or(Error::BufferTooSmall)?;
                        if end > unknown.len() {
                            return Err(Error::BufferTooSmall);
                        }
                        unknown[unknown_len..end].copy_from_slice(&options[..option_len]);
                        unknown_len = end;
                    }
                }
                1..=3 => return Err(Error::MalformedL2cap),
                _ => {
                    if !has_unknown {
                        unknown_len = 0;
                        has_unknown = true;
                    }
                    let end = unknown_len
                        .checked_add(option_len)
                        .ok_or(Error::BufferTooSmall)?;
                    if end > unknown.len() {
                        return Err(Error::BufferTooSmall);
                    }
                    unknown[unknown_len..end].copy_from_slice(&options[..option_len]);
                    unknown_len = end;
                }
            }
        }
        options = &options[2 + length..];
    }
    Ok((unknown_len, has_unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_family_requires_complete_known_names() {
        assert_eq!(
            ControllerFamily::from_name(b"Pro Controller"),
            ControllerFamily::SwitchPro
        );
        assert_eq!(
            ControllerFamily::from_name(b"Joy-Con (L)\0"),
            ControllerFamily::JoyConLeft
        );
        assert_eq!(
            ControllerFamily::from_name(b"PDP Pro Controller"),
            ControllerFamily::Generic
        );
        assert_eq!(
            ControllerFamily::from_name(b"Joy-Con (R) clone"),
            ControllerFamily::Generic
        );
        assert_eq!(
            ControllerFamily::from_name(b"Xbox Wireless Controller"),
            ControllerFamily::Xbox
        );
        assert_eq!(
            ControllerFamily::from_name(b"Xbox compatible pad"),
            ControllerFamily::Generic
        );
    }
    fn complete(host: &mut HidHost, opcode: HciOpcode, params: &[u8]) {
        let mut event = alloc::vec![0x0e, (4 + params.len()) as u8, 1];
        event.extend_from_slice(&opcode.raw().to_le_bytes());
        event.push(0);
        event.extend_from_slice(params);
        host.receive_event(&event, 0);
    }
    fn complete_with_status(host: &mut HidHost, opcode: HciOpcode, status: u8) {
        let mut event = alloc::vec![0x0e, 4, 1];
        event.extend_from_slice(&opcode.raw().to_le_bytes());
        event.push(status);
        host.receive_event(&event, 0);
    }
    fn command_status(host: &mut HidHost, opcode: HciOpcode, status: u8) {
        let mut event = alloc::vec![0x0f, 4, status, 1];
        event.extend_from_slice(&opcode.raw().to_le_bytes());
        host.receive_event(&event, 0);
    }
    fn next_hci(host: &mut HidHost) -> (HciOpcode, Vec<u8>) {
        match host.next_outgoing(0) {
            Some(Outgoing::Hci { opcode, params }) => (opcode, params),
            other => panic!("expected HCI command, received {other:?}"),
        }
    }
    fn awaiting_patch_start() -> HidHost {
        let mut host = HidHost::new();
        host.start();
        assert_eq!(next_hci(&mut host).0, HciOpcode::RESET);
        complete(&mut host, HciOpcode::RESET, &[]);
        assert_eq!(next_hci(&mut host).0, HciOpcode::new(4, 5));
        complete(&mut host, HciOpcode::new(4, 5), &[64, 0, 0, 8, 0, 0, 0]);
        for opcode in [
            HciOpcode::new(3, 0x24),
            HciOpcode::new(3, 0x13),
            HciOpcode::new(3, 0x0a),
            HciOpcode::new(3, 0x33),
            HciOpcode::new(4, 1),
        ] {
            assert_eq!(next_hci(&mut host).0, opcode);
            complete(&mut host, opcode, &[]);
        }
        assert_eq!(next_hci(&mut host).0, HciOpcode::READ_BD_ADDR);
        complete(&mut host, HciOpcode::READ_BD_ADDR, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(next_hci(&mut host).0, HciOpcode::new(4, 3));
        complete(&mut host, HciOpcode::new(4, 3), &[0; 8]);
        for (opcode, params) in [
            (HciOpcode::new(3, 0x45), &[1][..]),
            (HciOpcode::new(3, 0x47), &[1][..]),
            (HciOpcode::new(3, 0x43), &[1][..]),
            (HciOpcode::new(3, 0x24), &[0x04, 0x02, 0x40][..]),
            (HciOpcode::new(3, 0x18), &[0, 0x20][..]),
        ] {
            assert_eq!(next_hci(&mut host), (opcode, params.to_vec()));
            complete(&mut host, opcode, &[]);
        }
        assert_eq!(next_hci(&mut host).0, HciOpcode::new(3, 0x0d));
        complete(&mut host, HciOpcode::new(3, 0x0d), &[8, 0, 0, 0]);
        assert_eq!(host.diagnostics().stage, HostStage::AdapterPatch);
        host
    }

    #[test]
    fn stored_key_read_counts_return_event_without_using_its_zeroed_keys() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });

        // Status is decoded separately. These four bytes are Max_Num_Keys=8
        // and Num_Keys_Read=1, each encoded as little-endian u16.
        complete(&mut host, HciOpcode::new(3, 0x0d), &[8, 0, 1, 0]);
        assert_eq!(host.diagnostics().stage, HostStage::StoredKeys);
        assert!(host.next_outgoing(0).is_none());

        let address = [1, 2, 3, 4, 5, 6];
        let key = [0; 16];
        let mut event = alloc::vec![0x15, 23, 1];
        event.extend_from_slice(&address);
        event.extend_from_slice(&key);
        host.receive_event(&event, 1);

        assert_eq!(host.diagnostics().stage, HostStage::AdapterPatch);
        assert!(host.keys.iter().all(Option::is_none));
        assert_eq!(next_hci(&mut host).0, HciOpcode::new(0x3f, 0x4f));
    }

    #[test]
    fn stored_key_read_uses_legacy_adapter_key_bytes_when_present() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        complete(&mut host, HciOpcode::new(3, 0x0d), &[8, 0, 1, 0]);
        let address = [6, 5, 4, 3, 2, 1];
        let key = [0x5a; 16];
        let mut event = alloc::vec![0x15, 23, 1];
        event.extend_from_slice(&address);
        event.extend_from_slice(&key);
        host.receive_event(&event, 1);

        assert!(host.keys.contains(&Some(StoredLinkKey {
            address: BluetoothAddress::new(address),
            key,
            origin: LinkKeyOrigin::Registered,
        })));
    }

    #[test]
    fn retains_all_registered_and_guest_keys_for_later_wii_remote_reconnects() {
        let mut host = HidHost::new();
        for index in 0..GUEST_LINK_KEY_LIMIT {
            host.load_guest_link_key(
                BluetoothAddress::new([0xa0, 0, 0, 0, 0, index as u8]),
                [0xb0 + index as u8; 16],
            );
        }

        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        complete(
            &mut host,
            HciOpcode::new(3, 0x0d),
            &[
                REGISTERED_LINK_KEY_LIMIT as u8,
                0,
                REGISTERED_LINK_KEY_LIMIT as u8,
                0,
            ],
        );

        // The Wii can have ten registered keys in its BT store. Preserve a
        // later key even when all guest keys are already loaded.
        let remote_index = REGISTERED_LINK_KEY_LIMIT - 1;
        let remote_address = [0xc0, 1, 2, 3, 4, remote_index as u8];
        let remote_key = [0xd0 + remote_index as u8; 16];
        let mut event = alloc::vec![
            0x15,
            (1 + REGISTERED_LINK_KEY_LIMIT * 22) as u8,
            REGISTERED_LINK_KEY_LIMIT as u8
        ];
        for index in 0..REGISTERED_LINK_KEY_LIMIT {
            event.extend_from_slice(&[0xc0, 1, 2, 3, 4, index as u8]);
            event.extend_from_slice(&[0xd0 + index as u8; 16]);
        }
        host.receive_event(&event, 1);

        assert_eq!(host.keys.len(), LINK_KEY_CAPACITY);
        assert!(host.keys.iter().all(Option::is_some));
        assert!(host.keys.contains(&Some(StoredLinkKey {
            address: BluetoothAddress::new(remote_address),
            key: remote_key,
            origin: LinkKeyOrigin::Registered,
        })));

        // Completing the key dump advances startup into the adapter patch.
        let _patch_command = next_hci(&mut host);
        host.command = None;
        host.diagnostic.stage = HostStage::Ready;
        let mut key_request = alloc::vec![0x17, 6];
        key_request.extend_from_slice(&remote_address);
        host.receive_event(&key_request, 2);
        let (opcode, params) = next_hci(&mut host);
        assert_eq!(opcode, HciOpcode::LINK_KEY_REQUEST_REPLY);
        assert_eq!(&params[..6], &remote_address);
        assert_eq!(&params[6..], &remote_key);
    }

    #[test]
    fn stored_key_read_rejects_count_above_capacity() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        complete(&mut host, HciOpcode::new(3, 0x0d), &[1, 0, 2, 0]);
        assert_eq!(host.diagnostics().stage, HostStage::Failed);
        assert_eq!(host.diagnostics().failure, Some(Failure::Protocol));
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn stored_key_records_may_arrive_before_command_complete() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        let mut event = alloc::vec![0x15, 23, 1];
        event.extend_from_slice(&[6, 5, 4, 3, 2, 1]);
        event.extend_from_slice(&[0; 16]);
        host.receive_event(&event, 1);
        assert_eq!(host.diagnostics().stage, HostStage::StoredKeys);

        complete(&mut host, HciOpcode::new(3, 0x0d), &[8, 0, 1, 0]);
        assert_eq!(host.diagnostics().stage, HostStage::AdapterPatch);
        assert!(host.keys.iter().all(Option::is_none));
    }
    #[test]
    fn stored_key_event_rejects_zero_record_count() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.receive_event(&[0x15, 1, 0], 0);
        assert_eq!(host.diagnostics().malformed, 1);
        assert_eq!(host.diagnostics().stage, HostStage::StoredKeys);
    }

    #[test]
    fn stored_key_read_times_out_if_return_events_are_missing() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        complete(&mut host, HciOpcode::new(3, 0x0d), &[8, 0, 1, 0]);
        host.tick(COMMAND_TIMEOUT_MS);
        assert_eq!(host.diagnostics().stage, HostStage::Failed);
        assert_eq!(host.diagnostics().failure, Some(Failure::Timeout));
        assert!(host.next_outgoing(COMMAND_TIMEOUT_MS).is_none());
    }
    fn ready() -> HidHost {
        let mut host = awaiting_patch_start();
        let vendor_start = HciOpcode::new(0x3f, 0x4f);
        let vendor_continue = HciOpcode::new(0x3f, 0x4c);
        let (opcode, params) = next_hci(&mut host);
        assert_eq!(opcode, vendor_start);
        assert_eq!(params, [0]);
        complete(&mut host, opcode, &[]);
        let (opcode, params) = next_hci(&mut host);
        assert_eq!(opcode, vendor_continue);
        assert_eq!(params.len(), 184);
        complete(&mut host, opcode, &[]);
        let (opcode, params) = next_hci(&mut host);
        assert_eq!(opcode, vendor_start);
        assert_eq!(params.len(), 92);
        complete(&mut host, opcode, &[]);
        assert_eq!(next_hci(&mut host).0, HciOpcode::RESET);
        complete(&mut host, HciOpcode::RESET, &[]);
        assert_eq!(next_hci(&mut host).0, HciOpcode::new(4, 5));
        complete(&mut host, HciOpcode::new(4, 5), &[64, 0, 0, 8, 0, 0, 0]);
        for (opcode, expected_params) in [
            (HciOpcode::new(3, 0x45), &[1][..]),
            (HciOpcode::new(3, 0x47), &[1][..]),
            (HciOpcode::new(3, 0x43), &[1][..]),
            (HciOpcode::new(3, 0x24), &[0, 4, 0x48][..]),
            (HciOpcode::new(3, 0x18), &[0, 0x80][..]),
            (HciOpcode::new(3, 0x13), &[b'W', b'i', b'i', 0][..]),
        ] {
            let (actual, params) = next_hci(&mut host);
            assert_eq!(actual, opcode);
            if opcode == HciOpcode::new(3, 0x13) {
                assert_eq!(params.len(), 248);
                assert_eq!(&params[..4], expected_params);
                assert!(params[4..].iter().all(|byte| *byte == 0));
            } else {
                assert_eq!(params, expected_params);
            }
            complete(&mut host, opcode, &[]);
        }
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::WRITE_SCAN_ENABLE, alloc::vec![2])
        );
        complete(&mut host, HciOpcode::WRITE_SCAN_ENABLE, &[]);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::SET_EVENT_FILTER, alloc::vec![1, 0])
        );
        complete(&mut host, HciOpcode::SET_EVENT_FILTER, &[]);
        assert_eq!(host.diagnostics().stage, HostStage::Ready);
        assert!(host.adapter_patch.is_complete());
        host
    }
    fn configured(family: ControllerFamily) -> HidHost {
        let mut host = ready();
        let slot = host
            .allocate(BluetoothAddress::new([6, 5, 4, 3, 2, 1]), 0)
            .unwrap();
        let link = host.links[slot].as_mut().unwrap();
        link.handle = Some(1);
        link.family = family;
        link.naming = false;
        link.authenticated = true;
        for channel in [&mut link.channels.control, &mut link.channels.interrupt] {
            channel.active = true;
            channel.connected = true;
            channel.remote_cid = channel.local_cid + 0x10;
            channel.local_configured = true;
            channel.remote_configured = true;
        }
        host.check_ready(slot);
        host
    }

    #[test]
    fn malformed_discovery_completion_reports_failure_without_host_timeout() {
        let mut host = inbound_remote_awaiting_authentication();
        command_status(&mut host, HciOpcode::AUTH_REQUESTED, 0);
        host.receive_event(&[0x06, 3, 0, 1, 0], 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::WRITE_LINK_POLICY);
        complete(&mut host, HciOpcode::WRITE_LINK_POLICY, &[]);
        let links_before = host.links.iter().flatten().count();
        assert!(links_before > 0);
        assert!(host.search_wii_guest(0));
        assert_eq!(next_hci(&mut host).0, HciOpcode::DISCONNECT);
        command_status(&mut host, HciOpcode::DISCONNECT, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::PERIODIC_INQUIRY);
        // Independent bytes: Command Complete, length 3, one command credit,
        // opcode 0x0403, and no mandatory status byte.
        host.receive_event(&[0x0e, 3, 1, 3, 4], 1);
        assert_eq!(host.diagnostics().stage, HostStage::Ready);
        assert_eq!(host.diagnostics().malformed, 1);
        assert_eq!(host.diagnostics().hci_command_failures, 1);
        assert_eq!(
            host.diagnostics().failed_opcode,
            Some(HciOpcode::PERIODIC_INQUIRY)
        );
        assert_eq!(host.diagnostics().hci_status, None);
        assert!(!host.diagnostics().searching);
        assert!(host.command.is_none());
        assert!(host.events.iter().any(|event| matches!(
            event,
            HidEvent::Failed {
                slot: None,
                failure: Failure::Protocol
            }
        )));
        host.tick(COMMAND_TIMEOUT_MS + 10);
        assert_eq!(host.diagnostics().stage, HostStage::Ready);
        assert_eq!(host.links.iter().flatten().count(), links_before);
        assert!(host.search(20));
        assert_eq!(next_hci(&mut host).0, HciOpcode::INQUIRY);
    }

    #[test]
    fn paired_remote_reconnects_with_non_peripheral_class() {
        let mut host = ready();
        let address = BluetoothAddress::new([6, 5, 4, 3, 2, 1]);
        host.load_link_key(address, [0xa0; 16]);
        // Dolphin's emulated Remote uses these class bytes. Pairing identity,
        // not the advertised class, permits a registered peer to reconnect.
        host.receive_event(&[0x04, 10, 6, 5, 4, 3, 2, 1, 0, 4, 0x48, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::ACCEPT_CONNECTION_REQUEST,
                alloc::vec![6, 5, 4, 3, 2, 1, 0]
            )
        );
        assert_eq!(host.diagnostics().connection_attempts, 1);
        assert_eq!(host.diagnostics().connections, 0);
        command_status(&mut host, HciOpcode::ACCEPT_CONNECTION_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
    }

    #[test]
    fn unknown_non_peripheral_peer_is_still_rejected() {
        let mut host = ready();
        host.load_link_key(BluetoothAddress::new([1, 2, 3, 4, 5, 6]), [0xa0; 16]);
        host.receive_event(&[0x04, 10, 6, 5, 4, 3, 2, 1, 0, 4, 0x48, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::REJECT_CONNECTION_REQUEST,
                alloc::vec![6, 5, 4, 3, 2, 1, 0x0d]
            )
        );
        assert_eq!(host.diagnostics().connection_attempts, 0);
        assert_eq!(host.diagnostics().connections, 0);
    }

    /// Controlled peer packets exercise the production event engine. This is
    /// protocol verification, not a replacement for an IOS/hardware run.
    fn inbound_remote_awaiting_authentication() -> HidHost {
        let mut host = ready();
        host.receive_event(&[0x04, 10, 6, 5, 4, 3, 2, 1, 4, 5, 0, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::ACCEPT_CONNECTION_REQUEST,
                alloc::vec![6, 5, 4, 3, 2, 1, 0]
            )
        );
        command_status(&mut host, HciOpcode::ACCEPT_CONNECTION_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);
        let name = b"Nintendo RVL-CNT-01";
        let mut named = alloc::vec![0x07, (7 + name.len()) as u8, 0, 6, 5, 4, 3, 2, 1];
        named.extend_from_slice(name);
        host.receive_event(&named, 0);
        host.receive_event(&[0x03, 11, 0, 1, 0, 6, 5, 4, 3, 2, 1, 1, 0], 0);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::AUTH_REQUESTED, alloc::vec![1, 0])
        );
        // Accepting authentication is not completion, and inbound links do
        // not emit their own competing L2CAP Connection Request.
        assert!(host.next_outgoing(0).is_none());
        host
    }

    fn configure_peer_channel(host: &mut HidHost, psm: u16, local: u16, remote: u16) {
        let mut packet = [0; 128];
        let used = build_l2cap_connection_request(&mut packet, 1, 1, psm, remote).unwrap();
        host.receive_acl(&packet[..used]);
        for code in [3, 4] {
            let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
                panic!("missing channel reply")
            };
            let acl = parse_acl_packet(&packet).unwrap();
            let l2cap = parse_l2cap_packet(acl.payload).unwrap();
            assert_eq!(parse_l2cap_signal(l2cap.payload).unwrap().code, code);
        }
        let used = build_l2cap_configuration_request(&mut packet, 1, 2, local).unwrap();
        host.receive_acl(&packet[..used]);
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        let used = build_l2cap_configuration_response(&mut packet, 1, 3, remote, 0).unwrap();
        host.receive_acl(&packet[..used]);
    }

    #[test]
    fn inbound_guest_remote_authenticates_and_exposes_key_only_when_ready() {
        let mut host = inbound_remote_awaiting_authentication();
        let address = BluetoothAddress::new([6, 5, 4, 3, 2, 1]);
        host.links[0].as_mut().unwrap().guest_pairing = true;
        let mut notification = alloc::vec![0x18, 23];
        notification.extend_from_slice(&address.bytes);
        notification.extend_from_slice(&[9; 16]);
        notification.push(4);
        host.receive_event(&notification, 0);
        assert!(host.hci.is_empty());
        assert!(host.next_event().is_none());
        command_status(&mut host, HciOpcode::AUTH_REQUESTED, 0);
        configure_peer_channel(&mut host, 0x11, 0x40, 0x50);
        configure_peer_channel(&mut host, 0x13, 0x41, 0x51);
        assert!(host.next_event().is_none());
        assert!(host.next_outgoing(0).is_none());
        host.receive_event(&[0x06, 3, 0, 1, 0], 0);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::WRITE_LINK_POLICY, alloc::vec![1, 0, 5, 0])
        );
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Connected {
                slot: 0,
                address,
                handle: 1,
                family: ControllerFamily::WiiRemote,
                calibration: None,
                guest_pairing: true,
                guest_key: Some([9; 16]),
            })
        );
        complete(&mut host, HciOpcode::WRITE_LINK_POLICY, &[1, 0]);
        report(&mut host, &[0xa1, 0x30, 0, 8]);
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Report {
                slot: 0,
                data: alloc::vec![0x30, 0, 8]
            })
        );
    }

    #[test]
    fn authentication_request_failure_and_completion_failure_remove_only_that_link() {
        for rejection_at_submission in [true, false] {
            let mut host = inbound_remote_awaiting_authentication();
            if rejection_at_submission {
                command_status(&mut host, HciOpcode::AUTH_REQUESTED, 5);
            } else {
                command_status(&mut host, HciOpcode::AUTH_REQUESTED, 0);
                host.receive_event(&[0x06, 3, 5, 1, 0], 0);
            }
            assert_eq!(host.diagnostics().stage, HostStage::Ready);
            assert_eq!(host.diagnostics().authentication_failures, 1);
            assert_eq!(host.diagnostics().last_authentication_status, Some(5));
            assert!(host.links[0].is_none());
            assert_eq!(
                next_hci(&mut host),
                (HciOpcode::DISCONNECT, alloc::vec![1, 0, 0x16])
            );
            assert_eq!(
                host.next_event(),
                Some(HidEvent::Disconnected {
                    slot: 0,
                    reason: 0x16
                })
            );
            assert_eq!(
                host.next_event(),
                Some(HidEvent::Failed {
                    slot: Some(0),
                    failure: Failure::Authentication
                })
            );
            assert!(host.next_event().is_none());
        }
    }

    #[test]
    fn guest_search_uses_liac_and_peer_pin_even_after_search_ends() {
        let mut host = ready();
        assert!(host.search_wii_guest(0));
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::PERIODIC_INQUIRY,
                alloc::vec![5, 0, 4, 0, 0, 0x8b, 0x9e, 3, 8]
            )
        );
        complete(&mut host, HciOpcode::PERIODIC_INQUIRY, &[]);
        host.receive_event(&[0x02, 15, 1, 6, 5, 4, 3, 2, 1, 1, 0, 0, 4, 5, 0, 0, 0], 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::CREATE_CONNECTION);
        command_status(&mut host, HciOpcode::CREATE_CONNECTION, 0);
        host.receive_event(&[0x01, 1, 0], 0);
        assert!(host.diagnostics().searching);
        host.stop_search();
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::EXIT_PERIODIC_INQUIRY, alloc::vec![])
        );
        complete(&mut host, HciOpcode::EXIT_PERIODIC_INQUIRY, &[]);
        host.receive_event(&[0x16, 6, 6, 5, 4, 3, 2, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::PIN_CODE_REQUEST_REPLY,
                alloc::vec![
                    6, 5, 4, 3, 2, 1, 6, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ]
            )
        );
    }

    #[test]
    fn guest_search_disconnects_wii_remotes_before_periodic_inquiry() {
        let mut host = configured(ControllerFamily::WiiRemote);
        let _ = host.next_event();
        assert_eq!(host.wii_remote_link_mask(), 1);

        assert!(host.search_wii_guest(10));
        assert!(host.has_link(0));
        assert!(host.diagnostics().searching);
        assert_eq!(host.hci.len(), 2);
        assert!(matches!(
            host.hci.pop_front(),
            Some(Outgoing::Hci {
                opcode: HciOpcode::DISCONNECT,
                params
            }) if params == [1, 0, 0x16]
        ));
        assert!(matches!(
            host.hci.pop_front(),
            Some(Outgoing::Hci {
                opcode: HciOpcode::PERIODIC_INQUIRY,
                params
            }) if params == [5, 0, 4, 0, 0, 0x8b, 0x9e, 3, 8]
        ));
    }

    #[test]
    fn guest_search_preserves_other_hid_links_and_fails_without_partial_queueing() {
        let mut host = configured(ControllerFamily::WiiRemote);
        let _ = host.next_event();
        let guest_address = BluetoothAddress::new([3, 3, 3, 3, 3, 3]);
        let registered_address = BluetoothAddress::new([4, 4, 4, 4, 4, 4]);
        host.load_guest_link_key(guest_address, [5; 16]);
        host.load_link_key(registered_address, [6; 16]);
        let generic_slot = host
            .allocate(BluetoothAddress::new([2, 3, 4, 5, 6, 7]), 0)
            .unwrap();
        let generic = host.links[generic_slot].as_mut().unwrap();
        generic.handle = Some(2);
        generic.family = ControllerFamily::Generic;
        generic.naming = false;
        generic.ready = true;
        assert_eq!(host.wii_remote_link_mask(), 1);

        host.hci
            .extend((0..(QUEUE_LIMIT - 1)).map(|_| Outgoing::Hci {
                opcode: HciOpcode::RESET,
                params: Vec::new(),
            }));
        let queued_before = host.hci.len();
        assert!(!host.search_wii_guest(10));
        assert_eq!(host.hci.len(), queued_before);
        assert!(
            host.keys
                .iter()
                .flatten()
                .any(|key| { key.address == guest_address && key.origin == LinkKeyOrigin::Guest })
        );
        assert!(host.keys.iter().flatten().any(|key| {
            key.address == registered_address && key.origin == LinkKeyOrigin::Registered
        }));
        assert!(host.has_link(0));
        assert!(host.has_link(generic_slot));
        assert!(!host.diagnostics().searching);
    }

    #[test]
    fn general_search_keeps_giac_and_normal_pin_and_finishes_after_one_cycle() {
        let mut host = ready();
        assert!(host.search(0));
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::INQUIRY, alloc::vec![0x33, 0x8b, 0x9e, 8, 8])
        );
        command_status(&mut host, HciOpcode::INQUIRY, 0);
        host.receive_event(
            &[0x02, 15, 1, 6, 5, 4, 3, 2, 1, 1, 0, 0, 4, 0x25, 0, 0, 0],
            0,
        );
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::CREATE_CONNECTION);
        command_status(&mut host, HciOpcode::CREATE_CONNECTION, 0);
        host.receive_event(&[0x01, 1, 0], 0);
        assert!(!host.diagnostics().searching);
        host.receive_event(&[0x16, 6, 6, 5, 4, 3, 2, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::PIN_CODE_REQUEST_REPLY,
                alloc::vec![
                    6, 5, 4, 3, 2, 1, 6, 1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ]
            )
        );
    }

    #[test]
    fn incoming_request_during_guest_search_cannot_replace_outgoing_link() {
        let mut host = ready();
        assert!(host.search_wii_guest(0));
        assert_eq!(next_hci(&mut host).0, HciOpcode::PERIODIC_INQUIRY);
        complete(&mut host, HciOpcode::PERIODIC_INQUIRY, &[]);
        host.receive_event(
            &[0x02, 15, 1, 6, 5, 4, 3, 2, 1, 1, 0, 0, 4, 0x25, 0, 0, 0],
            0,
        );
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::CREATE_CONNECTION);
        command_status(&mut host, HciOpcode::CREATE_CONNECTION, 0);
        host.receive_event(&[0x04, 10, 6, 5, 4, 3, 2, 1, 4, 0x25, 0, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::REJECT_CONNECTION_REQUEST,
                alloc::vec![6, 5, 4, 3, 2, 1, 0x0d]
            )
        );
        assert!(!host.links[0].as_ref().unwrap().incoming);
        assert!(host.links[0].as_ref().unwrap().guest_pairing);
        assert_eq!(host.diagnostics().connection_attempts, 1);
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn unallocated_incoming_remote_during_guest_search_uses_guest_pin() {
        let mut host = ready();
        assert!(host.search_wii_guest(0));
        assert_eq!(next_hci(&mut host).0, HciOpcode::PERIODIC_INQUIRY);
        complete(&mut host, HciOpcode::PERIODIC_INQUIRY, &[]);

        host.receive_event(&[0x04, 10, 6, 5, 4, 3, 2, 1, 4, 0x25, 0, 1], 0);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::ACCEPT_CONNECTION_REQUEST,
                alloc::vec![6, 5, 4, 3, 2, 1, 0]
            )
        );
        assert!(host.links[0].as_ref().unwrap().incoming);
        assert!(host.links[0].as_ref().unwrap().guest_pairing);
        command_status(&mut host, HciOpcode::ACCEPT_CONNECTION_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);

        host.receive_event(&[0x16, 6, 6, 5, 4, 3, 2, 1], 0);
        let (opcode, params) = next_hci(&mut host);
        assert_eq!(opcode, HciOpcode::PIN_CODE_REQUEST_REPLY);
        assert_eq!(&params[..13], &[6, 5, 4, 3, 2, 1, 6, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn outbound_interrupt_waits_for_complete_control_configuration() {
        let mut host = ready();
        host.receive_event(&[0x02, 15, 1, 6, 5, 4, 3, 2, 1, 1, 0, 0, 4, 5, 0, 0, 0], 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::REMOTE_NAME_REQUEST);
        command_status(&mut host, HciOpcode::REMOTE_NAME_REQUEST, 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::CREATE_CONNECTION);
        command_status(&mut host, HciOpcode::CREATE_CONNECTION, 0);
        host.receive_event(&[0x03, 11, 0, 1, 0, 6, 5, 4, 3, 2, 1, 1, 0], 0);
        assert_eq!(next_hci(&mut host).0, HciOpcode::AUTH_REQUESTED);
        command_status(&mut host, HciOpcode::AUTH_REQUESTED, 0);
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        let mut packet = [0; 128];
        let used = build_l2cap_connection_response(&mut packet, 1, 1, 0x50, 0x40, 0).unwrap();
        host.receive_acl(&packet[..used]);
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        assert!(host.next_outgoing(0).is_none());
        let used = build_l2cap_configuration_response(&mut packet, 1, 2, 0x50, 0).unwrap();
        host.receive_acl(&packet[..used]);
        assert!(host.next_outgoing(0).is_none());
        let used = build_l2cap_configuration_request(&mut packet, 1, 3, 0x40).unwrap();
        host.receive_acl(&packet[..used]);
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!("missing interrupt request")
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let signal = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(
            parse_l2cap_signal_payload(signal).unwrap(),
            L2capSignalPayload::ConnectionRequest {
                psm: 0x13,
                source_cid: 0x41
            }
        );
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn peer_channel_disconnect_queues_response_and_keeps_acl_link() {
        let mut host = configured(ControllerFamily::Generic);
        host.acl.clear();
        host.hci.clear();
        let _ = host.next_event();

        let local_cid = L2CAP_HID_CTRL_LOCAL_CID;
        let remote_cid = local_cid + 0x10;
        let incorrect_source_cid = remote_cid + 1;
        let request = [
            L2CAP_DISCONN_REQ,
            0x2a,
            4,
            0,
            local_cid as u8,
            (local_cid >> 8) as u8,
            incorrect_source_cid as u8,
            (incorrect_source_cid >> 8) as u8,
        ];
        host.signal(0, parse_l2cap_signal(&request).unwrap());

        let packet = match host.next_outgoing(0) {
            Some(Outgoing::Acl(packet)) => packet,
            other => panic!("expected L2CAP disconnection response, got {other:?}"),
        };
        let acl = parse_acl_packet(&packet).unwrap();
        assert_eq!(acl.handle, 1);
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        assert_eq!(l2cap.channel_id, L2CAP_CID_SIGNALING);
        let response = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(response.code, L2CAP_DISCONN_RSP);
        assert_eq!(response.identifier, 0x2a);
        assert_eq!(
            parse_l2cap_signal_payload(response).unwrap(),
            L2capSignalPayload::DisconnectionResponse {
                dest_cid: local_cid,
                source_cid: remote_cid,
            }
        );
        assert!(host.hci.is_empty());
        assert!(
            host.links[0]
                .as_ref()
                .is_some_and(|link| link.handle == Some(1))
        );
        let link = host.links[0].as_ref().unwrap();
        assert!(!link.ready);
        assert!(!link.channels.control.ready());
        assert!(link.channels.interrupt.ready());
        assert_eq!(
            host.next_event(),
            Some(HidEvent::ChannelDisconnected {
                slot: 0,
                channel: HidChannel::Control,
            })
        );
        assert_eq!(host.next_event(), None);
    }

    #[test]
    fn full_outgoing_queue_still_closes_peer_disconnection_channel() {
        let mut host = configured(ControllerFamily::Generic);
        host.acl.clear();
        host.hci.clear();
        let _ = host.next_event();
        host.acl.resize(QUEUE_LIMIT, alloc::vec![0; 4]);

        let local_cid = L2CAP_HID_CTRL_LOCAL_CID;
        let remote_cid = local_cid + 0x10;
        let request = [
            L2CAP_DISCONN_REQ,
            0x2c,
            4,
            0,
            local_cid as u8,
            (local_cid >> 8) as u8,
            remote_cid as u8,
            (remote_cid >> 8) as u8,
        ];

        host.signal(0, parse_l2cap_signal(&request).unwrap());

        let link = host.links[0]
            .as_ref()
            .expect("queue full must not remove the ACL link");
        assert_eq!(link.handle, Some(1));
        assert!(!link.ready);
        assert!(!link.channels.control.ready());
        assert!(link.channels.interrupt.ready());
        assert_eq!(host.acl.len(), QUEUE_LIMIT);
        assert!(host.hci.is_empty());
        assert_eq!(
            host.next_event(),
            Some(HidEvent::ChannelDisconnected {
                slot: 0,
                channel: HidChannel::Control,
            })
        );
        assert_eq!(host.next_event(), None);
    }

    #[test]
    fn unmatched_peer_channel_disconnect_gets_invalid_cid_reject() {
        let mut host = configured(ControllerFamily::Generic);
        host.acl.clear();
        host.hci.clear();
        let _ = host.next_event();

        let requested_local_cid = 0x0042;
        let request = [
            L2CAP_DISCONN_REQ,
            0x2b,
            4,
            0,
            requested_local_cid as u8,
            (requested_local_cid >> 8) as u8,
            0x52,
            0,
        ];
        host.signal(0, parse_l2cap_signal(&request).unwrap());

        let packet = match host.next_outgoing(0) {
            Some(Outgoing::Acl(packet)) => packet,
            other => panic!("expected invalid-CID command reject, got {other:?}"),
        };
        let acl = parse_acl_packet(&packet).unwrap();
        assert_eq!(acl.handle, 1);
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        assert_eq!(l2cap.channel_id, L2CAP_CID_SIGNALING);
        let reject = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(reject.code, L2CAP_CMD_REJECT);
        assert_eq!(reject.identifier, 0x2b);
        assert_eq!(
            reject.payload,
            &[L2CAP_INVALID_CID as u8, 0, 0x42, 0, L2CAP_NULL_CID as u8, 0]
        );

        let link = host.links[0].as_ref().unwrap();
        assert_eq!(link.handle, Some(1));
        assert!(link.ready);
        assert!(link.channels.control.ready());
        assert!(link.channels.interrupt.ready());
        assert!(host.hci.is_empty());
        assert_eq!(host.next_event(), None);
    }

    #[test]
    fn inactive_reserved_peer_cid_gets_invalid_cid_reject() {
        let mut host = configured(ControllerFamily::Generic);
        host.acl.clear();
        host.hci.clear();
        let _ = host.next_event();
        let initial_request = [L2CAP_DISCONN_REQ, 0x2b, 4, 0, 0x40, 0, 0x50, 0];
        host.signal(0, parse_l2cap_signal(&initial_request).unwrap());
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        assert_eq!(
            host.next_event(),
            Some(HidEvent::ChannelDisconnected {
                slot: 0,
                channel: HidChannel::Control,
            })
        );

        let request = [L2CAP_DISCONN_REQ, 0x2c, 4, 0, 0x40, 0, 0x50, 0];
        host.signal(0, parse_l2cap_signal(&request).unwrap());

        let packet = match host.next_outgoing(0) {
            Some(Outgoing::Acl(packet)) => packet,
            other => panic!("expected invalid-CID command reject, got {other:?}"),
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let reject = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(reject.code, L2CAP_CMD_REJECT);
        assert_eq!(reject.identifier, 0x2c);
        assert_eq!(reject.payload, &[2, 0, 0x40, 0, 0, 0]);
        assert!(!host.links[0].as_ref().unwrap().channels.control.active);
        assert!(host.links[0].as_ref().unwrap().channels.interrupt.active);
        assert_eq!(host.next_event(), None);
    }

    #[test]
    fn unknown_l2cap_signal_gets_not_understood_command_reject() {
        let mut host = configured(ControllerFamily::Generic);
        host.acl.clear();
        for code in [0x0a, 0x0b, 0x80] {
            host.signal(
                0,
                L2capSignal {
                    code,
                    identifier: 0x2a,
                    payload: &[],
                },
            );

            let packet = host.acl.pop_front().expect("command reject queued");
            let l2cap = parse_l2cap_packet(&packet[4..]).unwrap();
            assert_eq!(l2cap.channel_id, L2CAP_CID_SIGNALING);
            let reject = parse_l2cap_signal(l2cap.payload).unwrap();
            assert_eq!(reject.code, L2CAP_CMD_REJECT);
            assert_eq!(reject.identifier, 0x2a);
            assert_eq!(reject.payload, &[0, 0]);
        }
    }

    fn report(host: &mut HidHost, payload: &[u8]) {
        let mut packet = [0; 256];
        let len = build_l2cap_data(&mut packet, 1, L2CAP_HID_INTR_LOCAL_CID, payload).unwrap();
        host.receive_acl(&packet[..len]);
    }

    fn switch_ack(host: &mut HidHost, subcommand: u8, accepted: bool, data: &[u8]) {
        let mut packet = alloc::vec![0u8; 16 + data.len()];
        packet[0] = 0xa1;
        packet[1] = switch::SWITCH_HID_INPUT_SUBCOMMAND_ACK;
        packet[14] = if accepted { 0x80 } else { 0 };
        packet[15] = subcommand;
        packet[16..].copy_from_slice(data);
        report(host, &packet);
    }

    fn switch_spi_echo(address: u32, data: &[u8]) -> Vec<u8> {
        let mut echo = address.to_le_bytes().to_vec();
        echo.push(data.len() as u8);
        echo.extend_from_slice(data);
        echo
    }

    fn user_calibration_fixture() -> [u8; 22] {
        [
            0xb2, 0xa1, 0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32, 0xb2, 0xa1, 0xd0,
            0x47, 0x83, 0x84, 0x03, 0x32, 0xe8, 0xc3, 0x44,
        ]
    }

    fn factory_calibration_fixture() -> [u8; 18] {
        [
            0xe8, 0xc3, 0x44, 0xd0, 0x47, 0x83, 0x84, 0x03, 0x32, 0xd0, 0x47, 0x83, 0x84, 0x03,
            0x32, 0xe8, 0xc3, 0x44,
        ]
    }
    #[test]
    fn output_data_uses_interrupt_channel_for_switch_and_xbox() {
        for family in [
            ControllerFamily::SwitchPro,
            ControllerFamily::Xbox,
            ControllerFamily::WiiRemote,
        ] {
            let mut host = configured(family);
            host.acl.clear();
            assert!(host.send_report(0, &[1, 2, 3]));
            let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
                panic!()
            };
            let acl = parse_acl_packet(&packet).unwrap();
            let l2cap = parse_l2cap_packet(acl.payload).unwrap();
            assert_eq!(l2cap.channel_id, L2CAP_HID_INTR_LOCAL_CID + 0x10);
            assert_eq!(l2cap.payload, &[0xa2, 1, 2, 3]);
        }
    }
    #[test]
    fn configuration_error_preserves_opcode_status_and_failed_stage() {
        let mut host = HidHost::new();
        host.start();
        host.next_outgoing(0);
        complete(&mut host, HciOpcode::RESET, &[]);
        host.next_outgoing(0);
        complete(&mut host, HciOpcode::new(4, 5), &[64, 0, 0, 8, 0, 0, 0]);
        let opcode = HciOpcode::new(3, 0x24);
        assert_eq!(
            host.next_outgoing(0),
            Some(Outgoing::Hci {
                opcode,
                params: alloc::vec![0, 0x1f, 0]
            })
        );
        let mut event = alloc::vec![0x0e, 4, 1];
        event.extend_from_slice(&opcode.raw().to_le_bytes());
        event.push(0x12);
        host.receive_event(&event, 0);
        let diagnostic = host.diagnostics();
        assert_eq!(diagnostic.stage, HostStage::Failed);
        assert_eq!(diagnostic.failed_stage, Some(HostStage::Configure));
        assert_eq!(diagnostic.failed_opcode, Some(opcode));
        assert_eq!(diagnostic.hci_status, Some(0x12));
    }

    #[test]
    fn continued_configuration_response_sends_empty_request() {
        let mut host = configured(ControllerFamily::WiiRemote);
        host.acl.clear();
        host.links[0]
            .as_mut()
            .unwrap()
            .channels
            .control
            .local_configured = false;

        let response = [L2CAP_CFG_RSP, 9, 6, 0, 0x50, 0, 1, 0, 0, 0];
        host.signal(0, parse_l2cap_signal(&response).unwrap());

        assert!(
            host.links[0]
                .as_ref()
                .unwrap()
                .channels
                .control
                .local_configured
        );
        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!("expected the continuation configuration request");
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let signal = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(signal.code, L2CAP_CFG_REQ);
        assert_eq!(
            parse_l2cap_signal_payload(signal).unwrap(),
            L2capSignalPayload::ConfigurationRequest {
                dest_cid: 0x0050,
                flags: 0,
                options: &[],
            }
        );
    }

    #[test]
    fn simple_pairing_is_enabled_only_when_adapter_supports_it() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::Features;
        host.queue_hci(HciOpcode::new(4, 3), &[]);
        host.next_outgoing(0);
        complete(&mut host, HciOpcode::new(4, 3), &[0, 0, 0, 0, 0, 0, 8, 0]);
        assert!(host.diagnostics().simple_pairing);
        assert_eq!(
            host.next_outgoing(0),
            Some(Outgoing::Hci {
                opcode: HciOpcode::new(3, 0x56),
                params: alloc::vec![1]
            })
        );
        complete(&mut host, HciOpcode::new(3, 0x56), &[]);
        for (opcode, params) in [
            (HciOpcode::new(3, 0x45), &[1][..]),
            (HciOpcode::new(3, 0x47), &[1][..]),
            (HciOpcode::new(3, 0x43), &[1][..]),
            (HciOpcode::new(3, 0x24), &[0x04, 0x02, 0x40][..]),
            (HciOpcode::new(3, 0x18), &[0, 0x20][..]),
        ] {
            assert_eq!(next_hci(&mut host), (opcode, params.to_vec()));
            complete(&mut host, opcode, &[]);
        }
        assert_eq!(host.diagnostics().stage, HostStage::StoredKeys);
    }

    #[test]
    fn adapter_patch_command_failure_stops_host_initialization() {
        let mut host = awaiting_patch_start();
        let opcode = HciOpcode::new(0x3f, 0x4f);
        assert_eq!(next_hci(&mut host).0, opcode);
        complete_with_status(&mut host, opcode, 0x0c);
        let diagnostics = host.diagnostics();
        assert_eq!(diagnostics.stage, HostStage::Failed);
        assert_eq!(diagnostics.failed_stage, Some(HostStage::AdapterPatch));
        assert_eq!(diagnostics.failed_opcode, Some(opcode));
        assert_eq!(diagnostics.hci_status, Some(0x0c));
        assert_eq!(diagnostics.failure, Some(Failure::Protocol));
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn malformed_stored_key_completion_does_not_start_adapter_patch() {
        let mut host = HidHost::new();
        host.diagnostic.stage = HostStage::StoredKeys;
        host.command = Some(PendingCommand {
            opcode: HciOpcode::new(3, 0x0d),
            deadline: COMMAND_TIMEOUT_MS,
            authentication_handle: None,
        });
        complete(&mut host, HciOpcode::new(3, 0x0d), &[]);
        assert_eq!(host.diagnostics().stage, HostStage::Failed);
        assert_eq!(host.diagnostics().failed_stage, Some(HostStage::StoredKeys));
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn adapter_patch_requires_command_complete_not_command_status() {
        let mut host = awaiting_patch_start();
        let opcode = HciOpcode::new(0x3f, 0x4f);
        assert_eq!(next_hci(&mut host).0, opcode);
        command_status(&mut host, opcode, 0);
        assert_eq!(host.diagnostics().stage, HostStage::Failed);
        assert_eq!(
            host.diagnostics().failed_stage,
            Some(HostStage::AdapterPatch)
        );
        assert_eq!(host.diagnostics().failure, Some(Failure::Protocol));
        assert!(host.next_outgoing(0).is_none());
    }

    #[test]
    fn ssp_confirmation_rejects_unknown_addresses() {
        let mut host = ready();
        host.diagnostic.simple_pairing = true;
        host.receive_event(&[0x33, 10, 1, 2, 3, 4, 5, 6, 0, 0, 0, 0], 0);
        assert_eq!(
            host.next_outgoing(0),
            Some(Outgoing::Hci {
                opcode: HciOpcode::new(1, 0x2d),
                params: alloc::vec![1, 2, 3, 4, 5, 6]
            })
        );
        host.command = None;
        host.allocate(BluetoothAddress::new([1, 2, 3, 4, 5, 6]), 0);
        host.receive_event(&[0x31, 6, 1, 2, 3, 4, 5, 6], 0);
        assert_eq!(
            host.next_outgoing(0),
            Some(Outgoing::Hci {
                opcode: HciOpcode::new(1, 0x2b),
                params: alloc::vec![1, 2, 3, 4, 5, 6, 3, 0, 1]
            })
        );
    }
    #[test]
    fn link_key_notification_updates_the_cached_key() {
        let mut host = ready();
        let address = BluetoothAddress::new([1; 6]);
        host.keys[2] = Some(StoredLinkKey {
            address,
            key: [1; 16],
            origin: LinkKeyOrigin::Guest,
        });
        let slot = host.allocate(address, 0).unwrap();
        host.links[slot].as_mut().unwrap().guest_pairing = true;
        let mut event = alloc::vec![0x18, 23];
        event.extend_from_slice(&address.bytes);
        event.extend_from_slice(&[2; 16]);
        event.push(4);
        host.receive_event(&event, 0);
        assert_eq!(
            host.keys[2],
            Some(StoredLinkKey {
                address,
                key: [2; 16],
                origin: LinkKeyOrigin::Guest,
            })
        );
        assert!(host.keys[0].is_none());
        assert!(host.next_event().is_none());
        assert!(host.hci.is_empty());
    }
    #[test]
    fn normal_pairing_writes_registered_key_without_guest_config_event() {
        let mut host = ready();
        let address = BluetoothAddress::new([2; 6]);
        host.allocate(address, 0).unwrap();
        let mut event = alloc::vec![0x18, 23];
        event.extend_from_slice(&address.bytes);
        event.extend_from_slice(&[3; 16]);
        event.push(4);

        host.receive_event(&event, 0);

        assert_eq!(
            host.keys
                .iter()
                .flatten()
                .find(|key| key.address == address),
            Some(&StoredLinkKey {
                address,
                key: [3; 16],
                origin: LinkKeyOrigin::Registered,
            })
        );
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::new(3, 0x11),
                alloc::vec![
                    1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3
                ]
            )
        );
        assert!(host.next_event().is_none());
    }
    #[test]
    fn guest_search_clears_guest_key_cache_and_preserves_registered_keys() {
        let mut host = ready();
        let guest_address = BluetoothAddress::new([3; 6]);
        let registered_address = BluetoothAddress::new([4; 6]);
        host.load_guest_link_key(guest_address, [5; 16]);
        host.load_link_key(registered_address, [6; 16]);
        host.clear_guest_link_keys();
        assert!(
            host.keys
                .iter()
                .flatten()
                .all(|key| key.address != guest_address)
        );
        assert!(
            host.keys
                .iter()
                .flatten()
                .any(|key| key.address == registered_address)
        );
        host.load_guest_link_key(guest_address, [5; 16]);
        assert!(host.search_wii_guest(1));
        assert!(host.next_event().is_none());
        assert_eq!(next_hci(&mut host).0, HciOpcode::PERIODIC_INQUIRY);
        complete(&mut host, HciOpcode::PERIODIC_INQUIRY, &[]);

        let mut request = alloc::vec![0x17, 6];
        request.extend_from_slice(&guest_address.bytes);
        host.receive_event(&request, 2);
        assert_eq!(
            next_hci(&mut host),
            (
                HciOpcode::LINK_KEY_REQUEST_NEG_REPLY,
                guest_address.bytes.to_vec()
            )
        );
        complete(&mut host, HciOpcode::LINK_KEY_REQUEST_NEG_REPLY, &[]);

        request[2..].copy_from_slice(&registered_address.bytes);
        host.receive_event(&request, 3);
        let mut expected = registered_address.bytes.to_vec();
        expected.extend_from_slice(&[6; 16]);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::LINK_KEY_REQUEST_REPLY, expected)
        );
    }
    #[test]
    fn event_backpressure_retains_events_until_the_consumer_makes_space() {
        let mut host = HidHost::new();
        for value in 0..QUEUE_LIMIT {
            assert!(host.has_event_capacity());
            host.emit(HidEvent::Report {
                slot: 0,
                data: alloc::vec![0x30, value as u8],
            });
        }
        assert!(!host.has_event_capacity());
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Report {
                slot: 0,
                data: alloc::vec![0x30, 0],
            })
        );
        assert!(host.has_event_capacity());
        for value in 1..QUEUE_LIMIT {
            assert_eq!(
                host.next_event(),
                Some(HidEvent::Report {
                    slot: 0,
                    data: alloc::vec![0x30, value as u8],
                })
            );
        }
        assert!(host.next_event().is_none());
        assert_eq!(host.diagnostics().failure, None);
    }

    #[test]
    fn acl_connection_failure_preserves_all_events_when_queue_has_one_slot() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event(); // discard the initial Connected event
        for value in 0..(QUEUE_LIMIT - 1) {
            host.emit(HidEvent::Report {
                slot: 4,
                data: alloc::vec![0x30, value as u8],
            });
        }

        let mut packet = [0; 64];
        let used =
            build_l2cap_connection_response(&mut packet, 1, 1, L2CAP_HID_CTRL_LOCAL_CID, 0x0050, 2)
                .unwrap();
        host.receive_acl(&packet[..used]);

        assert_eq!(host.events.len(), QUEUE_LIMIT + 1);
        assert!(matches!(
            host.events[QUEUE_LIMIT - 1],
            HidEvent::Disconnected { slot: 0, .. }
        ));
        assert!(!host.has_event_capacity());

        for value in 0..(QUEUE_LIMIT - 1) {
            assert!(matches!(
                host.next_event(),
                Some(HidEvent::Report { slot: 4, data }) if data == [0x30, value as u8]
            ));
        }
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Disconnected { slot: 0, .. })
        ));
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Failed {
                slot: Some(0),
                failure: Failure::Channel,
            })
        );
    }

    #[test]
    fn peek_event_preserves_event_until_processing_succeeds() {
        let mut host = HidHost::new();
        let event = HidEvent::Failed {
            slot: None,
            failure: Failure::Protocol,
        };
        host.emit(event.clone());

        assert_eq!(host.peek_event(), Some(event.clone()));
        assert_eq!(host.peek_event(), Some(event.clone()));
        assert_eq!(host.next_event(), Some(event));
        assert!(host.peek_event().is_none());
    }
    #[test]
    fn host_reset_keeps_guest_keys_in_volatile_cache() {
        let mut host = HidHost::new();
        let address = BluetoothAddress::new([6, 5, 4, 3, 2, 1]);
        host.load_guest_link_key(address, [9; 16]);
        host.reset_preserving_keys();

        assert!(host.next_event().is_none());
        host.diagnostic.stage = HostStage::Ready;
        let mut request = alloc::vec![0x17, 6];
        request.extend_from_slice(&address.bytes);
        host.receive_event(&request, 0);
        let mut expected = address.bytes.to_vec();
        expected.extend_from_slice(&[9; 16]);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::LINK_KEY_REQUEST_REPLY, expected)
        );
    }
    #[test]
    fn foreign_command_status_does_not_fail_startup() {
        let mut host = HidHost::new();
        host.start();
        host.next_outgoing(0);
        host.receive_event(&[0x0f, 4, 0x12, 1, 0xff, 0xff], 0);
        assert_eq!(host.diagnostics().stage, HostStage::Reset);
        complete(&mut host, HciOpcode::RESET, &[]);
        assert_eq!(host.diagnostics().stage, HostStage::BufferSize);
    }
    #[test]
    fn startup_timeout_is_terminal_and_idempotent_start_does_not_duplicate_reset() {
        let mut host = HidHost::new();
        host.start();
        host.start();
        assert!(host.next_outgoing(0).is_some());
        assert_eq!(host.next_outgoing(0), None);
        host.tick(COMMAND_TIMEOUT_MS);
        assert_eq!(host.diagnostics().failure, Some(Failure::Timeout));
        assert_eq!(host.diagnostics().stage, HostStage::Failed);
        assert_eq!(host.diagnostics().failed_opcode, Some(HciOpcode::RESET));
        assert_eq!(host.diagnostics().hci_status, None);
    }
    #[test]
    fn six_byte_scan_completion_is_valid() {
        let _ = ready();
    }
    #[test]
    fn controller_allocator_uses_all_six_wpad_slots() {
        let mut host = HidHost::new();
        let slots = (0..HID_SLOTS)
            .map(|address| {
                host.allocate(BluetoothAddress::new([address as u8 + 1; 6]), 0)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(slots, [0, 1, 2, 3, 4, 5]);
        assert_eq!(host.allocate(BluetoothAddress::new([7; 6]), 0), None);
    }
    #[test]
    fn switch_ready_waits_for_calibration_and_exposes_user_axes() {
        let mut host = configured(ControllerFamily::SwitchPro);
        assert_eq!(host.next_event(), None);
        let Some(Outgoing::Acl(low_power)) = host.next_outgoing(0) else {
            panic!("missing low-power request");
        };
        assert_eq!(
            &low_power[8..],
            &[0xa2, 1, 0, 0, 1, 64, 64, 0, 1, 64, 64, 8, 0]
        );
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let Some(Outgoing::Acl(report_mode)) = host.next_outgoing(0) else {
            panic!("missing report-mode request");
        };
        assert_eq!(
            &report_mode[8..],
            &[0xa2, 1, 1, 0, 1, 64, 64, 0, 1, 64, 64, 3, 48]
        );
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let Some(Outgoing::Acl(user_read)) = host.next_outgoing(0) else {
            panic!("missing user calibration read");
        };
        assert_eq!(
            &user_read[8..],
            &[
                0xa2, 1, 2, 0, 1, 64, 64, 0, 1, 64, 64, 16, 16, 128, 0, 0, 22
            ]
        );
        let user = user_calibration_fixture();
        let echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &user);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected {
                family: ControllerFamily::SwitchPro,
                ..
            })
        ));
        assert_eq!(host.next_event(), None);
        let calibration = host.switch_calibration(0).unwrap();
        let left = calibration.left.unwrap();
        assert_eq!((left.x.min, left.x.center, left.x.max), (1100, 2000, 3000));
        assert_eq!(left.source, switch::SwitchCalibrationSource::User);
        report(&mut host, &[0xa1, 0x30, 1, 2, 3]);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Report { data, .. }) if data == [0x30, 1, 2, 3]
        ));
    }

    #[test]
    fn connected_event_keeps_calibration_after_later_disconnect() {
        let mut host = configured(ControllerFamily::SwitchPro);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report-mode request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user-calibration request");
        let user = user_calibration_fixture();
        let echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &user);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        let expected = host.switch_calibration(0).unwrap();
        host.disconnect(0);
        assert_eq!(host.switch_calibration(0), None);

        assert_eq!(
            host.next_event(),
            Some(HidEvent::Connected {
                slot: 0,
                address: BluetoothAddress::new([6, 5, 4, 3, 2, 1]),
                handle: 1,
                family: ControllerFamily::SwitchPro,
                calibration: Some(expected),
                guest_pairing: false,
                guest_key: None,
            })
        );
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Disconnected {
                slot: 0,
                reason: 0x16,
            })
        );
    }

    #[test]
    fn switch_reads_factory_data_only_when_user_magic_is_absent() {
        let mut host = configured(ControllerFamily::SwitchPro);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report mode");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user calibration read");
        let absent = [0u8; 22];
        let echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &absent);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        let Some(Outgoing::Acl(factory_read)) = host.next_outgoing(0) else {
            panic!("missing factory calibration read");
        };
        assert_eq!(
            &factory_read[8..],
            &[0xa2, 1, 3, 0, 1, 64, 64, 0, 1, 64, 64, 16, 61, 96, 0, 0, 18]
        );
        let factory = factory_calibration_fixture();
        let echo = switch_spi_echo(switch::SWITCH_FACTORY_CALIBRATION_ADDRESS, &factory);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected { .. })
        ));
        let calibration = host.switch_calibration(0).unwrap();
        assert_eq!(
            calibration.left.unwrap().source,
            switch::SwitchCalibrationSource::Factory
        );
        assert_eq!(
            calibration.right.unwrap().source,
            switch::SwitchCalibrationSource::Factory
        );
    }

    #[test]
    fn switch_bad_spi_echo_and_invalid_present_user_data_fail_protocol() {
        let mut host = configured(ControllerFamily::SwitchPro);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report mode");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user calibration read");
        let user = user_calibration_fixture();
        let mut echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &user);
        echo[0] ^= 1;
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Disconnected { reason: 0x16, .. })
        ));
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Failed {
                failure: Failure::Protocol,
                ..
            })
        ));
        assert_eq!(host.switch_calibration(0), None);

        let mut host = configured(ControllerFamily::JoyConLeft);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report mode");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user calibration read");
        let mut malformed = user_calibration_fixture();
        malformed[2..11].fill(0);
        let echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &malformed);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Disconnected { reason: 0x16, .. })
        ));
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Failed {
                failure: Failure::Protocol,
                ..
            })
        ));
    }

    #[test]
    fn joycon_left_ignores_malformed_calibration_for_unused_right_stick() {
        let mut host = configured(ControllerFamily::JoyConLeft);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report mode");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user calibration read");
        let mut user = user_calibration_fixture();
        user[13..22].fill(0);
        switch_ack(
            &mut host,
            switch::SWITCH_SUBCMD_SPI_FLASH_READ,
            true,
            &switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &user),
        );
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected { .. })
        ));
        let calibration = host.switch_calibration(0).unwrap();
        assert!(calibration.left.is_some());
        assert!(calibration.right.is_none());
    }

    #[test]
    fn switch_nack_and_setup_timeout_retry_then_fail_without_ready() {
        let mut host = configured(ControllerFamily::SwitchPro);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, false, &[]);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Disconnected { reason: 0x16, .. })
        ));
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Failed {
                failure: Failure::Protocol,
                ..
            })
        ));

        let mut host = configured(ControllerFamily::SwitchPro);
        let Some(Outgoing::Acl(first)) = host.next_outgoing(0) else {
            panic!("missing first setup packet");
        };
        host.tick(COMMAND_TIMEOUT_MS);
        let Some(Outgoing::Acl(retry)) = host.next_outgoing(COMMAND_TIMEOUT_MS) else {
            panic!("missing bounded retry");
        };
        assert_eq!(first[8], retry[8]);
        assert_eq!(first[9], retry[9]);
        assert_ne!(first[10], retry[10]);
        host.tick(COMMAND_TIMEOUT_MS * 2);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Disconnected { reason: 0x16, .. })
        ));
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Failed {
                failure: Failure::Timeout,
                ..
            })
        ));
        assert_eq!(host.switch_calibration(0), None);
    }

    #[test]
    fn switch_calibration_getter_waits_for_the_applicable_single_joycon_stick() {
        let mut host = configured(ControllerFamily::JoyConLeft);
        let _ = host.next_outgoing(0).expect("low-power request");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_LOW_POWER_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("report mode");
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SET_REPORT_MODE, true, &[]);
        let _ = host.next_outgoing(0).expect("user calibration read");
        let mut user = user_calibration_fixture();
        user[11..13].fill(0);
        let echo = switch_spi_echo(switch::SWITCH_USER_CALIBRATION_ADDRESS, &user);
        switch_ack(&mut host, switch::SWITCH_SUBCMD_SPI_FLASH_READ, true, &echo);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected { .. })
        ));
        let calibration = host.switch_calibration(0).unwrap();
        assert!(calibration.left.is_some());
        assert!(calibration.right.is_none());
    }

    #[test]
    fn output_queue_failure_is_not_submission_success() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        for _ in 0..QUEUE_LIMIT {
            assert!(host.send_report(0, &[1]));
        }
        assert!(!host.send_report(0, &[1]));
        assert_eq!(host.diagnostics().failure, Some(Failure::QueueFull));
    }
    #[test]
    fn disconnect_request_waits_for_adapter_completion() {
        let mut host = configured(ControllerFamily::Generic);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected { slot: 0, .. })
        ));
        assert_eq!(host.request_disconnect(0), Ok(()));
        assert!(host.has_link(0));
        assert!(!host.has_link(HID_SLOTS));
        assert_eq!(host.next_event(), None);
        assert_eq!(
            next_hci(&mut host),
            (HciOpcode::DISCONNECT, alloc::vec![1, 0, 0x16])
        );
        host.receive_event(&[5, 4, 0, 1, 0, 0x16], 0);
        assert!(!host.has_link(0));
        assert!(!host.has_link(HID_SLOTS));
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Disconnected {
                slot: 0,
                reason: 0x16
            })
        );
    }
    #[test]
    fn rejected_disconnect_request_preserves_the_live_link() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        assert_eq!(host.request_disconnect(HID_SLOTS), Err(Failure::Protocol));
        for _ in 0..QUEUE_LIMIT {
            host.hci.push_back(Outgoing::Hci {
                opcode: HciOpcode::RESET,
                params: Vec::new(),
            });
        }
        assert_eq!(host.request_disconnect(0), Err(Failure::QueueFull));
        assert!(host.links[0].is_some());
        assert_eq!(host.hci.len(), QUEUE_LIMIT);
        assert_eq!(host.next_event(), None);
    }
    #[test]
    fn disconnect_clears_only_the_matching_slot() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        let slot = host.allocate(BluetoothAddress::new([2; 6]), 0).unwrap();
        host.links[slot].as_mut().unwrap().handle = Some(2);
        host.receive_event(&[5, 4, 0, 1, 0, 0x13], 0);
        assert!(host.links[0].is_none());
        assert!(host.links[1].is_some());
        assert_eq!(
            host.next_event(),
            Some(HidEvent::Disconnected {
                slot: 0,
                reason: 0x13
            })
        );
    }
    #[test]
    fn disconnect_ignores_out_of_range_slots() {
        let mut host = HidHost::new();
        host.disconnect(usize::MAX);
        assert_eq!(host.next_event(), None);
    }
    #[test]
    fn fragmented_acl_reassembles_before_report_decode() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        let mut packet = [0; 64];
        let len = build_l2cap_data(
            &mut packet,
            1,
            L2CAP_HID_INTR_LOCAL_CID,
            &[0xa1, 1, 2, 3, 4, 5],
        )
        .unwrap();
        let mut first = packet[..9].to_vec();
        first[2..4].copy_from_slice(&5u16.to_le_bytes());
        host.receive_acl(&first);
        assert_eq!(host.next_event(), None);
        let mut rest = alloc::vec![1, 0x10, (len - 9) as u8, 0];
        rest.extend_from_slice(&packet[9..len]);
        host.receive_acl(&rest);
        assert!(
            matches!(host.next_event(), Some(HidEvent::Report { data, .. }) if data == [1,2,3,4,5])
        );
    }
    #[test]
    fn foreign_or_duplicate_completions_do_not_create_acl_credits() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        host.acl_credits = 1;
        assert!(host.send_report(0, &[1]));
        host.next_outgoing(0);
        host.receive_event(&[0x13, 5, 1, 2, 0, 20, 0], 0);
        assert_eq!(host.acl_credits, 0);
        host.receive_event(&[0x13, 5, 1, 1, 0, 20, 0], 0);
        assert_eq!(host.acl_credits, 1);
        host.receive_event(&[0x13, 5, 1, 1, 0, 20, 0], 0);
        assert_eq!(host.acl_credits, 1);
    }
    #[test]
    fn credits_prevent_unbounded_acl_writes() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        host.acl_credits = 1;
        assert!(host.send_report(0, &[1, 2]));
        assert!(host.send_report(0, &[3, 4]));
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        assert_eq!(host.next_outgoing(0), None);
        host.receive_event(&[0x13, 5, 1, 1, 0, 1, 0], 0);
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
    }
    #[test]
    fn malformed_packets_and_foreign_handles_do_not_change_buttons() {
        let mut host = configured(ControllerFamily::Generic);
        host.next_event();
        host.receive_acl(&[1]);
        host.receive_event(&[0x0e, 4], 0);
        assert_eq!(host.diagnostics().malformed, 2);
        assert_eq!(host.next_event(), None);
    }
    #[test]
    fn wii_pin_uses_adapter_address_and_cached_keys_reply() {
        let mut host = configured(ControllerFamily::WiiRemote);
        host.next_event();
        let address = [6, 5, 4, 3, 2, 1];
        let mut event = alloc::vec![0x16, 6];
        event.extend_from_slice(&address);
        host.receive_event(&event, 0);
        assert!(
            matches!(host.next_outgoing(0), Some(Outgoing::Hci { opcode:HciOpcode::PIN_CODE_REQUEST_REPLY,params }) if params[6]==6 && params[7..13]==[1,2,3,4,5,6])
        );
        host.keys[0] = Some(StoredLinkKey {
            address: BluetoothAddress::new(address),
            key: [7; 16],
            origin: LinkKeyOrigin::Registered,
        });
        host.command = None;
        event[0] = 0x17;
        host.receive_event(&event, 0);
        assert!(
            matches!(host.next_outgoing(0), Some(Outgoing::Hci { opcode:HciOpcode::LINK_KEY_REQUEST_REPLY,params }) if params[6..]==[7;16])
        );
    }
    #[test]
    fn inbound_hid_channel_gets_a_connection_and_config_response() {
        let mut host = ready();
        let slot = host.allocate(BluetoothAddress::new([1; 6]), 0).unwrap();
        host.links[slot].as_mut().unwrap().handle = Some(1);
        let mut packet = [0; 64];
        let len =
            build_l2cap_connection_request(&mut packet, 1, 3, L2CAP_PSM_HID_CTRL, 0x55).unwrap();
        host.receive_acl(&packet[..len]);
        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!()
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let signal = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(
            parse_l2cap_signal_payload(signal).unwrap(),
            L2capSignalPayload::ConnectionResponse {
                dest_cid: 0x40,
                source_cid: 0x55,
                result: 0,
                status: 0
            }
        );
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
    }

    #[test]
    fn continued_configuration_request_accumulates_unknown_options() {
        let mut host = configured(ControllerFamily::Generic);
        while host.next_outgoing(0).is_some() {}
        host.links[0]
            .as_mut()
            .unwrap()
            .channels
            .control
            .remote_configured = false;

        let first = [L2CAP_CFG_REQ, 8, 7, 0, 0x40, 0, 1, 0, 4, 1, 0xaa];
        host.signal(0, parse_l2cap_signal(&first).unwrap());
        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!("expected continuation response");
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let response = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(
            parse_l2cap_signal_payload(response).unwrap(),
            L2capSignalPayload::ConfigurationResponse {
                source_cid: 0x50,
                flags: 0,
                result: L2CAP_CFG_SUCCESS,
                options: &[],
            }
        );

        let final_segment = [L2CAP_CFG_REQ, 9, 4, 0, 0x40, 0, 0, 0];
        host.signal(0, parse_l2cap_signal(&final_segment).unwrap());
        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!("expected final configuration response");
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let response = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(
            parse_l2cap_signal_payload(response).unwrap(),
            L2capSignalPayload::ConfigurationResponse {
                source_cid: 0x50,
                flags: 0,
                result: L2CAP_CFG_UNKNOWN,
                options: &[4, 1, 0xaa],
            }
        );
        assert!(
            !host.links[0]
                .as_ref()
                .unwrap()
                .channels
                .control
                .remote_configured
        );
    }

    #[test]
    fn continued_non_success_configuration_response_does_not_disconnect() {
        let mut host = configured(ControllerFamily::Generic);
        assert!(matches!(
            host.next_event(),
            Some(HidEvent::Connected {
                family: ControllerFamily::Generic,
                ..
            })
        ));
        while host.next_outgoing(0).is_some() {}
        let signal = [
            L2CAP_CFG_RSP,
            10,
            6,
            0,
            0x50,
            0,
            1,
            0,
            L2CAP_CFG_UNKNOWN as u8,
            0,
        ];
        host.signal(0, parse_l2cap_signal(&signal).unwrap());
        assert!(host.links[0].is_some());
        assert!(matches!(host.next_outgoing(0), Some(Outgoing::Acl(_))));
        assert_eq!(host.next_event(), None);
    }

    #[test]
    fn required_unknown_configuration_option_gets_unknown_response() {
        let mut host = configured(ControllerFamily::Generic);
        while host.next_outgoing(0).is_some() {}
        host.links[0]
            .as_mut()
            .unwrap()
            .channels
            .control
            .remote_configured = false;
        let signal = [L2CAP_CFG_REQ, 7, 7, 0, 0x40, 0, 0, 0, 4, 1, 0xaa];

        host.signal(0, parse_l2cap_signal(&signal).unwrap());

        let Some(Outgoing::Acl(packet)) = host.next_outgoing(0) else {
            panic!("expected a configuration response");
        };
        let acl = parse_acl_packet(&packet).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let response = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(response.code, L2CAP_CFG_RSP);
        assert_eq!(
            parse_l2cap_signal_payload(response).unwrap(),
            L2capSignalPayload::ConfigurationResponse {
                source_cid: 0x50,
                flags: 0,
                result: L2CAP_CFG_UNKNOWN,
                options: &[4, 1, 0xaa],
            }
        );
        assert!(host.links[0].is_some());
        assert!(
            !host.links[0]
                .as_ref()
                .unwrap()
                .channels
                .control
                .remote_configured
        );
    }
}
